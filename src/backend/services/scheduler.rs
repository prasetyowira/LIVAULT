// src/backend/services/scheduler.rs
// Placeholder for daily maintenance tasks (called by timer or external cron) 

use crate::{
    error::VaultError,
    models::common::{InviteStatus, VaultStatus},
    services::vault_service,
    storage::{self, Cbor, StorableString},
    models::{VaultConfig, VaultInviteToken},
    services::upload_service, // To access ACTIVE_UPLOADS
};
use ic_cdk::api::time;
use std::time::Duration;

// Constants for time calculations (consider moving to a config module)
const DAY_NANOS: u64 = 24 * 60 * 60 * 1_000_000_000;
const HOUR_NANOS: u64 = 60 * 60 * 1_000_000_000;
const FOURTEEN_DAYS_NANOS: u64 = 14 * DAY_NANOS;
const ONE_YEAR_NANOS: u64 = 365 * DAY_NANOS; // Approximate

/// Performs daily maintenance tasks for the entire system.
/// This function is intended to be called by a timer or an external trigger (e.g., Cloudflare Worker).
pub fn perform_daily_maintenance() -> Result<(), VaultError> {
    let current_time = time();
    ic_cdk::print(format!(
        "⚙️ SCHEDULER: Starting daily maintenance at {}",
        current_time
    ));

    let mut errors: Vec<String> = Vec::new();

    // --- Tasks --- //

    // 1. Purge Expired Invite Tokens
    if let Err(e) = purge_expired_invites(current_time) {
        let msg = format!("Failed to purge invites: {:?}", e);
        ic_cdk::eprintln!("🔥 SCHEDULER ERROR: {}", msg);
        errors.push(msg);
    }

    // 2. Check Vault Expirations & Advance Lifecycle States
    if let Err(e) = check_vault_lifecycles(current_time) {
         let msg = format!("Failed to check lifecycles: {:?}", e);
        ic_cdk::eprintln!("🔥 SCHEDULER ERROR: {}", msg);
        errors.push(msg);
    }

    // 3. Cleanup Stale Upload Sessions (if using in-memory staging)
    if let Err(e) = cleanup_stale_uploads(current_time) {
         let msg = format!("Failed to cleanup uploads: {:?}", e);
        ic_cdk::eprintln!("🔥 SCHEDULER ERROR: {}", msg);
        errors.push(msg);
    }

    // 4. TODO: Compact Audit Logs (if implemented)

    // 5. TODO: Other periodic tasks (e.g., recalculate metrics)

    if errors.is_empty() {
        ic_cdk::print("⚙️ SCHEDULER: Daily maintenance completed successfully.");
        Ok(())
    } else {
        ic_cdk::eprintln!("⚙️ SCHEDULER: Daily maintenance completed with {} errors.", errors.len());
        // Combine errors into a single error message
        Err(VaultError::InternalError(format!("Scheduler errors: {}", errors.join("; "))))
    }
}

/// Iterates through invite tokens and marks expired ones.
/// NOTE: This iterates the entire map, which can be inefficient.
pub fn purge_expired_invites(current_time: u64) -> Result<(), VaultError> {
    ic_cdk::print("⚙️ SCHEDULER: Purging expired invite tokens...");
    let mut updates: Vec<(StorableString, Cbor<VaultInviteToken>)> = Vec::new();
    let mut error_count = 0;

    storage::INVITE_TOKENS.with(|map_ref| {
        let map = map_ref.borrow();
        for (key, value) in map.iter() {
            let mut token: VaultInviteToken = value.0;
            if token.status == InviteStatus::Pending && current_time > token.expires_at {
                ic_cdk::print(format!("⏳ SCHEDULER: Marking token {} as expired.", token.token_id));
                token.status = InviteStatus::Expired;
                // Assuming VaultInviteToken has an updated_at field
                // token.updated_at = current_time;
                updates.push((key, Cbor(token)));
            }
        }
    });

    // Apply updates outside the initial borrow
    if !updates.is_empty() {
        storage::INVITE_TOKENS.with(|map_ref| {
            let mut map = map_ref.borrow_mut();
            for (key, value) in updates {
                if let Err(e) = map.insert(key, value) {
                    ic_cdk::eprintln!("🔥 SCHEDULER ERROR: Failed to update expired token: {:?}", e);
                    error_count += 1;
                }
            }
        });
    }

    ic_cdk::print(format!("⚙️ SCHEDULER: Invite token purge finished. {} updates applied, {} errors.", updates.len(), error_count));
    if error_count > 0 {
        Err(VaultError::StorageError(format!("{} errors occurred during invite token purge.", error_count)))
    } else {
        Ok(())
    }
}

/// Checks vault statuses and transitions them based on time (expiry, grace periods).
/// NOTE: This iterates the entire map, which can be inefficient.
pub fn check_vault_lifecycles(current_time: u64) -> Result<(), VaultError> {
    ic_cdk::print("⚙️ SCHEDULER: Checking vault lifecycles...");
    let mut transitions: Vec<(String, VaultStatus)> = Vec::new();
    let mut vault_ids_to_delete: Vec<String> = Vec::new();

    storage::VAULT_CONFIGS.with(|map_ref| {
        let map = map_ref.borrow();
        for (key, value) in map.iter() {
            let config: VaultConfig = value.0;
            let vault_id: String = key.0.0; // Extract VaultId from StorableString(Cbor(VaultId))

            match config.status {
                VaultStatus::Active if current_time > config.expires_at => {
                    ic_cdk::print(format!(
                        "⏳ SCHEDULER: Vault {} expired, moving to GraceMaster.",
                        vault_id
                    ));
                    transitions.push((vault_id, VaultStatus::GraceMaster));
                }
                VaultStatus::GraceMaster if current_time > config.expires_at.saturating_add(FOURTEEN_DAYS_NANOS) => {
                     ic_cdk::print(format!(
                        "⏳ SCHEDULER: Vault {} master grace ended, moving to GraceHeir.",
                        vault_id
                    ));
                    transitions.push((vault_id, VaultStatus::GraceHeir));
                }
                VaultStatus::GraceHeir if current_time > config.expires_at.saturating_add(2 * FOURTEEN_DAYS_NANOS) => {
                    // If grace period ends and not unlocked, mark for deletion
                     ic_cdk::print(format!(
                        "⏳ SCHEDULER: Vault {} heir grace ended without unlock, marking for deletion.",
                        vault_id
                    ));
                    // We transition to Deleted status first, actual data removal happens later or by another trigger
                    transitions.push((vault_id, VaultStatus::Deleted));
                }
                VaultStatus::Unlockable => {
                    let unlock_expiry = config.unlocked_at.map_or(0, |t| t.saturating_add(ONE_YEAR_NANOS));
                    if config.unlocked_at.is_some() && current_time > unlock_expiry {
                        ic_cdk::print(format!(
                            "⏳ SCHEDULER: Vault {} unlock window ended, moving to Expired.",
                            vault_id
                        ));
                        transitions.push((vault_id, VaultStatus::Expired));
                    }
                }
                 VaultStatus::Expired => {
                    // Consider adding a further delay before actual deletion
                    // For now, if it's Expired, mark it for deletion check
                    let expired_duration = config.unlocked_at // Use unlocked_at if available, else expires_at as reference
                        .map_or(config.expires_at, |t| t.saturating_add(ONE_YEAR_NANOS));
                    // Add another buffer (e.g., 30 days) before actual deletion trigger
                    if current_time > expired_duration.saturating_add(30 * DAY_NANOS) {
                         ic_cdk::print(format!(
                            "⏳ SCHEDULER: Vault {} is Expired and past final buffer, marking for data deletion.",
                            vault_id
                        ));
                        vault_ids_to_delete.push(vault_id);
                    }
                 }
                _ => { /* No time-based transition for other states */ }
            }
        }
    });

    let mut error_count = 0;

    // Apply state transitions
    for (vault_id, new_status) in transitions {
        if let Err(e) = vault_service::set_vault_status(&vault_id, new_status, None) { // Triggered by System
             ic_cdk::eprintln!(
                "🔥 SCHEDULER ERROR: Failed vault {} transition to {:?}: {:?}",
                vault_id, new_status, e
            );
            error_count += 1;
        }
    }

    // Trigger actual deletion for vaults marked for deletion
    for vault_id in vault_ids_to_delete {
         ic_cdk::print(format!("⚙️ SCHEDULER: Initiating deletion for vault {}.", vault_id));
         // The delete_vault function needs proper authorization checks.
         // Assuming system (scheduler) is authorized for now.
         let system_principal = ic_cdk::api::id(); // Or a designated admin principal
         if let Err(e) = vault_service::delete_vault(&vault_id, system_principal) {
             ic_cdk::eprintln!(
                "🔥 SCHEDULER ERROR: Failed to delete vault {}: {:?}",
                vault_id, e
            );
            error_count += 1;
         }
    }

    ic_cdk::print(format!("⚙️ SCHEDULER: Vault lifecycle check finished. {} transitions, {} deletions attempted, {} errors.", transitions.len(), vault_ids_to_delete.len(), error_count));
     if error_count > 0 {
        Err(VaultError::InternalError(format!("{} errors occurred during lifecycle checks.", error_count)))
    } else {
        Ok(())
    }
}

/// Cleans up upload sessions that were started but never finished.
pub fn cleanup_stale_uploads(current_time: u64) -> Result<(), VaultError> {
    ic_cdk::print("⚙️ SCHEDULER: Cleaning up stale upload sessions...");
    let cutoff_time = current_time.saturating_sub(24 * HOUR_NANOS);
    let mut removed_count = 0;

    upload_service::ACTIVE_UPLOADS.with(|uploads_ref| {
        let mut uploads = uploads_ref.borrow_mut();
        // Retain only uploads created within the last 24 hours
        uploads.retain(|upload_id, upload_state| {
            if upload_state.created_at < cutoff_time {
                ic_cdk::print(format!(
                    "⏳ SCHEDULER: Removing stale upload session {}. Created at: {}, Cutoff: {}",
                    upload_id, upload_state.created_at, cutoff_time
                ));
                removed_count += 1;
                false // Remove the entry
            } else {
                true // Keep the entry
            }
        });
    });

    ic_cdk::print(format!("⚙️ SCHEDULER: Stale upload cleanup finished. {} sessions removed.", removed_count));
    Ok(())
}

// TODO: Add any other scheduled tasks identified in docs. 
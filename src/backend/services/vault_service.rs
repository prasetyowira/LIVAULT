// src/backend/services/vault_service.rs
// Placeholder for Vault related business logic 

use crate::{
    error::VaultError,
    models::{
        common::*, // Import common types like VaultId, Timestamp, PrincipalId, VaultStatus
        vault_config::VaultConfig, // Import the VaultConfig model
        vault_member::VaultMember, // Needed for listing vaults by member
        UnlockConditions, // Import UnlockConditions
        // Add other models as needed, e.g., VaultUpdate payload struct
    },
    utils::crypto::generate_unique_principal, // Import Principal generation
};
use crate::storage;
use ic_cdk::api::{time, caller}; // For timestamps and caller
use std::time::Duration; // For duration calculations
use candid::Principal as PrincipalId; // Explicit import

// --- Vault Initialization Struct (Example - Define properly in models or api later) ---
// This struct would typically come from the API layer (Phase 3)
#[derive(Clone, Debug, candid::CandidType, serde::Deserialize)]
pub struct VaultInitData {
    pub name: String,
    pub description: Option<String>,
    pub plan: String,         // e.g., "Basic"
    pub owner: PrincipalId, // Owner principal from caller/auth
                              // Unlock conditions might be set here or later
}

// --- Vault Update Struct (Example - Define properly in models or api later) ---
// This struct would typically come from the API layer (Phase 3)
#[derive(Clone, Debug, candid::CandidType, serde::Deserialize)]
pub struct VaultUpdateData {
    pub name: Option<String>,
    pub description: Option<String>,
    pub unlock_conditions: Option<UnlockConditions>,
    pub plan: Option<String>,
    // Add fields for updating other settings if needed
}

// --- Service Functions ---

/// Creates a new vault with initial configuration.
///
/// # Arguments
/// * `init_data` - Initial data for the vault configuration.
///
/// # Returns
/// * `Result<VaultId, VaultError>` - The ID of the newly created vault or an error.
pub async fn create_new_vault(init_data: VaultInitData) -> Result<VaultId, VaultError> {
    let vault_id = generate_unique_principal().await?;
    let current_time = time();

    // Calculate expires_at: 10 years from now (as per PRD)
    let ten_years_duration = Duration::from_secs(10 * 365 * 24 * 60 * 60); // Approx 10 years
    let expires_at = current_time.saturating_add(ten_years_duration.as_nanos() as u64);

    // Determine storage_quota_bytes based on plan (already implemented)
    let storage_quota_bytes = match init_data.plan.as_str() {
        "Basic" => 5 * 1024 * 1024,         // 5 MB
        "Standard" => 10 * 1024 * 1024,     // 10 MB
        "Premium" => 50 * 1024 * 1024,      // 50 MB
        "Deluxe" => 100 * 1024 * 1024,    // 100 MB
        "Titan" => 250 * 1024 * 1024,       // 250 MB
        _ => return Err(VaultError::InvalidInput("Invalid plan specified".to_string())), // Use InvalidInput
    };

    let config = VaultConfig {
        vault_id: vault_id.clone(),
        owner: init_data.owner,
        name: init_data.name,
        description: init_data.description,
        status: VaultStatus::Draft, // Initial state as per architecture
        plan: init_data.plan,
        storage_quota_bytes,
        storage_used_bytes: 0,
        unlock_conditions: Default::default(), // Default unlock conditions initially
        created_at: current_time,
        updated_at: current_time,
        expires_at,
        unlocked_at: None,
        last_accessed_by_owner: Some(current_time), // Owner created it
    };

    // Store the configuration using the dedicated storage helper function
    match storage::vault_configs::insert_vault_config(&config) {
        Some(_) => Err(VaultError::AlreadyExists(vault_id)), // Should not happen if ID is unique
        None => Ok(vault_id),
    }
    /* // OLD direct access - Incorrect key type!
    storage::VAULT_CONFIGS.with(|map| {
        let key = Cbor(vault_id.clone()); // This key is Cbor<Principal>
        let value = Cbor(config); // Map expects Cbor<String> key

        // Correct match for Option<V> returned by insert
        match map.borrow_mut().insert(key, value) {
            Some(_) => Err(VaultError::AlreadyExists(vault_id)), // Should not happen if ID is unique
            None => Ok(vault_id),
        }
    }) */
}

/// Retrieves a vault's configuration by its ID.
///
/// # Arguments
/// * `vault_id` - The ID of the vault to retrieve.
///
/// # Returns
/// * `Result<VaultConfig, VaultError>` - The vault configuration or an error if not found.
pub async fn get_vault_config(vault_id: &VaultId) -> Result<VaultConfig, VaultError> {
    // Use the helper function which handles key conversion
    storage::vault_configs::get_vault_config(vault_id)
        .ok_or_else(|| VaultError::VaultNotFound(vault_id.clone().to_string()))
}

/// Updates an existing vault's configuration.
/// Only certain fields might be updatable depending on the state and permissions.
///
/// # Arguments
/// * `vault_id` - The ID of the vault to update.
/// * `update_data` - The data containing fields to update.
/// * `caller` - The principal attempting the update (for authorization checks).
///
/// # Returns
/// * `Result<(), VaultError>` - Success or an error.
pub async fn update_vault_config(
    vault_id: &VaultId,
    update_data: VaultUpdateData,
    caller: PrincipalId,
) -> Result<(), VaultError> {
    // 1. Retrieve existing config using the helper
    let mut config = storage::vault_configs::get_vault_config(vault_id)
        .ok_or_else(|| VaultError::VaultNotFound(vault_id.clone().to_string()))?;

    // 2. Authorization Check: Ensure caller is the owner
    if config.owner != caller {
        return Err(VaultError::NotAuthorized(format!(
            "Caller {} is not the owner of vault {}",
            caller, vault_id
        )));
    }

    // --- Apply updates (logic remains the same) ---
    let mut updated = false;
    if let Some(name) = update_data.name {
        if config.name != name {
            config.name = name;
            updated = true;
        }
    }
    // Compare Option<String> with Option<String>
    if update_data.description != config.description {
         config.description = update_data.description;
         updated = true;
    }
    if let Some(unlock_conditions) = update_data.unlock_conditions {
        // Basic structure validation is handled by Candid types.
        // Complex validation (e.g., thresholds vs member count) might go here if needed.
        if config.unlock_conditions != unlock_conditions {
            config.unlock_conditions = unlock_conditions;
            updated = true;
        }
    }
    if let Some(new_plan) = update_data.plan {
        if config.plan != new_plan {
            // TODO: Implement prorate calculation for plan change.
            // This likely requires integration with payment_service to handle potential
            // cost differences and payment verification before applying the change.
            // For now, just update the plan name and quota as an example.
            let new_storage_quota_bytes = match new_plan.as_str() {
                "Basic" => 5 * 1024 * 1024,
                "Standard" => 10 * 1024 * 1024,
                "Premium" => 50 * 1024 * 1024,
                "Deluxe" => 100 * 1024 * 1024,
                "Titan" => 250 * 1024 * 1024,
                _ => return Err(VaultError::InvalidInput("Invalid new plan specified".to_string())),
            };
            // Check if new quota is sufficient for current usage
            if new_storage_quota_bytes < config.storage_used_bytes {
                return Err(VaultError::StorageError("New plan quota is less than current usage.".to_string()));
            }
            config.plan = new_plan;
            config.storage_quota_bytes = new_storage_quota_bytes;
            updated = true;
            ic_cdk::print(format!("📝 INFO: Vault {} plan updated to {} by owner {}", vault_id, config.plan, caller));
        }
    }
    // --- End Apply updates ---

    // 4. If updated, update timestamp and save using the helper
    if updated {
        config.updated_at = time();
        // Insert the updated config back using the helper function
        match storage::vault_configs::insert_vault_config(&config) {
            Some(_) => {
                // Insertion successful (updated existing)
                 ic_cdk::print(format!("📝 INFO: Vault {} updated by owner {}", vault_id, caller));
                 Ok(())
             },
             None => {
                // Insertion successful (was new? should not happen in update)
                ic_cdk::eprintln!("⚠️ WARNING: Vault {} config insertion reported None during update.", vault_id);
                Ok(()) // Treat as success
             }
             // Note: insert_vault_config doesn't return Result, so no Err case here.
             //       Error handling would need to be added to the storage function itself if needed.
        }

        /* // OLD Direct Access
        storage::VAULT_CONFIGS.with(|map| {
            let key = Cbor(vault_id.clone()); // OLD direct access
            let mut borrowed_map = map.borrow_mut();
            match borrowed_map.insert(key, Cbor(config)) { // Key type mismatch!
                 Some(_) => {
                     ic_cdk::print(format!("📝 INFO: Vault {} updated by owner {}", vault_id, caller));
                     Ok(())
                 },
                 None => {
                    // Should not happen if we just fetched it, but handle defensively
                    Err(VaultError::StorageError("Failed to update vault config (vault disappeared?)".to_string()))
                 },
            }
        })
        */
    } else {
        Ok(()) // No changes made
    }
}

/// Saves the provided VaultConfig to stable storage.
/// This is intended for internal use by services after modifying config.
///
/// # Arguments
/// * `config` - The VaultConfig object to save.
///
/// # Returns
/// * `Result<(), VaultError>` - Success or storage error.
pub async fn save_vault_config(config: &VaultConfig) -> Result<(), VaultError> {
    // Use the helper function directly
    storage::vault_configs::insert_vault_config(config);
    // insert_vault_config returns Option<VaultConfig>, not Result.
    // Assume success for now. Add error handling in storage if needed.
    Ok(())
    /* // OLD Direct Access
    let key = Cbor(config.vault_id.clone()); // OLD direct access
    let value = Cbor(config.clone()); // Clone the config to store
    storage::VAULT_CONFIGS.with(|map| {
        // Correct match for Option<V> returned by insert
        match map.borrow_mut().insert(key, value) { // Key type mismatch!
            Some(_) => Ok(()),
            None => Ok(()),
        }
    }) */
}

/// Sets the status of a vault. This function should enforce valid state transitions.
///
/// # Arguments
/// * `vault_id` - The ID of the vault.
/// * `new_status` - The new status to set.
/// * `triggering_principal` - Optional principal triggering the change (for logging/audit).
///
/// # Returns
/// * `Result<(), VaultError>` - Success or an error if the transition is invalid or the vault is not found.
pub async fn set_vault_status(vault_id: &VaultId, new_status: VaultStatus, triggering_principal: Option<PrincipalId>) -> Result<(), VaultError> {
    // 1. Retrieve existing config using the helper
    let mut config = storage::vault_configs::get_vault_config(vault_id)
        .ok_or_else(|| VaultError::VaultNotFound(vault_id.clone().to_string()))?;

    let old_status = config.status;

    // --- State Transition Validation (Logic remains the same) ---
    let is_valid_transition = match (old_status, new_status) {
        // Initial Setup Flow
        (VaultStatus::Draft, VaultStatus::NeedSetup) => true, // After creation + payment verification
        (VaultStatus::NeedSetup, VaultStatus::Active) => true, // After owner finishes setup (content upload) & invites are claimed (finalize_setup)

        // Normal Operations / Updates
        (VaultStatus::Active, VaultStatus::Active) => true, // Allow staying Active (e.g., content update)
        (VaultStatus::Active, VaultStatus::GracePeriodPlan) => true, // Plan expires, moves to grace
        (VaultStatus::Active, VaultStatus::Unlockable) => true, // Unlock conditions met, triggered by witness

        // Unlock Flow
        (VaultStatus::Unlockable, VaultStatus::Unlocked) => true, // Heir confirms unlock
        (VaultStatus::Unlocked, VaultStatus::GracePeriodUnlock) => true, // Unlock access window expires

        // Grace Periods & Expiry
        (VaultStatus::GracePeriodPlan, VaultStatus::Active) => true, // Payment made during grace
        (VaultStatus::GracePeriodPlan, VaultStatus::Expired) => true, // Grace period ends without payment
        (VaultStatus::GracePeriodUnlock, VaultStatus::Expired) => true, // Grace period ends after unlock window

        // Deletion (Manual/Cron)
        (VaultStatus::Expired, VaultStatus::Deleted) => true, // Cron job or admin action cleans up expired vaults
        (_, VaultStatus::Deleted) => true, // Allow deletion from almost any state (e.g., admin override, edge cases)

        // Self-loops are generally allowed if not explicitly denied
        (s1, s2) if s1 == s2 => true,

        // Deny all other transitions
        _ => false,
    };

    if !is_valid_transition {
        return Err(VaultError::InvalidStateTransition(format!(
            "Cannot transition vault {} from {:?} to {:?}",
            vault_id, old_status, new_status
        )));
    }
    // --- End State Transition Validation ---

    // If transition is valid, update the status and timestamp
    if old_status != new_status {
        config.status = new_status;
        config.updated_at = time(); // Update timestamp on status change

        // Specific logic for entering certain states
        if new_status == VaultStatus::Unlocked {
            config.unlocked_at = Some(time());
        }
        // Reset unlocked_at if moving *out* of Unlocked state (e.g., back to Active if that were allowed)
        else if old_status == VaultStatus::Unlocked && new_status != VaultStatus::Unlocked {
             config.unlocked_at = None;
        }

        // Insert the updated config back using the helper function
        match storage::vault_configs::insert_vault_config(&config) {
            Some(_) => {
                 let principal_str = triggering_principal.map_or_else(|| "System".to_string(), |p| p.to_string());
                 ic_cdk::print(format!("📝 INFO: Vault {} status changed from {:?} to {:?} by {}", vault_id, old_status, new_status, principal_str));
                 Ok(())
             },
             None => {
                 ic_cdk::eprintln!("⚠️ WARNING: Vault {} config insertion reported None during status update.", vault_id);
                 Ok(()) // Treat as success
             }
             // Note: insert_vault_config doesn't return Result.
        }
        /* // OLD Direct Access
        storage::VAULT_CONFIGS.with(|map| {
             let key = Cbor(vault_id.clone());
             let mut borrowed_map = map.borrow_mut();
             match borrowed_map.insert(key, Cbor(config)) { // Key type mismatch!
                 Some(_) => {
                     let principal_str = triggering_principal.map_or_else(|| "System".to_string(), |p| p.to_string());
                     ic_cdk::print(format!("📝 INFO: Vault {} status changed from {:?} to {:?} by {}", vault_id, old_status, new_status, principal_str));
                     Ok(())
                 },
                 None => {
                     // Should not happen if we just fetched it
                     Err(VaultError::StorageError("Failed to update vault status (vault disappeared?)".to_string()))
                 },
             }
        }) */
    } else {
        Ok(()) // No status change needed
    }
}

/// Trigger vault unlock process (e.g., called by witness or scheduler).
/// Checks unlock conditions and transitions state to Unlockable if met.
///
/// # Arguments
/// * `vault_id` - The ID of the vault.
/// * `caller` - The principal triggering the unlock (witness/admin).
///
/// # Returns
/// * `Result<(), VaultError>` - Success or an error.
pub async fn trigger_unlock(vault_id: &VaultId, caller: PrincipalId) -> Result<(), VaultError> {
    let config = get_vault_config(vault_id).await?;

    // Authorization: Check if caller is a witness or admin (add roles later)
    // For now, let's allow any principal to trigger for testing, but add checks.
    // Example: Check if caller is in the vault_members list with role Witness
    // let members = invite_service::get_members_for_vault(vault_id)?;
    // if !members.iter().any(|m| m.principal == caller && m.role == Role::Witness) {
    //     return Err(VaultError::NotAuthorized("Only a witness can trigger unlock".to_string()));
    // }

    // Check if vault is in a state where unlock can be triggered (e.g., Active, GraceMaster, GraceHeir)
    if !matches!(config.status, VaultStatus::Active | VaultStatus::GraceMaster | VaultStatus::GraceHeir) {
        return Err(VaultError::InternalError(format!("Cannot trigger unlock from status {:?}", config.status)));
    }

    // Check unlock conditions (time, inactivity, approvals)
    let conditions_met = check_unlock_conditions(&config).await?;

    if conditions_met {
        ic_cdk::print(format!("🔓 INFO: Unlock conditions met for vault {}. Triggered by {}.", vault_id, caller));
        set_vault_status(vault_id, VaultStatus::Unlockable, Some(caller)).await
    } else {
        ic_cdk::print(format!("⏳ INFO: Unlock trigger for vault {} received by {}, but conditions not yet met.", vault_id, caller));
        // Optionally log which conditions failed
        Err(VaultError::UnlockConditionsNotMet)
    }
}

/// Placeholder function to check if unlock conditions are met for a vault.
/// This needs to implement the logic based on `config.unlock_conditions`.
async fn check_unlock_conditions(config: &VaultConfig) -> Result<bool, VaultError> {
    let current_time = time();
    let conditions = &config.unlock_conditions;

    // 1. Time-based unlock
    if let Some(unlock_time) = conditions.time_based_unlock_epoch_sec {
        if current_time >= unlock_time {
            ic_cdk::print(format!(
                "✅ UNLOCK CHECK: Vault {} passed time-based condition.",
                config.vault_id
            ));
            return Ok(true);
        }
    }

    // 2. Inactivity-based unlock
    if let Some(inactivity_sec) = conditions.inactivity_duration_sec {
        // Use last_accessed_by_owner if available, otherwise updated_at as fallback
        let last_active_time = config.last_accessed_by_owner.unwrap_or(config.updated_at);
        let inactivity_duration_nanos = current_time.saturating_sub(last_active_time);
        // Convert inactivity_sec (u64 seconds) to nanos for comparison
        let required_inactivity_nanos = (inactivity_sec as u128) * 1_000_000_000;

        if inactivity_duration_nanos as u128 >= required_inactivity_nanos {
            ic_cdk::print(format!(
                "✅ UNLOCK CHECK: Vault {} passed inactivity condition ({}s).",
                config.vault_id, inactivity_sec
            ));
            return Ok(true);
        }
    }

    // 3. Approval Threshold
    // Assume a storage function `get_approval_status` exists, returning counts.
    // This storage module/function needs implementation based on `storage.md`.
    let required_heirs = conditions.required_heir_approvals.unwrap_or(0);
    let required_witnesses = conditions.required_witness_approvals.unwrap_or(0);

    if required_heirs > 0 || required_witnesses > 0 {
        match storage::approvals::get_approval_status(&config.vault_id).await {
            Ok(approvals) => {
                 if approvals.heir_approvals >= required_heirs && approvals.witness_approvals >= required_witnesses {
                    ic_cdk::print(format!(
                        "✅ UNLOCK CHECK: Vault {} passed approval threshold (Heirs: {}/{}, Witnesses: {}/{}).",
                        config.vault_id,
                        approvals.heir_approvals, required_heirs,
                        approvals.witness_approvals, required_witnesses
                    ));
                    return Ok(true);
                } else {
                     ic_cdk::print(format!(
                        "⏳ UNLOCK CHECK: Vault {} pending approvals (Heirs: {}/{}, Witnesses: {}/{}).",
                        config.vault_id,
                        approvals.heir_approvals, required_heirs,
                        approvals.witness_approvals, required_witnesses
                    ));
                }
            }
            Err(e) => {
                // Log error fetching approvals, but don't block other checks unless this is the only condition
                 ic_cdk::eprintln!(
                    "⚠️ WARNING: Failed to get approval status for vault {}: {:?}",
                    config.vault_id, e
                 );
                 // Decide if failure to get approvals prevents unlock. For now, assume it does if approvals are required.
                 return Err(VaultError::StorageError(format!("Failed to check approvals: {}", e)));
            }
        }
    }


    // If none of the conditions are met
    ic_cdk::print(format!("⏳ UNLOCK CHECK: Vault {} conditions not met.", config.vault_id));
    Ok(false)
}

/// Lists all vaults owned by a specific principal.
///
/// **Note:** This implementation iterates through all vaults and is inefficient.
/// A secondary index (e.g., `owner_principal -> Vec<vault_id>`) in stable storage
/// is required for scalable performance.
///
/// # Arguments
/// * `owner` - The PrincipalId of the owner.
///
/// # Returns
/// * `Result<Vec<VaultConfig>, VaultError>` - A list of vault configurations or an error.
pub async fn get_vaults_by_owner(owner: PrincipalId) -> Result<Vec<VaultConfig>, VaultError> {
    let owned = storage::get_vaults_config_by_owner(owner);
    Ok(owned)
}

/// Lists all vaults a specific principal is a member of (Heir or Witness).
///
/// # Arguments
/// * `member_principal` - The PrincipalId of the member.
///
/// # Returns
/// * `Result<Vec<VaultConfig>, VaultError>` - A list of vault configurations or an error.
pub async fn get_vaults_by_member(member_principal: PrincipalId) -> Result<Vec<VaultConfig>, VaultError> {
    let member_vaults = storage::get_vaults_by_member(member_principal);
    Ok(member_vaults)
}

/// Lists all vaults (for admin use).
/// Supports pagination.
///
/// # Arguments
/// * `offset` - Number of vaults to skip.
/// * `limit` - Maximum number of vaults to return.
///
/// # Returns
/// * `Result<(Vec<VaultConfig>, u64), VaultError>` - A tuple containing a vector of vault configurations and the total count, or an error.
pub async fn list_all_vaults(offset: u64, limit: usize) -> Result<(Vec<VaultConfig>, u64), VaultError> {
    storage::vault_configs::CONFIGS.with(|map_ref| {
        let map = map_ref.borrow();
        let total = map.len();
        let vaults: Vec<VaultConfig> = map.iter()
            .skip(offset as usize)
            .take(limit)
            .map(|(_key, value)| value.0) // Extract VaultConfig from Cbor
            .collect();
        Ok((vaults, total))
    })
}

/// Deletes a vault and potentially associated data.
/// Requires owner authorization and specific vault status (e.g., Expired).
///
/// # Arguments
/// * `vault_id` - The ID of the vault to delete.
/// * `caller` - The principal attempting the deletion.
///
/// # Returns
/// * `Result<(), VaultError>` - Success or an error.
pub async fn delete_vault(vault_id: &VaultId, caller: PrincipalId) -> Result<(), VaultError> {
    let config = get_vault_config(vault_id).await?;

    // 1. Authorization Check: Ensure caller is the owner
    if config.owner != caller {
        return Err(VaultError::NotAuthorized(format!(
            "Caller {} is not the owner of vault {}",
            caller, vault_id
        )));
    }

    // 2. Status Check: Ensure vault is in a deletable state (e.g., Expired)
    //    Alternatively, allow deletion from any state but log appropriately.
    //    For now, let's restrict to Expired or Deleted (for idempotency).
    if !matches!(config.status, VaultStatus::Expired | VaultStatus::Deleted) {
        return Err(VaultError::InvalidState(format!(
            "Vault {} cannot be deleted from status {:?}. Must be Expired.",
            vault_id, config.status
        )));
    }

    // If already Deleted, return Ok for idempotency
    if config.status == VaultStatus::Deleted {
        ic_cdk::print(format!("ℹ️ INFO: Vault {} is already marked as Deleted.", vault_id));
        return Ok(());
    }

    ic_cdk::print(format!("🗑️ INFO: Initiating deletion for vault {} by owner {}", vault_id, caller));

    // --- Cleanup Steps (Placeholders - Require Implementation) ---

    // TODO: Delete associated members from storage::members
    // storage::members::remove_members_by_vault(vault_id).await?;

    // TODO: Delete associated content items (metadata + chunks)
    // let content_ids = storage::content_index::get_index(vault_id).await?.unwrap_or_default();
    // for content_id_str in content_ids {
    //    let content_id = PrincipalId::from_text(&content_id_str)?; // Assuming stored as text
    //    // Lookup internal ID using index
    //    if let Some(internal_content_id) = storage::content::get_internal_content_id(content_id).await? {
    //        storage::uploads::delete_content_chunks(internal_content_id).await?; // Need function to delete associated chunks
    //        storage::content::remove_content(internal_content_id, content_id).await?;
    //    }
    // }
    // storage::content_index::remove_index(vault_id).await?;


    // TODO: Delete associated invite tokens from storage::tokens (if not handled by scheduler)
    // storage::tokens::remove_tokens_by_vault(vault_id).await?;

    // TODO: Delete associated audit logs from storage::audit_logs
    // storage::audit_logs::remove_logs(vault_id).await?;

    // TODO: Delete associated approvals from storage::approvals
    // storage::approvals::remove_approvals(vault_id).await?;

    // --- Final Step: Remove Vault Config ---
    match storage::vault_configs::remove_vault_config(vault_id).await {
        Ok(Some(_)) => {
            ic_cdk::print(format!("✅ SUCCESS: Vault {} configuration removed.", vault_id));
            // Optionally, update metrics
            // storage::metrics::decrement_vault_count().await?;
             // Set status to Deleted explicitly before final removal? Or just remove.
             // For consistency, maybe call set_vault_status first, then remove?
             // set_vault_status(vault_id, VaultStatus::Deleted, Some(caller)).await?;
             // For now, direct removal after cleanup seems simpler.
             Ok(())
        }
        Ok(None) => {
             ic_cdk::eprintln!("⚠️ WARNING: Vault {} config already removed during deletion process.", vault_id);
             Ok(()) // Idempotent
        }
        Err(e) => {
            ic_cdk::eprintln!("❌ ERROR: Failed to remove vault {} config during deletion: {:?}", vault_id, e);
            Err(VaultError::StorageError(format!("Failed to remove vault config: {}", e)))
        }
    }

    // Note: Billing records are likely kept for historical purposes and not deleted.
}
// src/backend/services/vault_service.rs
// Placeholder for Vault related business logic 

use crate::{
    error::VaultError,
    models::{
        common::*, // Import common types like VaultId, Timestamp, PrincipalId, VaultStatus
        VaultConfig, // Import the VaultConfig model
        UnlockConditions, // Import UnlockConditions
        // Add other models as needed, e.g., VaultUpdate payload struct
    },
    storage::{self, Cbor, StorableString}, // Import storage functions/structs
    utils::crypto::generate_ulid, // Import ULID generation
};
use ic_cdk::api::time; // For timestamps
use std::time::Duration; // For duration calculations

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
    let vault_id = generate_ulid().await;
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
        _ => return Err(VaultError::InternalError("Invalid plan specified".to_string())), // Or a specific PlanInvalid error
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

    // Store the configuration
    storage::VAULT_CONFIGS.with(|map| {
        let key = StorableString(Cbor(vault_id.clone()));
        let value = Cbor(config);
        map.borrow_mut()
            .insert(key, value)
            .map_err(|e| VaultError::StorageError(format!("Failed to insert vault config: {:?}", e)))?;
        Ok(vault_id)
    })
}

/// Retrieves a vault's configuration by its ID.
///
/// # Arguments
/// * `vault_id` - The ID of the vault to retrieve.
///
/// # Returns
/// * `Result<VaultConfig, VaultError>` - The vault configuration or an error if not found.
pub fn get_vault_config(vault_id: &VaultId) -> Result<VaultConfig, VaultError> {
    storage::VAULT_CONFIGS.with(|map| {
        let key = StorableString(Cbor(vault_id.clone()));
        match map.borrow().get(&key) {
            Some(storable_config) => Ok(storable_config.0), // Access inner VaultConfig from Cbor wrapper
            None => Err(VaultError::VaultNotFound(vault_id.clone())),
        }
    })
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
pub fn update_existing_vault(
    vault_id: &VaultId,
    update_data: VaultUpdateData,
    caller: PrincipalId,
) -> Result<(), VaultError> {
    storage::VAULT_CONFIGS.with(|map| {
        let key = StorableString(Cbor(vault_id.clone()));
        let mut borrowed_map = map.borrow_mut();

        // 1. Retrieve existing config
        let mut config = match borrowed_map.get(&key) {
            Some(storable_config) => storable_config.0,
            None => return Err(VaultError::VaultNotFound(vault_id.clone())),
        };

        // 2. Authorization Check: Ensure caller is the owner
        if config.owner != caller {
            return Err(VaultError::NotAuthorized(format!(
                "Caller {} is not the owner of vault {}",
                caller, vault_id
            )));
        }

        // 3. Apply updates
        let mut updated = false;
        if let Some(name) = update_data.name {
            if config.name != name {
                config.name = name;
                updated = true;
            }
        }
        if let Some(description) = update_data.description {
             if config.description != description {
                config.description = description;
                updated = true;
            }
        }
        if let Some(unlock_conditions) = update_data.unlock_conditions {
            // TODO: Add validation for unlock conditions if needed
            if config.unlock_conditions != unlock_conditions {
                config.unlock_conditions = unlock_conditions;
                updated = true;
            }
        }
        if let Some(new_plan) = update_data.plan {
            if config.plan != new_plan {
                // TODO: Implement prorate calculation for plan change.
                // This would involve checking the new plan's validity, calculating cost difference,
                // potentially initiating a new payment flow if needed, and updating quota.
                // For now, just update the plan name and quota as an example.
                let new_storage_quota_bytes = match new_plan.as_str() {
                    "Basic" => 5 * 1024 * 1024,
                    "Standard" => 10 * 1024 * 1024,
                    "Premium" => 50 * 1024 * 1024,
                    "Deluxe" => 100 * 1024 * 1024,
                    "Titan" => 250 * 1024 * 1024,
                    _ => return Err(VaultError::InternalError("Invalid new plan specified".to_string())),
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


        // 4. If updated, update timestamp and save
        if updated {
            config.updated_at = time();
            borrowed_map
                .insert(key, Cbor(config))
                .map_err(|e| VaultError::StorageError(format!("Failed to update vault config: {:?}", e)))?;
            ic_cdk::print(format!("📝 INFO: Vault {} updated by owner {}", vault_id, caller));
        }

        Ok(())
    })
}

/// Changes the status of a vault based on defined lifecycle rules.
///
/// # Arguments
/// * `vault_id` - The ID of the vault.
/// * `new_status` - The target status.
/// * `triggering_principal` - Optional principal causing the state change (for logging/validation).
///
/// # Returns
/// * `Result<(), VaultError>` - Success or an error.
pub fn set_vault_status(vault_id: &VaultId, new_status: VaultStatus, triggering_principal: Option<PrincipalId>) -> Result<(), VaultError> {
    storage::VAULT_CONFIGS.with(|map| {
        let key = StorableString(Cbor(vault_id.clone()));
        let mut borrowed_map = map.borrow_mut();

        let mut config = match borrowed_map.get(&key) {
            Some(storable_config) => storable_config.0,
            None => return Err(VaultError::VaultNotFound(vault_id.clone())),
        };

        let old_status = config.status;

        // Validate state transition based on prd.md diagram
        match (old_status, new_status) {
            // Payment & Setup
            (VaultStatus::Draft, VaultStatus::NeedSetup) => (), // payment_success
            (VaultStatus::NeedSetup, VaultStatus::SetupComplete) => (), // content_set + heir_claimed (or finalized by owner)
            (VaultStatus::SetupComplete, VaultStatus::Active) => (), // setup_finalized

            // Expiry Path
            (VaultStatus::Active, VaultStatus::GraceMaster) => (), // expires_at hit
            (VaultStatus::GraceMaster, VaultStatus::GraceHeir) => (), // 14d no master action
            (VaultStatus::GraceHeir, VaultStatus::Deleted) => (), // 14d no unlock action

            // Unlock Path
            (VaultStatus::Active, VaultStatus::Unlockable) => (), // manual_unlock_valid (e.g., witness trigger + quorum)
            (VaultStatus::GraceHeir, VaultStatus::Unlockable) => (), // unlock_conditions_met during heir grace

            // Post-Unlock / Purge Path
            (VaultStatus::Unlockable, VaultStatus::Expired) => (), // unlock_window_ended (e.g., 1 year post-unlock)
            (VaultStatus::Expired, VaultStatus::Deleted) => (), // purge trigger

            // Allow setting to the same status (idempotent)
            (s1, s2) if s1 == s2 => (),

            // Reject all other transitions
            _ => {
                return Err(VaultError::InvalidStateTransition(format!(
                    "Invalid status transition for vault {} from {:?} to {:?}",
                    vault_id, old_status, new_status
                )));
            }
        }

        // Update status and timestamp
        config.status = new_status;
        config.updated_at = time();

        // Record unlock time if applicable
        if new_status == VaultStatus::Unlockable && config.unlocked_at.is_none() {
            config.unlocked_at = Some(time());
        }

        // Save the updated configuration
        borrowed_map
            .insert(key, Cbor(config))
            .map_err(|e| VaultError::StorageError(format!("Failed to set vault status: {:?}", e)))?;

        ic_cdk::print(format!(
            "🔄 INFO: Vault {} status changed from {:?} to {:?} (Triggered by: {:?})",
            vault_id,
            old_status,
            new_status,
            triggering_principal.map(|p| p.to_string()).unwrap_or_else(|| "System".to_string())
        ));

        Ok(())
    })
}

/// Retrieves all vault configurations owned by a specific principal.
/// NOTE: This iterates through all vaults and can be inefficient for large numbers of vaults.
/// Consider adding a secondary index (owner -> Vec<vault_id>) for performance if needed.
///
/// # Arguments
/// * `owner` - The principal ID of the owner.
///
/// # Returns
/// * `Result<Vec<VaultConfig>, VaultError>` - A list of vault configurations owned by the principal.
pub fn get_vaults_by_owner(owner: PrincipalId) -> Result<Vec<VaultConfig>, VaultError> {
     ic_cdk::print(format!("⏳ INFO: Iterating vaults to find those owned by {}", owner));
    // TODO: Implement efficient iteration over VAULT_CONFIGS stable BTreeMap.
    // The current `ic-stable-structures` BTreeMap doesn't directly support efficient
    // value-based filtering during iteration without reading all values.
    // A possible approach is to iterate keys/values and filter in memory.
    // For large scale, a secondary index (e.g., another BTreeMap mapping Owner -> Vec<VaultId>)
    // would be necessary.

    let mut owned_vaults = Vec::new();
    storage::VAULT_CONFIGS.with(|map_ref| {
        let map = map_ref.borrow();
        for (_key, value) in map.iter() {
             let config: VaultConfig = value.0; // Access inner VaultConfig from Cbor wrapper
             if config.owner == owner {
                 owned_vaults.push(config);
             }
        }
    });

    ic_cdk::print(format!("✅ INFO: Found {} vaults owned by {}", owned_vaults.len(), owner));
    Ok(owned_vaults)
}

/// Deletes a vault and all associated data (members, content, invites, etc.).
/// This is a destructive operation and should be used with caution, typically only
/// after a vault reaches the end of its lifecycle (e.g., Deleted status).
///
/// # Arguments
/// * `vault_id` - The ID of the vault to delete.
/// * `caller` - Principal attempting the deletion (for authorization/logging).
///
/// # Returns
/// * `Result<(), VaultError>` - Success or an error.
pub fn delete_vault(vault_id: &VaultId, caller: PrincipalId) -> Result<(), VaultError> {
    ic_cdk::print(format!("🗑️ WARNING: Attempting to delete vault {} by caller {}", vault_id, caller));

    // 1. Authorization: Ensure caller is authorized (e.g., system admin or maybe owner in specific states?)
    // TODO: Implement proper authorization for deletion. For now, allow deletion if called.

    // 2. Delete Vault Config
    let removed_config = storage::VAULT_CONFIGS.with(|map| {
        let key = StorableString(Cbor(vault_id.clone()));
        map.borrow_mut().remove(&key)
    });

    if removed_config.is_none() {
        // Vault config already gone or never existed. Might still need cleanup.
        ic_cdk::print(format!("⚠️ WARN: Vault config for {} not found during deletion, continuing cleanup.", vault_id));
    } else {
         ic_cdk::print(format!("✅ INFO: Vault config for {} removed.", vault_id));
    }

    // 3. Delete Associated Data
    // TODO: Implement deletion logic for associated data using prefix iteration/deletion if keys support it.
    // This needs to cover:
    // - VAULT_MEMBERS (prefix: `member:<vault_id>:`)
    // - INVITE_TOKENS (potentially iterate and check `vault_id` field if no prefix)
    // - CONTENT_ITEMS (prefix: `content:<content_id>`) -> Requires getting Content IDs first from CONTENT_INDEX
    // - CONTENT_INDEX (prefix: `content_idx:<vault_id>`)
    // - UPLOAD_STAGING (if moved to stable memory)
    // - APPROVALS (prefix: `approval:<vault_id>`)
    // - AUDIT_LOGS (prefix: `audit:<vault_id>`)

    ic_cdk::print(format!("⏳ INFO: Placeholder for deleting associated data for vault {}", vault_id));
    // Example (conceptual - requires actual iteration implementation):
    // storage::delete_by_prefix(storage::VAULT_MEMBERS, format!("member:{}:", vault_id).as_bytes());
    // storage::delete_by_prefix(storage::CONTENT_INDEX, format!("content_idx:{}:", vault_id).as_bytes());
    // ... and so on for other maps ...

    ic_cdk::print(format!("✅ INFO: Vault {} deletion process completed by caller {}", vault_id, caller));
    Ok(())
}

// TODO: Add functions for vault deletion (handle cleanup of members, content, etc.)
// TODO: Add functions to get vaults by owner, etc. (requires secondary indexing or iteration) 
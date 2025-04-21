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
    storage::{self, Cbor, StorableString, VAULT_CONFIGS, VAULT_MEMBERS}, // Import storage functions/structs
    utils::crypto::generate_ulid, // Import ULID generation
};
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

    // Store the configuration
    storage::VAULT_CONFIGS.with(|map| {
        let key = Cbor(vault_id.clone());
        let value = Cbor(config);

        // Correct match for Option<V> returned by insert
        match map.borrow_mut().insert(key, value) {
            Some(_) => Err(VaultError::AlreadyExists(vault_id)), // Should not happen if ID is unique
            None => Ok(vault_id),
        }
    })
}

/// Retrieves a vault's configuration by its ID.
///
/// # Arguments
/// * `vault_id` - The ID of the vault to retrieve.
///
/// # Returns
/// * `Result<VaultConfig, VaultError>` - The vault configuration or an error if not found.
pub async fn get_vault_config(vault_id: &VaultId) -> Result<VaultConfig, VaultError> {
    storage::VAULT_CONFIGS.with(|map| {
        let key = Cbor(vault_id.clone()); // Use Cbor directly
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
pub async fn update_vault_config(
    vault_id: &VaultId,
    update_data: VaultUpdateData,
    caller: PrincipalId,
) -> Result<(), VaultError> {
    storage::VAULT_CONFIGS.with(|map| {
        let key = Cbor(vault_id.clone()); // Use Cbor directly
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
        // Compare Option<String> with Option<String>
        if update_data.description != config.description {
             config.description = update_data.description;
             updated = true;
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

        // 4. If updated, update timestamp and save
        if updated {
            config.updated_at = time();
            // Insert the updated config back
            // Correct match for Option<V> returned by insert
            match borrowed_map.insert(key, Cbor(config)) {
                 Some(_) => {
                     ic_cdk::print(format!("📝 INFO: Vault {} updated by owner {}", vault_id, caller));
                     Ok(())
                 },
                 None => {
                    // Should not happen if we just fetched it, but handle defensively
                    Err(VaultError::StorageError("Failed to update vault config (vault disappeared?)".to_string()))
                 },
            }
        } else {
            Ok(()) // No changes made
        }
    })
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
    let key = Cbor(config.vault_id.clone());
    let value = Cbor(config.clone()); // Clone the config to store
    storage::VAULT_CONFIGS.with(|map| {
        // Correct match for Option<V> returned by insert
        match map.borrow_mut().insert(key, value) {
            Some(_) => Ok(()),
            None => Ok(()),
        }
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
pub async fn set_vault_status(vault_id: &VaultId, new_status: VaultStatus, _triggering_principal: Option<PrincipalId>) -> Result<(), VaultError> {
    storage::VAULT_CONFIGS.with(|map| {
        let key = Cbor(vault_id.clone()); // Use Cbor directly
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
            (s1, s2) if s1 == s2 => {
                 ic_cdk::print(format!("ℹ️ INFO: Vault {} status already {:?}. No change needed.", vault_id, new_status));
                 return Ok(()); // No change needed
            },

            // Reject all other transitions
            _ => {
                // Use a specific error variant if available, otherwise InternalError
                return Err(VaultError::InternalError(format!(
                    "Invalid state transition requested for vault {}: from {:?} to {:?}",
                    vault_id,
                    old_status,
                    new_status
                )));
            }
        }

        // If transition is valid, update status and timestamp
        config.status = new_status;
        config.updated_at = time();
        // Update unlocked_at specifically if transitioning to Unlockable
        if new_status == VaultStatus::Unlockable {
            config.unlocked_at = Some(time());
        }

        // Save the updated config
        // Correct match for Option<V> returned by insert
        match borrowed_map.insert(key, Cbor(config)) {
            Some(_) => {
                 ic_cdk::print(format!(
                    "⚙️ INFO: Vault {} status changed from {:?} to {:?}.",
                    vault_id,
                    old_status,
                    new_status
                ));
                Ok(())
            },
            None => {
                // Should not happen, handle defensively
                Err(VaultError::StorageError("Failed to set vault status (vault disappeared?)".to_string()))
            }
        }
    })
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
        let last_active_time = config.updated_at; // Use updated_at as proxy for activity
        let inactivity_duration = current_time.saturating_sub(last_active_time);
        if inactivity_duration.as_secs() >= inactivity_sec {
            ic_cdk::print(format!(
                "✅ UNLOCK CHECK: Vault {} passed inactivity condition.",
                config.vault_id
            ));
            return Ok(true);
        }
    }

    // 3. Approval Threshold
    // TODO: Implement fetching member approval status
    // let approvals = get_approvals(config.vault_id).await?;
    // let required_heirs = conditions.required_heir_approvals.unwrap_or(0);
    // let required_witnesses = conditions.required_witness_approvals.unwrap_or(0);
    // if approvals.heir_count >= required_heirs && approvals.witness_count >= required_witnesses {
    //    ic_cdk::print(format!("Condition Met: Approval threshold ({}/{}) met.", required_heirs, required_witnesses));
    //    return Ok(true);
    // }

    // If none of the conditions are met
    Ok(false)
}

/// Lists all vaults owned by a specific principal.
///
/// # Arguments
/// * `owner` - The PrincipalId of the owner.
///
/// # Returns
/// * `Result<Vec<VaultConfig>, VaultError>` - A list of vault configurations or an error.
pub async fn get_vaults_by_owner(owner: PrincipalId) -> Result<Vec<VaultConfig>, VaultError> {
    let mut owned_vaults = Vec::new();
    // This requires iterating through all vaults, which is inefficient.
    // A secondary index (owner -> vault_id) would be needed for performance.
    storage::VAULT_CONFIGS.with(|map_ref| {
        let map = map_ref.borrow();
        for (_key, value) in map.iter() {
            let config: VaultConfig = value.0;
            if config.owner == owner {
                owned_vaults.push(config);
            }
        }
    });
    // Sort or paginate if needed
    Ok(owned_vaults)
}

/// Lists all vaults a specific principal is a member of (Heir or Witness).
///
/// # Arguments
/// * `member_principal` - The PrincipalId of the member.
///
/// # Returns
/// * `Result<Vec<VaultConfig>, VaultError>` - A list of vault configurations or an error.
pub async fn get_vaults_by_member(member_principal: PrincipalId) -> Result<Vec<VaultConfig>, VaultError> {
    let mut member_vaults = Vec::new();
    let mut vault_ids = std::collections::HashSet::new(); // Avoid duplicates if member of multiple vaults

    // Inefficient: Iterate through all members
    storage::VAULT_MEMBERS.with(|map_ref| {
        let map = map_ref.borrow();
        for (_key, value) in map.iter() {
            let member: VaultMember = value.0;
            if member.principal == member_principal {
                vault_ids.insert(member.vault_id);
            }
        }
    });

    // Fetch config for each unique vault ID
    for vault_id in vault_ids {
        match get_vault_config(&vault_id).await {
            Ok(config) => member_vaults.push(config),
            Err(VaultError::VaultNotFound(_)) => { /* Vault might have been deleted, skip */ },
            Err(e) => return Err(e), // Propagate other errors
        }
    }
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
    storage::VAULT_CONFIGS.with(|map_ref| {
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
/// Requires owner authorization.
///
/// # Arguments
/// * `
// src/backend/services/vault_service.rs
// Placeholder for Vault related business logic 

use crate::{
    error::VaultError,
    models::{
        common::*, // Import common types like VaultId, Timestamp, PrincipalId, VaultStatus
        VaultConfig, // Import the VaultConfig model
        // Add other models as needed, e.g., VaultUpdate payload struct
    },
    storage::{self, Cbor, StorableString}, // Import storage functions/structs
    utils::crypto::generate_ulid, // Import ULID generation
};
use ic_cdk::api::time; // For timestamps

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
    // Add fields for updating unlock conditions, etc.
}

// --- Service Functions ---

/// Creates a new vault with initial configuration.
///
/// # Arguments
/// * `init_data` - Initial data for the vault configuration.
///
/// # Returns
/// * `Result<VaultId, VaultError>` - The ID of the newly created vault or an error.
pub fn create_new_vault(init_data: VaultInitData) -> Result<VaultId, VaultError> {
    let vault_id = generate_ulid();
    let current_time = time();

    // TODO: Calculate expires_at based on plan (e.g., 10 years from current_time)
    // TODO: Determine storage_quota_bytes based on plan
    let expires_at = current_time + (10 * 365 * 24 * 60 * 60 * 1_000_000_000); // Placeholder: 10 years in nanoseconds
    let storage_quota_bytes = match init_data.plan.as_str() {
        "Basic" => 5 * 1024 * 1024,
        "Standard" => 10 * 1024 * 1024,
        "Premium" => 50 * 1024 * 1024,
        "Deluxe" => 100 * 1024 * 1024,
        "Titan" => 250 * 1024 * 1024,
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

        // 3. Apply updates (add more fields as needed)
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
        // TODO: Add logic to update unlock conditions, plan (handle prorate?), etc.

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

/// Changes the status of a vault.
/// Includes basic validation to prevent invalid transitions.
///
/// # Arguments
/// * `vault_id` - The ID of the vault.
/// * `new_status` - The target status.
///
/// # Returns
/// * `Result<(), VaultError>` - Success or an error.
pub fn set_vault_status(vault_id: &VaultId, new_status: VaultStatus) -> Result<(), VaultError> {
    storage::VAULT_CONFIGS.with(|map| {
        let key = StorableString(Cbor(vault_id.clone()));
        let mut borrowed_map = map.borrow_mut();

        let mut config = match borrowed_map.get(&key) {
            Some(storable_config) => storable_config.0,
            None => return Err(VaultError::VaultNotFound(vault_id.clone())),
        };

        let old_status = config.status;

        // TODO: Implement more robust state transition validation based on the lifecycle diagram in prd.md
        // Example basic validation:
        match (old_status, new_status) {
            (VaultStatus::Draft, VaultStatus::NeedSetup) => (), // Valid: Payment confirmed
            (VaultStatus::NeedSetup, VaultStatus::SetupComplete) => (), // Valid: Initial setup done
            (VaultStatus::SetupComplete, VaultStatus::Active) => (), // Valid: Finalized
            (VaultStatus::Active, VaultStatus::GraceMaster) => (), // Valid: Expiry reached
            (VaultStatus::Active, VaultStatus::Unlockable) => (), // Valid: Manual unlock triggered
            // Add other valid transitions
            _ => {
                return Err(VaultError::InternalError(format!(
                    "Invalid status transition from {:?} to {:?}",
                    old_status, new_status
                )))
            }
        }

        config.status = new_status;
        config.updated_at = time();

        // Update unlocked_at timestamp if transitioning to Unlockable
        if new_status == VaultStatus::Unlockable {
            config.unlocked_at = Some(time());
        }

        borrowed_map
            .insert(key, Cbor(config))
            .map_err(|e| VaultError::StorageError(format!("Failed to set vault status: {:?}", e)))?;

        ic_cdk::print(format!(
            "📝 INFO: Vault {} status changed from {:?} to {:?}",
            vault_id, old_status, new_status
        ));
        Ok(())
    })
}

// TODO: Add functions for vault deletion (handle cleanup of members, content, etc.)
// TODO: Add functions to get vaults by owner, etc. (requires secondary indexing or iteration) 
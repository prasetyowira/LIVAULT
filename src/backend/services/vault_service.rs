// src/backend/services/vault_service.rs
// Placeholder for Vault related business logic 

use crate::{
    error::VaultError,
    models::{
        common::*, // Import common types like VaultId, Timestamp, PrincipalId, VaultStatus
        vault_config::{VaultConfig,UnlockConditions}, // Import the VaultConfig model
        vault_member::VaultMember, // Needed for listing vaults by member
        payment::{E8s, PaymentPurpose, PaymentSession, PaymentInitRequest}, // Import Payment related models
        // Add other models as needed, e.g., VaultUpdate payload struct
    },
    utils::crypto::generate_unique_principal, // Import Principal generation
};
use crate::storage;
use ic_cdk::api::{time, caller}; // For timestamps and caller
use std::time::Duration; // For duration calculations
use candid::Principal as PrincipalId; // Explicit import
use crate::services::payment_service; // Import payment_service

// Constants for plan calculations
const TEN_YEARS_IN_NANOS: u64 = 10 * 365 * 24 * 60 * 60 * 1_000_000_000; // Approx 10 years
const E8S_PER_ICP: u64 = 100_000_000;

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

// --- Helper: Get Base Storage Price in ICP e8s --- 
// Based on plans/backend.architecture.md#53-pricing-vs-10-year-cost-projection
fn get_plan_base_price_e8s(plan: &str) -> Result<u64, VaultError> {
    match plan {
        "Basic" => Ok(3_500_000_00),     // 3.5 ICP
        "Standard" => Ok(6_900_000_00),  // 6.9 ICP
        "Premium" => Ok(30_000_000_00), // 30 ICP
        "Deluxe" => Ok(61_000_000_00),  // 61 ICP
        "Titan" => Ok(151_000_000_00), // 151 ICP
        _ => Err(VaultError::InvalidInput(format!(
            "Invalid plan specified for price calculation: {}",
            plan
        ))),
    }
}

// --- Helper: Calculate Prorated Upgrade Cost ---
/// Calculates the prorated cost for upgrading a vault plan.
/// Returns the cost in e8s, or 0 if it's a downgrade or same plan.
fn calculate_prorated_upgrade_cost(
    current_config: &VaultConfig,
    new_plan: &str,
    current_time_ns: Timestamp,
) -> Result<E8s, VaultError> {
    // 1. Check if it's actually an upgrade (higher price)
    let old_price_e8s = get_plan_base_price_e8s(&current_config.plan)?;
    let new_price_e8s = get_plan_base_price_e8s(new_plan)?;

    if new_price_e8s <= old_price_e8s {
        return Ok(0); // Downgrade or same plan - no cost
    }

    // 2. Calculate remaining time factor
    // Vault lifetime is 10 years from creation
    let total_duration_ns = TEN_YEARS_IN_NANOS; 
    let elapsed_time_ns = current_time_ns.saturating_sub(current_config.created_at);
    let remaining_time_ns = total_duration_ns.saturating_sub(elapsed_time_ns);

    // Prevent division by zero or negative time
    if remaining_time_ns == 0 || total_duration_ns == 0 {
        return Ok(0); // No time remaining, no upgrade cost
    }

    // 3. Calculate prorated cost difference
    let price_difference_e8s = new_price_e8s - old_price_e8s;
    
    // Use u128 for intermediate calculation to avoid overflow
    let prorated_cost_e8s = (price_difference_e8s as u128 * remaining_time_ns as u128 / total_duration_ns as u128) as u64;

    Ok(prorated_cost_e8s)
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
    let storage_quota_bytes = get_plan_quota_bytes(&init_data.plan)?;

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
/// If a plan upgrade requires payment, initiates a payment session and returns it.
/// If it's a downgrade or non-plan update, applies changes directly.
///
/// # Arguments
/// * `vault_id` - The ID of the vault to update.
/// * `update_data` - The data containing fields to update.
/// * `caller` - The principal attempting the update (for authorization checks).
///
/// # Returns
/// * `Result<Option<PaymentSession>, VaultError>` - PaymentSession if required, None otherwise, or an error.
pub async fn update_vault_config(
    vault_id: &VaultId,
    update_data: VaultUpdateData,
    caller: PrincipalId,
) -> Result<Option<PaymentSession>, VaultError> {
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

    let current_time = time();
    let mut needs_save = false;
    let mut payment_session_needed: Option<PaymentSession> = None;

    // --- Apply non-plan updates directly ---
    if let Some(name) = update_data.name {
        if config.name != name {
            config.name = name;
            needs_save = true;
        }
    }
    if update_data.description != config.description {
        config.description = update_data.description;
        needs_save = true;
    }
    if let Some(unlock_conditions) = update_data.unlock_conditions {
        if config.unlock_conditions != unlock_conditions {
            config.unlock_conditions = unlock_conditions;
            needs_save = true;
        }
    }

    // --- Handle Plan Change ---
    if let Some(new_plan) = update_data.plan {
        if config.plan != new_plan {
            // Calculate potential upgrade cost
            let upgrade_cost_e8s = calculate_prorated_upgrade_cost(&config, &new_plan, current_time)?;

            if upgrade_cost_e8s > 0 {
                // --- Upgrade requires payment --- 
                ic_cdk::print(format!(
                    "INFO: Vault {} upgrade from {} to {} requires payment of {} e8s.",
                    vault_id, config.plan, new_plan, upgrade_cost_e8s
                ));

                let payment_req = PaymentInitRequest {
                    vault_plan: new_plan.clone(), // The target plan
                    amount_e8s: upgrade_cost_e8s,
                };
                let purpose = PaymentPurpose::PlanUpgrade { new_plan: new_plan.clone() };

                // Initiate payment session
                let session = payment_service::initialize_payment_session(payment_req, caller, Some(purpose)).await?;
                payment_session_needed = Some(session);
                // DO NOT apply the plan change to config yet. It happens after payment verification.
                needs_save = false; // No immediate save needed if payment is pending

            } else {
                // --- Downgrade or same plan - apply directly --- 
                ic_cdk::print(format!(
                    "INFO: Vault {} changing plan from {} to {} (downgrade/same). Applying directly.",
                    vault_id, config.plan, new_plan
                ));
                let new_storage_quota_bytes = get_plan_quota_bytes(&new_plan)?;
                
                // Check if new quota is sufficient for current usage
                if new_storage_quota_bytes < config.storage_used_bytes {
                    return Err(VaultError::StorageError(
                        "New plan quota is less than current usage.".to_string(),
                    ));
                }
                config.plan = new_plan;
                config.storage_quota_bytes = new_storage_quota_bytes;
                needs_save = true; // Apply change now
            }
        }
    }

    // 4. If changes applied directly (no payment needed or non-plan changes), save.
    if needs_save {
        config.updated_at = current_time;
        match storage::vault_configs::insert_vault_config(&config) {
            Some(_) => {
                ic_cdk::print(format!(
                    "📝 INFO: Vault {} config updated by owner {}. Payment required: {}",
                    vault_id, caller, payment_session_needed.is_some()
                ));
            }
            None => {
                ic_cdk::eprintln!(
                    "⚠️ WARNING: Vault {} config insertion reported None during update.",
                    vault_id
                );
            }
        }
    }

    // Return the payment session if one was created, otherwise None
    Ok(payment_session_needed)
}

/// Helper to get quota bytes for a plan string.
fn get_plan_quota_bytes(plan: &str) -> Result<u64, VaultError> {
    match plan {
        "Basic" => Ok(5 * 1024 * 1024),         // 5 MB
        "Standard" => Ok(10 * 1024 * 1024),     // 10 MB
        "Premium" => Ok(50 * 1024 * 1024),      // 50 MB
        "Deluxe" => Ok(100 * 1024 * 1024),    // 100 MB
        "Titan" => Ok(250 * 1024 * 1024),       // 250 MB
        _ => Err(VaultError::InvalidInput(format!(
            "Invalid plan specified for quota calculation: {}",
            plan
        ))),
    }
}

/// Internal function to apply a plan change after successful payment verification.
/// Should only be called by the payment service.
pub async fn finalize_plan_change(vault_id: &VaultId, new_plan: String) -> Result<(), VaultError> {
    ic_cdk::print(format!(
        "INFO: Finalizing plan change for vault {} to {}",
        vault_id, new_plan
    ));
    let mut config = storage::vault_configs::get_vault_config(vault_id)
        .ok_or_else(|| VaultError::VaultNotFound(vault_id.clone().to_string()))?;

    // Get new quota
    let new_storage_quota_bytes = get_plan_quota_bytes(&new_plan)?;

    // Check quota again (unlikely to change, but good practice)
    if new_storage_quota_bytes < config.storage_used_bytes {
        ic_cdk::eprintln!(
            "ERROR: Cannot finalize plan change for vault {}. New quota {} is less than current usage {}.",
            vault_id, new_storage_quota_bytes, config.storage_used_bytes
        );
        // Don't return error here? Payment already made. Log and maybe flag vault?
        // For now, log the error but proceed with plan name change.
        // return Err(VaultError::StorageError("New plan quota is less than current usage.".to_string()));
    }

    config.plan = new_plan.clone();
    config.storage_quota_bytes = new_storage_quota_bytes;
    config.updated_at = time();

    match storage::vault_configs::insert_vault_config(&config) {
        Some(_) => {
            ic_cdk::print(format!(
                "✅ SUCCESS: Vault {} plan finalized to {}.",
                vault_id, new_plan
            ));
            Ok(())
        }
        None => {
            ic_cdk::eprintln!(
                "❌ ERROR: Failed to save finalized plan change for vault {} (config disappeared?).",
                vault_id
            );
            Err(VaultError::StorageError(
                "Failed to save finalized plan change.".to_string(),
            ))
        }
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

    // --- State Transition Validation (Based on plans/readme.md Lifecycle) ---
    let is_valid_transition = match (old_status, new_status) {
        // Initial Setup Flow
        (VaultStatus::Draft, VaultStatus::NeedSetup) => true, // After payment verification
        (VaultStatus::NeedSetup, VaultStatus::SetupComplete) => true, // After owner finishes setup (config + invite sent)
        (VaultStatus::SetupComplete, VaultStatus::Active) => true, // After >= 1 heir joined

        // Active State Transitions
        (VaultStatus::Active, VaultStatus::Active) => true, // Allow updates while active
        (VaultStatus::Active, VaultStatus::GraceMaster) => true, // Plan expires

        // Grace Master Flow
        (VaultStatus::GraceMaster, VaultStatus::Active) => true, // Plan renewed
        (VaultStatus::GraceMaster, VaultStatus::GraceHeir) => true, // 14 days passed without owner action

        // Grace Heir Flow
        (VaultStatus::GraceHeir, VaultStatus::Active) => true, // Plan renewed during heir grace
        (VaultStatus::GraceHeir, VaultStatus::Unlockable) => true, // Quorum met or QR used
        (VaultStatus::GraceHeir, VaultStatus::Expired) => true, // 14 days passed without quorum/renewal

        // Unlockable Flow
        (VaultStatus::Unlockable, VaultStatus::Unlocked) => true, // After vault explicitly unlocked by heir/witness action
        (VaultStatus::Unlockable, VaultStatus::Expired) => true, // Optional: Auto-expire if not unlocked within a timeframe (e.g., 1 year)

        // Unlocked Flow
        (VaultStatus::Unlocked, VaultStatus::Expired) => true, // After max plan duration expired or specific unlock access window closes

        // Expiry and Deletion
        (VaultStatus::Expired, VaultStatus::Deleted) => true, // Admin/cron cleanup
        (_, VaultStatus::Deleted) => true, // Allow deletion from almost any state (admin override)

        // Self-loops are allowed
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

        // Reinstate logic for unlocked_at
        if new_status == VaultStatus::Unlocked {
            config.unlocked_at = Some(time());
        }
        // Reset unlocked_at if moving *out* of Unlocked state
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
    let is_authorized = storage::members::is_member_with_role(vault_id, &caller, Role::Witness).await?
                       || storage::config::get_admin_principal().await? == caller; // Allow admin trigger?

    if !is_authorized {
         return Err(VaultError::NotAuthorized("Only a witness or admin can trigger unlock".to_string()));
    }

    // Check if vault is in a state where unlock can be triggered (GraceHeir as per diagram, or Active if conditions met early?)
    // Let's allow trigger from Active or GraceHeir, check_unlock_conditions will validate timing/inactivity etc.
    if !matches!(config.status, VaultStatus::Active | VaultStatus::GraceHeir) {
        return Err(VaultError::InvalidState(format!(
            "Cannot trigger unlock from status {:?}. Expected Active or GraceHeir.",
            config.status
        )));
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

/// Checks if unlock conditions are met for a vault.
/// Returns true if *any* of the configured conditions are satisfied.
async fn check_unlock_conditions(config: &VaultConfig) -> Result<bool, VaultError> {
    let current_time_ns = time(); // Use nanoseconds for internal checks
    let conditions = &config.unlock_conditions;
    let vault_id = &config.vault_id;

    ic_cdk::print(format!("🔍 UNLOCK CHECK: Vault {}. Current time: {}", vault_id, current_time_ns));
    ic_cdk::print(format!("🔍 UNLOCK CHECK: Conditions: {:?}", conditions));

    // 1. Time-based unlock check
    if let Some(unlock_time_sec) = conditions.time_based_unlock_epoch_sec {
        // Convert unlock_time_sec (epoch seconds) to nanoseconds
        let unlock_time_ns = (unlock_time_sec as u128) * 1_000_000_000;
        ic_cdk::print(format!("🔍 UNLOCK CHECK: Time-based: {} >= {}?", current_time_ns as u128, unlock_time_ns));
        if current_time_ns as u128 >= unlock_time_ns {
            ic_cdk::print(format!(
                "✅ UNLOCK CHECK: Vault {} passed time-based condition.",
                vault_id
            ));
            return Ok(true);
        }
    }

    // 2. Inactivity-based unlock check
    if let Some(inactivity_sec) = conditions.inactivity_duration_sec {
        let last_active_time_ns = config.last_accessed_by_owner.unwrap_or(config.created_at); // Use created_at if never accessed
        let inactivity_duration_ns = current_time_ns.saturating_sub(last_active_time_ns);
        let required_inactivity_nanos = (inactivity_sec as u128) * 1_000_000_000;
        ic_cdk::print(format!("🔍 UNLOCK CHECK: Inactivity-based: Last active {}, Duration {} >= Required {}?",
            last_active_time_ns, inactivity_duration_ns as u128, required_inactivity_nanos));

        if inactivity_duration_ns as u128 >= required_inactivity_nanos {
            ic_cdk::print(format!(
                "✅ UNLOCK CHECK: Vault {} passed inactivity condition ({}s).",
                vault_id, inactivity_sec
            ));
            return Ok(true);
        }
    }

    // 3. Approval Threshold check
    let required_heirs = conditions.required_heir_approvals.unwrap_or(0);
    let required_witnesses = conditions.required_witness_approvals.unwrap_or(0);

    ic_cdk::print(format!("🔍 UNLOCK CHECK: Approval-based: Heirs {}/{}, Witnesses {}/{} required.",
        0, // Placeholder, actual count fetched below
        required_heirs,
        0, // Placeholder
        required_witnesses
    ));

    if required_heirs > 0 || required_witnesses > 0 {
        // Assume storage::approvals::get_approval_status exists and returns counts
        match storage::approvals::get_approval_status(vault_id).await {
            Ok(approvals) => {
                 ic_cdk::print(format!("🔍 UNLOCK CHECK: Fetched approvals: Heirs {}, Witnesses {}.",
                    approvals.heir_approvals, approvals.witness_approvals));
                 if approvals.heir_approvals >= required_heirs && approvals.witness_approvals >= required_witnesses {
                    ic_cdk::print(format!(
                        "✅ UNLOCK CHECK: Vault {} passed approval threshold (Heirs: {}/{}, Witnesses: {}/{}).",
                        vault_id,
                        approvals.heir_approvals, required_heirs,
                        approvals.witness_approvals, required_witnesses
                    ));
                    return Ok(true); // Approvals met
                } else {
                     ic_cdk::print(format!(
                        "⏳ UNLOCK CHECK: Vault {} pending approvals (Heirs: {}/{}, Witnesses: {}/{}).",
                        vault_id,
                        approvals.heir_approvals, required_heirs,
                        approvals.witness_approvals, required_witnesses
                    ));
                    // Continue checking other conditions
                }
            }
            Err(e) => {
                // Log error fetching approvals, but treat as condition not met for safety.
                 ic_cdk::eprintln!(
                    "❌ ERROR: Failed to get approval status for vault {}: {:?}. Treating approval condition as NOT MET.",
                    vault_id, e
                 );
                 // Do not return error here, just log and continue checking other conditions.
                 // return Err(VaultError::StorageError(format!("Failed to check approvals: {}", e)));
            }
        }
    } else {
         ic_cdk::print("🔍 UNLOCK CHECK: Approval condition not configured (0 heirs/witnesses required).");
    }

    // If none of the conditions were met after checking all configured ones
    ic_cdk::print(format!("⏳ UNLOCK CHECK: Vault {} - NO conditions met.", vault_id));
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

    // 1. Authorization Check: Ensure caller is the owner or admin
    let is_admin = storage::config::get_admin_principal().await? == caller;
    if config.owner != caller && !is_admin {
        return Err(VaultError::NotAuthorized(format!(
            "Caller {} is not the owner or admin of vault {}",
            caller, vault_id
        )));
    }

    // 2. Status Check: Allow deletion from Expired or potentially other states if admin.
    // For now, let's restrict non-admins to Expired or Deleted (for idempotency).
    if !is_admin && !matches!(config.status, VaultStatus::Expired | VaultStatus::Deleted) {
        return Err(VaultError::InvalidState(format!(
            "Vault {} cannot be deleted by owner from status {:?}. Must be Expired.",
            vault_id, config.status
        )));
    }

    // If already Deleted, return Ok for idempotency
    if config.status == VaultStatus::Deleted {
        ic_cdk::print(format!("ℹ️ INFO: Vault {} is already marked as Deleted.", vault_id));
        return Ok(());
    }

    let trigger_info = if is_admin { "admin" } else { "owner" };
    ic_cdk::print(format!("🗑️ INFO: Initiating deletion for vault {} by {}", vault_id, trigger_info));

    // --- Cleanup Steps (Placeholders - Require Implementation) ---

    // Remove members
    match storage::members::remove_members_by_vault(vault_id).await {
        Ok(count) => ic_cdk::print(format!("🗑️ INFO: Removed {} members for vault {}", count, vault_id)),
        Err(e) => ic_cdk::eprintln!("❌ ERROR: Failed removing members for vault {}: {:?}", vault_id, e), // Log error, continue deletion
    }

    // Remove content metadata (chunk removal assumed handled elsewhere or not needed)
    match storage::content::remove_all_content_for_vault(vault_id).await {
         Ok(count) => ic_cdk::print(format!("🗑️ INFO: Removed {} content items for vault {}", count, vault_id)),
         Err(e) => ic_cdk::eprintln!("❌ ERROR: Failed removing content for vault {}: {:?}", vault_id, e), // Log error, continue deletion
    }
    // Remove content index
    match storage::content_index::remove_index(vault_id).await {
        Ok(_) => ic_cdk::print(format!("🗑️ INFO: Removed content index for vault {}", vault_id)),
        Err(e) => ic_cdk::eprintln!("❌ ERROR: Failed removing content index for vault {}: {:?}", vault_id, e),
    }

    // Remove invite tokens
    match storage::tokens::remove_tokens_by_vault(vault_id).await {
        Ok(count) => ic_cdk::print(format!("🗑️ INFO: Removed {} tokens for vault {}", count, vault_id)),
        Err(e) => ic_cdk::eprintln!("❌ ERROR: Failed removing tokens for vault {}: {:?}", vault_id, e), // Log error, continue deletion
    }

    // Remove audit logs
    match storage::audit_logs::remove_audit_logs(vault_id).await {
        Ok(_) => ic_cdk::print(format!("🗑️ INFO: Removed audit logs for vault {}", vault_id)),
        Err(e) => ic_cdk::eprintln!("❌ ERROR: Failed removing audit logs for vault {}: {:?}", vault_id, e), // Log error, continue deletion
    }

    // Remove approvals
    match storage::approvals::remove_approvals(vault_id).await {
         Ok(_) => ic_cdk::print(format!("🗑️ INFO: Removed approvals for vault {}", vault_id)),
         Err(e) => ic_cdk::eprintln!("❌ ERROR: Failed removing approvals for vault {}: {:?}", vault_id, e), // Log error, continue deletion
    }

    // --- Final Step: Remove Vault Config & Update Metrics ---
    match storage::vault_configs::remove_vault_config(vault_id).await {
        Ok(Some(_)) => {
            ic_cdk::print(format!("✅ SUCCESS: Vault {} configuration removed.", vault_id));
            // Update metrics
            match storage::metrics::decrement_vault_count().await {
                Ok(_) => ic_cdk::print(format!("📊 INFO: Decremented vault count metric.")),
                Err(e) => ic_cdk::eprintln!("❌ ERROR: Failed decrementing vault count metric: {:?}", e),
            }
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

/// Internal helper function to update the storage usage for a vault.
/// Checks against the quota.
async fn update_storage_usage(vault_id: &VaultId, delta_bytes: i64) -> Result<(), VaultError> {
    let mut config = storage::vault_configs::get_vault_config(vault_id)
        .ok_or_else(|| VaultError::VaultNotFound(vault_id.clone().to_string()))?;

    let mut new_usage = config.storage_used_bytes as i64 + delta_bytes;

    // Ensure usage doesn't go below zero (e.g., if multiple deletions happen concurrently)
    if new_usage < 0 {
        ic_cdk::eprintln!("⚠️ WARNING: Storage usage calculation for vault {} resulted in negative value. Setting to 0.", vault_id);
        new_usage = 0;
    }

    let new_usage_u64 = new_usage as u64;

    // Check against quota if adding bytes
    if delta_bytes > 0 && new_usage_u64 > config.storage_quota_bytes {
        return Err(VaultError::StorageQuotaExceeded(
            vault_id.clone().to_string(),
            config.storage_quota_bytes,
            config.storage_used_bytes, // Report usage *before* the attempted addition
            delta_bytes as u64,
        ));
    }

    // Update config if usage changed
    if config.storage_used_bytes != new_usage_u64 {
        config.storage_used_bytes = new_usage_u64;
        config.updated_at = time(); // Also update the vault's general updated_at timestamp

        match storage::vault_configs::insert_vault_config(&config) {
            Some(_) => {
                ic_cdk::print(format!(
                    "💾 INFO: Updated storage usage for vault {} to {} bytes (delta: {}).",
                    vault_id, new_usage_u64, delta_bytes
                ));
            }
            None => {
                ic_cdk::eprintln!(
                    "❌ ERROR: Failed to save updated storage usage for vault {} (config disappeared?).",
                    vault_id
                );
                return Err(VaultError::StorageError(
                    "Failed to save updated storage usage.".to_string(),
                ));
            }
        }
    }

    Ok(())
}
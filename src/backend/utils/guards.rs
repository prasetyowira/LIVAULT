// src/backend/utils/guards.rs
use crate::{
    error::VaultError,
    models::{
        vault_config::VaultConfig,
        vault_member::{VaultMember, MemberRole},
        common::{VaultId, PrincipalId},
    },
    storage::{self, Cbor},
};
use ic_cdk::api::{caller as ic_caller, canister_balance128};
use candid::Principal;

// TODO: Make this configurable via init args or stable storage
const MIN_CYCLES_THRESHOLD: u128 = 10_000_000_000; // Example: 10B cycles

/// Checks if the canister has sufficient cycles.
///
/// # Errors
///
/// Returns `VaultError::CycleLow` if the balance is below the threshold.
pub fn check_cycles() -> Result<(), VaultError> {
    let balance = canister_balance128();
    if balance < MIN_CYCLES_THRESHOLD {
        ic_cdk::println!(
            "Cycle balance low: {} cycles, threshold: {}",
            balance,
            MIN_CYCLES_THRESHOLD
        );
        Err(VaultError::CycleLow(balance))
    } else {
        Ok(())
    }
}

/// Checks if the caller is the designated admin principal.
///
/// # Errors
///
/// Returns `VaultError::AdminGuardFailed` if the caller is not the admin.
pub fn check_admin(admin_principal: Principal) -> Result<(), VaultError> {
    let caller = ic_caller();
    if caller == admin_principal {
        Ok(())
    } else {
        Err(VaultError::AdminGuardFailed)
    }
}

// --- Configuration --- //
// TODO: Load these from stable storage or init args
thread_local! {
    // Replace with actual admin principal loading
    static ADMIN_PRINCIPAL: Principal = Principal::management_canister();
    // Replace with actual cron principal loading
    static CRON_PRINCIPAL: Principal = Principal::management_canister();
}

fn get_admin_principal() -> Principal { ADMIN_PRINCIPAL.with(|p| *p) }
fn get_cron_principal() -> Principal { CRON_PRINCIPAL.with(|p| *p) }

// --- Guard Implementations --- //

/// Guard: Allow only the admin principal.
pub fn admin_guard() -> Result<(), String> {
    let caller = ic_caller();
    let admin = get_admin_principal();
    if caller == admin {
        Ok(())
    } else {
        Err(format!("Caller {} is not the designated admin {}", caller, admin))
    }
}

/// Guard: Allow only the cron principal OR the admin principal.
pub fn cron_or_admin_guard() -> Result<(), String> {
    let caller = ic_caller();
    let admin = get_admin_principal();
    let cron = get_cron_principal();
    if caller == admin || caller == cron {
        Ok(())
    } else {
        Err(format!("Caller {} is not the admin {} or cron {}", caller, admin, cron))
    }
}

/// Guard: Check if caller is the owner of the specified vault.
/// NOTE: This requires fetching vault config, making it potentially expensive.
/// Consider alternative designs if performance is critical.
pub fn owner_guard(vault_id: VaultId) -> Result<(), String> {
    let caller = ic_caller();
    let vault_key = Cbor(vault_id.clone());
    storage::VAULT_CONFIGS.with(|map_ref| {
        let map = map_ref.borrow();
        match map.get(&vault_key) {
            Some(config_cbor) => {
                let config: VaultConfig = config_cbor.0;
                if config.owner == caller {
                    Ok(())
                } else {
                    Err(format!("Caller {} is not the owner of vault {}", caller, vault_id))
                }
            }
            None => Err(format!("Vault {} not found for owner check", vault_id)),
        }
    })
}

/// Placeholder Guard: Checks if the caller is either the owner or a designated heir.
/// TODO: Implement proper heir check based on VaultMember roles.
pub fn owner_or_heir_guard(vault_id: VaultId) -> Result<(), String> {
    let caller = ic_caller();
    let vault_key = Cbor(vault_id.clone());
    storage::VAULT_CONFIGS.with(|map_ref| {
        let map = map_ref.borrow();
        match map.get(&vault_key) {
            Some(config_cbor) => {
                let config: VaultConfig = config_cbor.0;
                if config.owner == caller {
                    return Ok(());
                }
                // TODO: Add heir check here by iterating members or using an index
                Err(format!("Caller {} is not the owner or a verified heir of vault {} (Heir check TODO)", caller, vault_id))
            }
            None => Err(format!("Vault {} not found for owner/heir check", vault_id)),
        }
    })
}

/// Placeholder Guard: Checks if the caller is a member (any role) or an heir.
/// TODO: Implement proper role checks.
pub fn member_or_heir_guard(vault_id: VaultId) -> Result<(), String> {
    let _caller = ic_caller(); // TODO: Use caller
    // TODO: Implement member check (similar to owner_or_heir_guard but checking for any member or specific roles)
    Err(format!("member_or_heir_guard not fully implemented for vault {}", vault_id))
}

/// Placeholder Guard: Checks if the caller is the specified member principal or the vault owner.
pub fn self_or_owner_guard(vault_id: VaultId, member_principal: PrincipalId) -> Result<(), String> {
     let caller = ic_caller();
    let vault_key = Cbor(vault_id.clone());
    storage::VAULT_CONFIGS.with(|map_ref| {
        let map = map_ref.borrow();
        match map.get(&vault_key) {
            Some(config_cbor) => {
                let config: VaultConfig = config_cbor.0;
                if config.owner == caller || member_principal == caller {
                    Ok(())
                } else {
                     Err(format!("Caller {} is not the owner or the specified principal {} for vault {}", caller, member_principal, vault_id))
                }
            }
             None => Err(format!("Vault {} not found for self/owner check", vault_id)),
        }
    })
} 
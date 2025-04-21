// src/backend/utils/guards.rs
use crate::{
    error::VaultError,
    models::{
        vault_config::VaultConfig,
        vault_member::{VaultMember, MemberStatus},
        common::{VaultId, PrincipalId, Role},
    },
    storage::{self, config as storage_config, Cbor},
};
use ic_cdk::api::{caller as ic_caller, canister_balance128};
use candid::Principal;

/// Checks if the canister has sufficient cycles.
///
/// # Errors
///
/// Returns `VaultError::CycleLow` if the balance is below the threshold.
pub fn check_cycles() -> Result<(), VaultError> {
    let threshold = storage_config::get_min_cycles_threshold();
    let balance = canister_balance128();
    if balance < threshold {
        ic_cdk::println!(
            "Cycle balance low: {} cycles, threshold: {}",
            balance,
            threshold
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

// --- Guard Implementations --- //

/// Guard: Allow only the admin principal.
pub fn admin_guard() -> Result<(), String> {
    let caller = ic_caller();
    let admin = storage_config::get_admin_principal();
    if caller == admin {
        Ok(())
    } else {
        Err(format!("Caller {} is not the designated admin {}", caller, admin))
    }
}

/// Guard: Allow only the cron principal OR the admin principal.
pub fn cron_or_admin_guard() -> Result<(), String> {
    let caller = ic_caller();
    let admin = storage_config::get_admin_principal();
    let cron = storage_config::get_cron_principal();
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

/// Guard: Checks if the caller is either the owner or a *verified* designated heir.
pub fn owner_or_heir_guard(vault_id: VaultId) -> Result<(), String> {
    let caller = ic_caller();
    let vault_key = Cbor(vault_id.clone());

    // 1. Check if caller is the owner
    match storage::VAULT_CONFIGS.with(|map_ref| {
        let map = map_ref.borrow();
        map.get(&vault_key).map(|c| c.0)
    }) {
        Some(config) => {
            if config.owner == caller {
                return Ok(());
            }
            // If not owner, proceed to check if they are a verified heir
        }
        None => return Err(format!("Vault {} not found for owner/heir check", vault_id)),
    }

    // 2. Check if caller is a verified heir
    let is_verified_heir = storage::MEMBERS.with(|map_ref| {
        let map = map_ref.borrow();
        // Define the range for the specific vault_id
        let range_start = (vault_id.clone(), Principal::min_id());
        let range_end = (vault_id.clone(), Principal::max_id());

        for ((_v_id, principal), member_cbor) in map.range(range_start..=range_end) {
            if principal == caller {
                let member: VaultMember = member_cbor.0.clone();
                if member.role == Role::Heir && member.status == MemberStatus::Verified {
                    return true; // Found caller as a verified heir
                }
                // Since keys are unique, no need to check further for this caller
                break;
            }
        }
        false // Caller is not a verified heir for this vault
    });

    if is_verified_heir {
        Ok(())
    } else {
        Err(format!(
            "Caller {} is not the owner or a verified heir of vault {}",
            caller,
            vault_id
        ))
    }
}

/// Guard: Checks if the caller is a member (any role) of the specified vault.
pub fn member_guard(vault_id: VaultId) -> Result<(), String> {
    let caller = ic_caller();

    let is_member = storage::MEMBERS.with(|map_ref| {
        let map = map_ref.borrow();
        // Direct lookup using the composite key
        let key = (vault_id.clone(), caller);
        map.contains_key(&key)
    });

    if is_member {
        Ok(())
    } else {
        Err(format!(
            "Caller {} is not a member of vault {}",
            caller,
            vault_id
        ))
    }
}

/// Guard: Checks if the caller is a member with the specified role in the vault.
pub fn role_guard(vault_id: VaultId, required_role: Role) -> Result<(), String> {
    let caller = ic_caller();

    let has_role = storage::MEMBERS.with(|map_ref| {
        let map = map_ref.borrow();
        // Direct lookup using the composite key
        let key = (vault_id.clone(), caller);
        match map.get(&key) {
            Some(member_cbor) => {
                let member: VaultMember = member_cbor.0;
                member.role == required_role // && member.status == MemberStatus::Verified // Optional status check
            }
            None => false, // Caller is not a member of this vault
        }
    });

    if has_role {
        Ok(())
    } else {
        Err(format!(
            "Caller {} does not have the required role '{:?}' in vault {}",
            caller,
            required_role,
            vault_id
        ))
    }
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
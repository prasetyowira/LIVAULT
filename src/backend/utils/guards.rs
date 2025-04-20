// src/backend/utils/guards.rs
use crate::error::VaultError;
use ic_cdk::api::canister_balance128;

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
        Err(VaultError::CycleLow)
    } else {
        Ok(())
    }
}

/// Checks if the caller is the designated admin principal.
///
/// # Errors
///
/// Returns `VaultError::AdminGuardFailed` if the caller is not the admin.
pub fn check_admin(admin_principal: candid::Principal) -> Result<(), VaultError> {
    let caller = ic_cdk::caller();
    if caller == admin_principal {
        Ok(())
    } else {
        Err(VaultError::AdminGuardFailed)
    }
} 
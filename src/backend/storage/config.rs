// src/backend/storage/config.rs
use crate::storage::memory::{get_memory, Memory};
use crate::storage::storable::Cbor; // Assuming Principal uses Cbor
use candid::Principal;
use ic_stable_structures::{MemoryId, StableCell};
use std::cell::RefCell;

// Define Memory IDs for config cells (ensure these are unique)
const ADMIN_PRINCIPAL_MEM_ID: MemoryId = MemoryId::new(25);
const CRON_PRINCIPAL_MEM_ID: MemoryId = MemoryId::new(26);
const MIN_CYCLES_THRESHOLD_MEM_ID: MemoryId = MemoryId::new(27);

// Default values (used if init fails or cell is uninitialized)
const DEFAULT_ADMIN_PRINCIPAL: Principal = Principal::management_canister();
const DEFAULT_CRON_PRINCIPAL: Principal = Principal::management_canister();
const DEFAULT_MIN_CYCLES_THRESHOLD: u128 = 10_000_000_000; // 10B cycles

thread_local! {
    /// Stable cell for the Admin Principal
    static ADMIN_PRINCIPAL: RefCell<StableCell<Cbor<Principal>, Memory>> = RefCell::new(
        StableCell::init(get_memory(ADMIN_PRINCIPAL_MEM_ID), Cbor(DEFAULT_ADMIN_PRINCIPAL))
            .expect("Failed to initialize admin principal stable cell")
    );

    /// Stable cell for the Cron Principal
    static CRON_PRINCIPAL: RefCell<StableCell<Cbor<Principal>, Memory>> = RefCell::new(
        StableCell::init(get_memory(CRON_PRINCIPAL_MEM_ID), Cbor(DEFAULT_CRON_PRINCIPAL))
            .expect("Failed to initialize cron principal stable cell")
    );

    /// Stable cell for the Minimum Cycles Threshold
    static MIN_CYCLES_THRESHOLD: RefCell<StableCell<u128, Memory>> = RefCell::new(
        StableCell::init(get_memory(MIN_CYCLES_THRESHOLD_MEM_ID), DEFAULT_MIN_CYCLES_THRESHOLD)
            .expect("Failed to initialize min cycles threshold stable cell")
    );
}

/// Initialize the configuration values from InitArgs.
/// Should be called only during canister initialization or upgrade.
pub fn init_config(admin: Principal, cron: Principal, threshold: u128) {
    ADMIN_PRINCIPAL.with(|cell| {
        cell.borrow_mut()
            .set(Cbor(admin))
            .expect("Failed to set admin principal");
    });
    CRON_PRINCIPAL.with(|cell| {
        cell.borrow_mut()
            .set(Cbor(cron))
            .expect("Failed to set cron principal");
    });
    MIN_CYCLES_THRESHOLD.with(|cell| {
        cell.borrow_mut()
            .set(threshold)
            .expect("Failed to set min cycles threshold");
    });
    ic_cdk::println!("Configuration initialized: Admin={}, Cron={}, Threshold={}", admin, cron, threshold);
}

/// Get the configured Admin Principal.
pub fn get_admin_principal() -> Principal {
    ADMIN_PRINCIPAL.with(|cell| cell.borrow().get().0)
}

/// Get the configured Cron Principal.
pub fn get_cron_principal() -> Principal {
    CRON_PRINCIPAL.with(|cell| cell.borrow().get().0)
}

/// Get the configured Minimum Cycles Threshold.
pub fn get_min_cycles_threshold() -> u128 {
    MIN_CYCLES_THRESHOLD.with(|cell| *cell.borrow().get())
} 
// src/backend/metrics.rs
use candid::{CandidType, Nat};
use serde::{Deserialize, Serialize};
use ic_stable_structures::{Storable, Bound};
use std::borrow::Cow;

#[derive(CandidType, Deserialize, Serialize, Default, Clone, Debug)]
pub struct VaultMetrics {
    pub total_vaults: Nat,
    pub active_vaults: Nat,
    pub unlocked_vaults: Nat,
    pub need_setup_vaults: Nat,
    pub expired_vaults: Nat,
    pub storage_used_bytes: Nat, // Use Nat for potentially large numbers
    // cycle_balance_t: Nat,       // Fetched dynamically, not stored
    // cycles_burn_per_day: Nat, // Calculated dynamically or by scheduler, not stored
    pub invites_today: Nat,       // Consider if this needs daily reset via scheduler
    pub invites_claimed: Nat,
    // unlock_avg_months: f64,  // Requires storing unlock dates, maybe defer post-MVP?
    pub scheduler_last_run: u64, // epoch sec
}

impl VaultMetrics {
    // Example update functions (implement more as needed)
    pub fn increment_total_vaults(&mut self) {
        self.total_vaults += Nat::from(1u32);
    }

    pub fn increment_active_vaults(&mut self) {
        self.active_vaults += Nat::from(1u32);
    }

    pub fn update_storage_used(&mut self, delta_bytes: i128) {
        let current_bytes = self.storage_used_bytes.0.to_u128().unwrap_or(0);
        if delta_bytes >= 0 {
            self.storage_used_bytes = Nat::from(current_bytes.saturating_add(delta_bytes as u128));
        } else {
            self.storage_used_bytes = Nat::from(current_bytes.saturating_sub(delta_bytes.abs() as u128));
        }
    }

    pub fn set_scheduler_last_run(&mut self, timestamp: u64) {
        self.scheduler_last_run = timestamp;
    }
}

// Implement Storable for VaultMetrics to use with StableCell
impl Storable for VaultMetrics {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(ciborium::into_writer(&self, Vec::new()).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        ciborium::from_reader(bytes.as_ref()).unwrap()
    }

    const BOUND: Bound = Bound::Unbounded;
} 
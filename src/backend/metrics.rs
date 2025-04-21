// src/backend/metrics.rs
use crate::models::common::{VaultStatus, ContentType};
use crate::storage::{get_metrics, update_metrics, Cbor};
use crate::error::VaultError;
use candid::{CandidType, Deserialize, Nat};
use serde::Serialize;
use ic_stable_structures::{Storable, Bound};
use std::borrow::Cow;
use num::{BigUint, ToPrimitive};

// Define the VaultMetrics struct based on backend.architecture.md
#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct VaultMetrics {
    pub total_vaults: u32,
    pub active_vaults: u32,
    pub unlocked_vaults: u32,
    pub need_setup_vaults: u32,
    pub expired_vaults: u32,
    pub storage_used_bytes: Cbor<BigUint>, // Use BigUint for potentially large numbers
    pub invites_sent_total: u64,
    pub invites_claimed_total: u64,
    pub unlock_triggers_total: u64,
    // Add other relevant metrics as needed
    pub scheduler_last_run_success: Option<u64>, // Timestamp of last successful run
}

// Implement Default for easy initialization
impl Default for VaultMetrics {
    fn default() -> Self {
        Self {
            total_vaults: 0,
            active_vaults: 0,
            unlocked_vaults: 0,
            need_setup_vaults: 0,
            expired_vaults: 0,
            storage_used_bytes: Cbor(BigUint::from(0u64)),
            invites_sent_total: 0,
            invites_claimed_total: 0,
            unlock_triggers_total: 0,
            scheduler_last_run_success: None,
        }
    }
}

// --- Metrics Update Helpers ---

// Example: Increment vault count based on status
pub fn increment_vault_count(status: VaultStatus) -> Result<(), String> {
    update_metrics(|metrics| {
        metrics.total_vaults = metrics.total_vaults.saturating_add(1);
        match status {
            VaultStatus::Active => metrics.active_vaults = metrics.active_vaults.saturating_add(1),
            VaultStatus::NeedSetup => metrics.need_setup_vaults = metrics.need_setup_vaults.saturating_add(1),
            _ => {} // Handle other statuses if needed for counts
        }
    })
}

// Example: Update storage usage (ensure atomicity if possible)
pub fn update_storage_usage(bytes_delta: i64) -> Result<(), String> {
    update_metrics(|metrics| {
        let current_bytes = metrics.storage_used_bytes.0.to_u128().unwrap_or(0);
        let new_bytes = if bytes_delta >= 0 {
            current_bytes.saturating_add(bytes_delta as u128)
        } else {
            current_bytes.saturating_sub(bytes_delta.abs() as u128)
        };
        metrics.storage_used_bytes = Cbor(BigUint::from(new_bytes));
    })
}

// Implement Storable for VaultMetrics using CBOR
impl Storable for VaultMetrics {
    fn to_bytes(&self) -> Cow<[u8]> {
        // Create a Vec<u8> writer, write into it, and return the owned Vec
        let mut writer = Vec::new();
        ciborium::into_writer(&self, &mut writer).expect("Failed to serialize VaultMetrics");
        Cow::Owned(writer)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        ciborium::from_reader(bytes.as_ref()).unwrap_or_else(|e| {
            ic_cdk::eprintln!("Error deserializing VaultMetrics: {:?}. Returning default.", e);
            VaultMetrics::default() // Return default on error
        })
    }

    // Use Unbounded as metrics size might grow, but unlikely to hit limits for a single cell.
    const BOUND: Bound = Bound::Unbounded;
}

// Public function to get current metrics (potentially useful for API layer)
pub async fn get_vault_metrics() -> Result<VaultMetrics, VaultError> {
    // Reads from the stable cell
    Ok(get_metrics())
} 
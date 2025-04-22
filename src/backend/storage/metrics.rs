use crate::storage::memory::{get_metrics_memory, Memory};
use crate::storage::storable::Cbor;
use crate::metrics::VaultMetrics; // Assuming VaultMetrics is defined here
use ic_stable_structures::StableCell;
use std::cell::RefCell;

type StorableVaultMetrics = Cbor<VaultMetrics>;

thread_local! {
    /// Global Vault Metrics
    pub static METRICS_CELL: RefCell<StableCell<StorableVaultMetrics, Memory>> = RefCell::new(
        StableCell::init(get_metrics_memory(), Cbor(VaultMetrics::default()))
            .expect("Failed to initialize metrics stable cell")
    );
}

/// Helper function to get metrics, handling potential cell initialization issues.
pub fn get_metrics() -> VaultMetrics {
    METRICS_CELL.with(|cell| cell.borrow().get().0.clone())
}

/// Helper function to update metrics.
pub fn update_metrics<F>(update_fn: F) -> Result<(), String>
where
    F: FnOnce(&mut VaultMetrics),
{
    METRICS_CELL.with(|cell| {
        let mut metrics = cell.borrow().get().0.clone(); // Clone the current metrics
        update_fn(&mut metrics); // Apply the update function
        cell.borrow_mut()
            .set(Cbor(metrics)) // Set the updated metrics back
            .map_err(|e| format!("Failed to update metrics: {:?}", e))?;
        Ok(())
    })
}

/// Increments the total vault count.
pub fn increment_vault_count() -> Result<(), String> {
    update_metrics(|metrics| {
        metrics.total_vaults = metrics.total_vaults.saturating_add(1);
    })
}

/// Decrements the total vault count (e.g., during deletion).
pub fn decrement_vault_count() -> Result<(), String> {
    update_metrics(|metrics| {
        metrics.total_vaults = metrics.total_vaults.saturating_sub(1);
    })
}

/// Updates the count of active vaults.
/// Typically called when a vault transitions into or out of the Active state.
pub fn update_active_vault_count(delta: i64) -> Result<(), String> {
    update_metrics(|metrics| {
        if delta > 0 {
            metrics.active_vaults = metrics.active_vaults.saturating_add(delta as u32);
        } else {
            metrics.active_vaults = metrics.active_vaults.saturating_sub(delta.abs() as u32);
        }
    })
}
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
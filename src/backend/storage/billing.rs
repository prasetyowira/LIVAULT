use crate::storage::memory::{
    get_billing_log_data_memory,
    get_billing_log_index_memory,
    Memory,
};
use crate::storage::storable::Cbor;
use crate::models::billing::BillingEntry;
use ic_stable_structures::StableLog;
use std::cell::RefCell;

type StorableBillingEntry = Cbor<BillingEntry>;

thread_local! {
    /// Billing Log: Append-only log of billing events.
    pub static BILLING_LOG: RefCell<StableLog<StorableBillingEntry, Memory, Memory>> = RefCell::new(
        StableLog::init(get_billing_log_index_memory(), get_billing_log_data_memory())
            .expect("Failed to initialize billing log")
    );
}

/// Helper function to append a billing entry to the log.
pub fn add_billing_entry(entry: BillingEntry) -> Result<u64, String> {
    BILLING_LOG.with(|log| {
        log.borrow_mut()
            .append(&Cbor(entry))
            .map_err(|e| format!("Failed to append billing entry: {:?}", e))
    })
}

/// Helper function to retrieve all billing entries.
/// Note: This reads the entire log. Implement pagination if needed for large logs.
pub fn get_all_billing_entries() -> Vec<BillingEntry> {
    BILLING_LOG.with(|log| {
        log.borrow()
            .iter()
            .map(|cbor_entry| cbor_entry.0)
            .collect()
    })
}

// TODO: Add function to query billing log entries with pagination
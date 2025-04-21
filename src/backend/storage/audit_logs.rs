use crate::storage::memory::{get_audit_log_data_memory, Memory}; // Assuming data memory is sufficient for map
use crate::storage::storable::{Cbor, StorableString};
use crate::models::audit_log::AuditLogEntry;
use ic_stable_structures::StableBTreeMap;
use std::cell::RefCell;
use ic_cdk::api::time;

// Using Vec for now as in structures.rs. Consider StableLog if append-only is sufficient.
type StorableAuditLogVec = Cbor<Vec<AuditLogEntry>>;

thread_local! {
    /// Audit Logs: Key = "audit:{vault_id}", Value = Vec<AuditLogEntry>
    /// Stores audit trail per vault. Capped manually during retrieval or maintenance.
    pub static LOGS: RefCell<StableBTreeMap<StorableString, StorableAuditLogVec, Memory>> = RefCell::new(
        StableBTreeMap::init(get_audit_log_data_memory()) // Reusing data memory ID as in structures.rs
    );
}

/// Generates a key for the AUDIT_LOGS map.
/// Format: "audit:{vault_id}"
fn create_audit_log_key(vault_id: &str) -> StorableString {
    // Use the Cbor constructor directly for StorableString key
    Cbor(format!("audit:{}", vault_id))
}

/// Helper function to add an audit log entry for a specific vault.
/// It retrieves the current log vector, appends the new entry, and saves it back.
/// Note: This can be potentially expensive for very long logs. Capping/rotation might be needed later.
pub fn add_entry(vault_id_str: &str, mut entry: AuditLogEntry) -> Result<(), String> {
    LOGS.with(|map_ref| {
        let key = create_audit_log_key(vault_id_str);
        let mut map = map_ref.borrow_mut();

        // Ensure timestamp and vault_id are set correctly in the entry
        entry.timestamp = time();
        entry.vault_id = vault_id_str.to_string(); // Use the passed string ID

        // Get current log vector or create a new one
        let mut current_log_vec = map.get(&key)
            .map(|cbor| cbor.0.clone()) // Clone the inner Vec<AuditLogEntry>
            .unwrap_or_else(Vec::new);

        // Append the new entry
        current_log_vec.push(entry);

        // Save the updated vector back to the map
        // StableBTreeMap::insert returns Option<V>, indicating the previous value.
        // Errors during stable memory operations typically trap, so we map success to Ok(()).
        map.insert(key, Cbor(current_log_vec));
        Ok(())
    })
}

/// Helper function to retrieve audit log entries for a specific vault.
/// Note: This retrieves the entire log. Implement pagination or filtering if needed.
pub fn get_entries(vault_id_str: &str) -> Option<Vec<AuditLogEntry>> {
    LOGS.with(|map_ref| {
        let key = create_audit_log_key(vault_id_str);
        map_ref.borrow().get(&key).map(|cbor| cbor.0.clone())
    })
}

// TODO: Add function for log compaction/rotation if needed. 
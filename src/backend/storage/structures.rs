// src/backend/storage/structures.rs
use crate::metrics::VaultMetrics;
use crate::models::vault_config::{VaultConfig, UnlockConditions};
use crate::models::vault_content_item::VaultContentItem;
use crate::models::vault_invite_token::VaultInviteToken;
use crate::models::vault_member::VaultMember;
use crate::models::billing::BillingEntry;
use crate::models::audit_log::AuditLogEntry;
use crate::models::common::{VaultId, PrincipalId};
use crate::storage::memory::{
    get_audit_log_data_memory,
    get_audit_log_index_memory,
    get_billing_log_data_memory,
    get_billing_log_index_memory,
    get_content_index_memory,
    get_content_items_memory,
    get_invite_tokens_memory,
    get_metrics_memory,
    get_vault_config_memory,
    get_vault_members_memory,
    Memory,
};
use crate::storage::storable::{Cbor, StorableString};
use ic_stable_structures::{StableBTreeMap, StableCell, StableLog};
use std::cell::RefCell;
use ic_cdk::api::time;
use serde;

// Define Storable types for models using the CBOR wrapper
type StorableVaultConfig = Cbor<VaultConfig>;
type StorableVaultMember = Cbor<VaultMember>;
type StorableInviteToken = Cbor<VaultInviteToken>;
type StorableContentItem = Cbor<VaultContentItem>;
// Define Storable<Vec<String>> for ContentIndex etc.
type StorableStringVec = Cbor<Vec<String>>;
// Add Storable types for AuditLogEntry
type StorableAuditLogEntry = Cbor<AuditLogEntry>;
type StorableAuditLogVec = Cbor<Vec<AuditLogEntry>>; // Vec of entries per vault
type StorableVaultMetrics = Cbor<VaultMetrics>;
type StorableBillingEntry = Cbor<BillingEntry>;

thread_local! {
    // --- Primary Data Maps ---

    /// Vault Configurations: Key = vault_id (String), Value = VaultConfig
    pub static VAULT_CONFIGS: RefCell<StableBTreeMap<StorableString, StorableVaultConfig, Memory>> = RefCell::new(
        StableBTreeMap::init(get_vault_config_memory())
    );

    /// Vault Members: Key = (VaultId, PrincipalId), Value = VaultMember
    pub static VAULT_MEMBERS: RefCell<StableBTreeMap<(VaultId, PrincipalId), StorableVaultMember, Memory>> = RefCell::new(
        StableBTreeMap::init(get_vault_members_memory())
    );

    /* // Removed: Handled by storage/tokens.rs
    /// Invite Tokens: Key = token_id (String), Value = VaultInviteToken
    pub static INVITE_TOKENS: RefCell<StableBTreeMap<StorableString, StorableInviteToken, Memory>> = RefCell::new(
        StableBTreeMap::init(get_invite_tokens_memory())
    );
    */

    /* // Removed: Handled by storage/content.rs
    /// Content Items: Key = content_id (String), Value = VaultContentItem
    pub static CONTENT_ITEMS: RefCell<StableBTreeMap<StorableString, StorableContentItem, Memory>> = RefCell::new(
        StableBTreeMap::init(get_content_items_memory())
    );
    */

    // --- Index Maps ---

    /// Content Index: Key = vault_id (String), Value = Vec<content_id: String>
    /// Stores the ordered list of content items per vault.
    pub static CONTENT_INDEX: RefCell<StableBTreeMap<StorableString, StorableStringVec, Memory>> = RefCell::new(
        StableBTreeMap::init(get_content_index_memory())
    );

    // --- Singleton Data ---

    /// Global Vault Metrics
    pub static METRICS: RefCell<StableCell<StorableVaultMetrics, Memory>> = RefCell::new(
        StableCell::init(get_metrics_memory(), Cbor(VaultMetrics::default()))
            .expect("Failed to initialize metrics stable cell")
    );

    // --- Log Data ---

    /// Billing Log: Append-only log of billing events.
    pub static BILLING_LOG: RefCell<StableLog<StorableBillingEntry, Memory, Memory>> = RefCell::new(
        StableLog::init(get_billing_log_index_memory(), get_billing_log_data_memory())
            .expect("Failed to initialize billing log")
    );

    /// Audit Logs: Key = "audit:{vault_id}", Value = Vec<AuditLogEntry>
    /// Stores audit trail per vault. Capped manually during retrieval or maintenance.
    pub static AUDIT_LOGS: RefCell<StableBTreeMap<StorableString, StorableAuditLogVec, Memory>> = RefCell::new(
        StableBTreeMap::init(get_audit_log_data_memory()) // Reusing data memory ID
    );

    // Add Metrics map later
}

// --- Key Generation Helpers (as per backend.architecture.md) ---

/// Generates a key for the AUDIT_LOGS map.
/// Format: "audit:{vault_id}"
pub fn create_audit_log_key(vault_id: &str) -> String {
    format!("audit:{}", vault_id)
}

// Add other key generation helpers if needed, e.g., for prefixed iteration.

/// Helper function to get the inner Cbor value from storage result
pub fn get_value<T: serde::Serialize + for<'de> serde::Deserialize<'de>>(result: Option<Cbor<T>>) -> Option<T> {
    result.map(|cbor| cbor.0)
}

/// Helper function to get metrics, handling potential cell initialization issues.
pub fn get_metrics() -> VaultMetrics {
    METRICS.with(|cell| cell.borrow().get().0.clone())
}

/// Helper function to update metrics.
pub fn update_metrics<F>(update_fn: F) -> Result<(), String>
where
    F: FnOnce(&mut VaultMetrics),
{
    METRICS.with(|cell| {
        let mut metrics = cell.borrow().get().0.clone(); // Clone the current metrics
        update_fn(&mut metrics); // Apply the update function
        cell.borrow_mut().set(Cbor(metrics)) // Set the updated metrics back
            .map(|_| ()) // Map successful set to Ok(())
            .map_err(|e| format!("Failed to update metrics: {:?}", e))
    })
}

/// Helper function to append a billing entry to the log.
pub fn add_billing_entry(entry: BillingEntry) -> Result<u64, String> {
    BILLING_LOG.with(|log| {
        log.borrow_mut()
            .append(&Cbor(entry))
            .map_err(|e| format!("Failed to append billing entry: {:?}", e))
    })
}

/// Helper function to add an audit log entry for a specific vault.
/// It retrieves the current log vector, appends the new entry, and saves it back.
/// Note: This can be potentially expensive for very long logs. Capping/rotation might be needed later.
pub fn add_audit_log_entry(vault_id: &str, mut entry: AuditLogEntry) -> Result<(), String> {
    AUDIT_LOGS.with(|map_ref| {
        // Use the Cbor constructor directly, as StorableString is a type alias
        let key = Cbor(create_audit_log_key(vault_id));
        let mut map = map_ref.borrow_mut();

        // Ensure timestamp and vault_id are set correctly in the entry
        entry.timestamp = time();
        entry.vault_id = vault_id.to_string();

        // Get current log vector or create a new one
        let mut current_log_vec = map.get(&key)
            .map(|cbor| cbor.0.clone()) // Clone the inner Vec<AuditLogEntry>
            .unwrap_or_else(Vec::new);

        // Append the new entry
        current_log_vec.push(entry);

        // Save the updated vector back to the map
        match map.insert(key, Cbor(current_log_vec)) {
            Some(_previous_value) => Ok(()), // Overwrite successful
            None => Ok(()), // Insert successful
            // Note: StableBTreeMap::insert itself doesn't return Err, but underlying storage operations could fail in theory.
            // However, the interface returns Option<V>. If an error occurs, it usually traps.
            // We handle potential errors at a higher level or rely on the trap mechanism.
        }
    })
}

// --- Audit Log Retrieval (Example, might need refinement/pagination) ---
/// Helper function to retrieve audit log entries for a specific vault.
/// Note: This retrieves the entire log. Implement pagination or filtering if needed.
pub fn get_audit_log_entries(vault_id: &str) -> Option<Vec<AuditLogEntry>> {
    AUDIT_LOGS.with(|map_ref| {
        // Use the Cbor constructor directly, as StorableString is a type alias
        let key = Cbor(create_audit_log_key(vault_id));
        map_ref.borrow().get(&key).map(|cbor| cbor.0.clone())
    })
}

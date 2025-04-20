// src/backend/storage/structures.rs
use crate::metrics::VaultMetrics;
use crate::models::{
    BillingEntry,
    VaultConfig,
    VaultContentItem,
    VaultInviteToken,
    VaultMember,
    // Add other models like AuditLogEntry, VaultMetrics later
};
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

// Define Storable types for models using the CBOR wrapper
type StorableVaultConfig = Cbor<VaultConfig>;
type StorableVaultMember = Cbor<VaultMember>;
type StorableInviteToken = Cbor<VaultInviteToken>;
type StorableContentItem = Cbor<VaultContentItem>;
// Define Storable<Vec<String>> for ContentIndex etc.
type StorableStringVec = Cbor<Vec<String>>;
// Add Storable types for AuditLogEntry, VaultMetrics later
type StorableVaultMetrics = Cbor<VaultMetrics>;
type StorableBillingEntry = Cbor<BillingEntry>;

thread_local! {
    // --- Primary Data Maps ---

    /// Vault Configurations: Key = vault_id (String), Value = VaultConfig
    pub static VAULT_CONFIGS: RefCell<StableBTreeMap<StorableString, StorableVaultConfig, Memory>> = RefCell::new(
        StableBTreeMap::init(get_vault_config_memory())
    );

    /// Vault Members: Key = member_key (e.g., "member:{vault_id}:{member_id}"), Value = VaultMember
    pub static VAULT_MEMBERS: RefCell<StableBTreeMap<StorableString, StorableVaultMember, Memory>> = RefCell::new(
        StableBTreeMap::init(get_vault_members_memory())
    );

    /// Invite Tokens: Key = token_id (String), Value = VaultInviteToken
    pub static INVITE_TOKENS: RefCell<StableBTreeMap<StorableString, StorableInviteToken, Memory>> = RefCell::new(
        StableBTreeMap::init(get_invite_tokens_memory())
    );

    /// Content Items: Key = content_id (String), Value = VaultContentItem
    pub static CONTENT_ITEMS: RefCell<StableBTreeMap<StorableString, StorableContentItem, Memory>> = RefCell::new(
        StableBTreeMap::init(get_content_items_memory())
    );

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

    // Add Audit Log Index/Data maps later
    // Add Metrics map later
}

// --- Key Generation Helpers (as per backend.architecture.md) ---

/// Generates a key for the VAULT_MEMBERS map.
/// Format: "member:{vault_id}:{member_id}"
pub fn create_member_key(vault_id: &str, member_id: &str) -> String {
    format!("member:{}:{}", vault_id, member_id)
}

// Add other key generation helpers if needed, e.g., for prefixed iteration.

/// Helper function to get the inner Cbor value from storage result
pub fn get_value<T>(result: Option<Cbor<T>>) -> Option<T> {
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
// src/backend/storage/structures.rs
use crate::models::{
    VaultConfig,
    VaultContentItem,
    VaultInviteToken,
    VaultMember,
    // Add other models like AuditLogEntry, VaultMetrics later
};
use crate::storage::memory::{
    get_audit_log_data_memory,
    get_audit_log_index_memory,
    get_content_index_memory,
    get_content_items_memory,
    get_invite_tokens_memory,
    get_metrics_memory,
    get_vault_config_memory,
    get_vault_members_memory,
    Memory,
};
use crate::storage::storable::{Cbor, StorableString};
use ic_stable_structures::StableBTreeMap;
use std::cell::RefCell;

// Define Storable types for models using the CBOR wrapper
type StorableVaultConfig = Cbor<VaultConfig>;
type StorableVaultMember = Cbor<VaultMember>;
type StorableInviteToken = Cbor<VaultInviteToken>;
type StorableContentItem = Cbor<VaultContentItem>;
// Define Storable<Vec<String>> for ContentIndex etc.
type StorableStringVec = Cbor<Vec<String>>;
// Add Storable types for AuditLogEntry, VaultMetrics later

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
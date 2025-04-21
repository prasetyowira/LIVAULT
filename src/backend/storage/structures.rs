// src/backend/storage/structures.rs
use crate::metrics::VaultMetrics;
use crate::models::vault_config::VaultConfig;
use crate::models::vault_content_item::VaultContentItem;
use crate::models::vault_invite_token::VaultInviteToken;
use crate::models::vault_member::VaultMember;
use crate::models::billing::BillingEntry;
use crate::models::audit_log::AuditLogEntry;
use crate::storage::storable::Cbor;
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

/// Helper function to get the inner Cbor value from storage result
pub fn get_value<T: serde::Serialize + for<'de> serde::Deserialize<'de>>(result: Option<Cbor<T>>) -> Option<T> {
    result.map(|cbor| cbor.0)
}

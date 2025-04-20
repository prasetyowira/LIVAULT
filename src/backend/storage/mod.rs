// src/backend/storage/mod.rs
// Placeholder for stable memory management using ic-stable-structures 

pub mod cursor;
pub mod memory;
pub mod storable;
pub mod structures;

// Re-export key storage structures and functions for easier access
pub use cursor::{get_cursor, increment_cursor, set_cursor};
pub use memory::Memory;
pub use storable::{Cbor, StorableString};
pub use structures::{
    add_billing_entry,
    add_audit_log_entry,
    get_audit_log_entries,
    create_member_key,
    create_audit_log_key,
    get_metrics,
    get_value,
    update_metrics,
    AUDIT_LOGS,
    BILLING_LOG,
    CONTENT_INDEX,
    CONTENT_ITEMS,
    INVITE_TOKENS,
    METRICS,
    VAULT_CONFIGS,
    VAULT_MEMBERS,
}; 
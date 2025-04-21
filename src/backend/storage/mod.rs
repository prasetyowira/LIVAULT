// src/backend/storage/mod.rs
// Placeholder for stable memory management using ic-stable-structures 

pub mod cursor;
pub mod memory;
pub mod storable;
pub mod structures; // Keep for now, contains other entities

// New modular storage
pub mod tokens;
pub mod content;
pub mod uploads;
// TODO: Add modules for vault_configs, members, audit_logs, metrics etc. if fully modularizing

// Re-export key storage structures and functions for easier access
pub use cursor::{get_cursor, increment_cursor, set_cursor};
pub use memory::Memory;
pub use storable::{Cbor, StorableString};

// Re-export functions from new modules
pub use tokens::{ get_next_token_id, insert_token, get_token, get_internal_token_id, remove_token };
pub use content::{ get_next_content_id, insert_content, get_content, get_internal_content_id, remove_content };
pub use uploads::{ get_next_upload_id, /* insert_upload_session, get_upload_session, get_internal_upload_id, remove_upload_session */ }; // Placeholder functions

// Re-export remaining items from the original structures module
pub use structures::{
    add_billing_entry,
    add_audit_log_entry,
    get_audit_log_entries,
    create_member_key, // Still needed for VAULT_MEMBERS
    create_audit_log_key,
    get_metrics,
    get_value,
    update_metrics,
    AUDIT_LOGS,
    BILLING_LOG,
    CONTENT_INDEX, // Keep for now, might need refactoring with content.rs
    METRICS,
    VAULT_CONFIGS, // Keep for now
    VAULT_MEMBERS, // Keep for now
    // CONTENT_ITEMS, // Removed, handled by content.rs
    // INVITE_TOKENS, // Removed, handled by tokens.rs
};

// TODO: Fully refactor structures.rs into modular components. 
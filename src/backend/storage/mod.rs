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
pub mod members;
pub mod config; // Add config module
pub mod vault_configs;
pub mod audit_logs;
// TODO: Add modules for vault_configs, members, audit_logs, metrics etc. if fully modularizing

// Re-export key storage structures and functions for easier access
pub use cursor::{get_cursor, increment_cursor, set_cursor};
pub use memory::Memory;
pub use storable::{Cbor, StorableString};

// Re-export functions from new modules
pub use tokens::{ get_next_token_id, insert_token, get_token, get_internal_token_id, remove_token };
pub use content::{ get_next_content_id, insert_content, get_content, get_internal_content_id, remove_content, update_content };
pub use uploads::{ get_next_upload_id, /* insert_upload_session, get_upload_session, get_internal_upload_id, remove_upload_session */ }; // Placeholder functions
pub use members::{ insert_member, get_member, remove_member, get_members_by_vault, is_member };
pub use config::{ get_admin_principal, get_cron_principal, get_min_cycles_threshold }; // Re-export config getters
pub use vault_configs::{ insert_vault_config, get_vault_config, remove_vault_config };
pub use audit_logs::{ add_entry as add_audit_log_entry, get_entries as get_audit_log_entries }; // Renaming for clarity

// Re-export remaining items from the original structures module
pub use structures::{
    add_billing_entry,
    get_metrics,
    get_value,
    update_metrics,
    create_audit_log_key, // Keep helper? Or move/remove? Let's keep for now.
    BILLING_LOG,
    CONTENT_INDEX, // Keep for now, might need refactoring with content.rs
    METRICS,
    // VAULT_CONFIGS, // Moved to storage::vault_configs
    // VAULT_MEMBERS, // Moved to storage::members
    // CONTENT_ITEMS, // Removed, handled by content.rs
    // INVITE_TOKENS, // Removed, handled by tokens.rs
};

// TODO: Fully refactor structures.rs into modular components. 
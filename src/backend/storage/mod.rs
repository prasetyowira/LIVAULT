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
pub mod metrics;
pub mod billing;
pub mod content_index;
pub mod approvals; // Added approvals module

// Re-export key storage structures and functions for easier access
pub use cursor::{get_cursor, increment_cursor, set_cursor};
pub use memory::Memory;
pub use storable::{Cbor, StorableString};

// Re-export functions from new modules
pub use tokens::{ get_next_token_id, insert_token, get_token, get_internal_token_id, remove_token, remove_tokens_by_vault };
pub use content::{ get_next_content_id, insert_content, get_content, get_internal_content_id, remove_content, update_content, remove_all_content_for_vault };
pub use uploads::{ get_next_upload_id, insert_upload_session, get_upload_session, get_internal_upload_id, remove_upload_session, save_chunk, get_chunk, delete_chunks };
pub use members::{ insert_member, get_member, remove_member, get_members_by_vault, is_member, get_vaults_by_member, is_member_with_role, remove_members_by_vault };
pub use config::{ get_admin_principal, get_cron_principal, get_min_cycles_threshold }; // Re-export config getters
pub use vault_configs::{ insert_vault_config, get_vault_config, remove_vault_config, get_vaults_config_by_owner };
pub use audit_logs::{add_entry as add_audit_log_entry, get_entries as get_audit_log_entries, compact_log as compact_audit_log, remove_audit_logs as remove_audit_logs };
pub use metrics::{ get_metrics, update_metrics };
pub use billing::{ add_billing_entry, get_all_billing_entries, query_billing_entries };
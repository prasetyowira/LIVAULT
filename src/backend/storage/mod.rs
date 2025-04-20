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
    create_member_key,
    get_value,
    CONTENT_INDEX,
    CONTENT_ITEMS,
    INVITE_TOKENS,
    VAULT_CONFIGS,
    VAULT_MEMBERS,
}; 
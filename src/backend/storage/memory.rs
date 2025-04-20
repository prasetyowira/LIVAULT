// src/backend/storage/memory.rs
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{DefaultMemoryImpl, StableCell};
use std::cell::RefCell;

// Define Memory IDs for stable structures
// Choose non-overlapping IDs
const UPGRADES_MEMORY_ID: MemoryId = MemoryId::new(0);
const VAULT_CONFIG_MEM_ID: MemoryId = MemoryId::new(1);
const VAULT_MEMBERS_MEM_ID: MemoryId = MemoryId::new(2);
const INVITE_TOKENS_MEM_ID: MemoryId = MemoryId::new(3);
const CONTENT_ITEMS_MEM_ID: MemoryId = MemoryId::new(4);
const CONTENT_INDEX_MEM_ID: MemoryId = MemoryId::new(5);
const AUDIT_LOG_INDEX_MEM_ID: MemoryId = MemoryId::new(6);
const AUDIT_LOG_DATA_MEM_ID: MemoryId = MemoryId::new(7);
const METRICS_MEM_ID: MemoryId = MemoryId::new(8);
// Reserve IDs 9-19 for future use
const STAGING_BUFFER_MEM_ID: MemoryId = MemoryId::new(20);
const CURSOR_MEM_ID: MemoryId = MemoryId::new(21);

// Define memory type alias
pub type Memory = VirtualMemory<DefaultMemoryImpl>;

thread_local! {
    // Memory manager
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    // Stable cell for managing upgrades (optional but good practice)
    pub static UPGRADES: RefCell<StableCell<u64, Memory>> = RefCell::new(
        StableCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(UPGRADES_MEMORY_ID)), 0)
            .expect("Failed to initialize upgrades cell")
    );
}

/// Get memory instance for a specific MemoryId.
pub fn get_memory(id: MemoryId) -> Memory {
    MEMORY_MANAGER.with(|m| m.borrow().get(id))
}

// Functions to get specific memory instances
pub fn get_vault_config_memory() -> Memory {
    get_memory(VAULT_CONFIG_MEM_ID)
}

pub fn get_vault_members_memory() -> Memory {
    get_memory(VAULT_MEMBERS_MEM_ID)
}

pub fn get_invite_tokens_memory() -> Memory {
    get_memory(INVITE_TOKENS_MEM_ID)
}

pub fn get_content_items_memory() -> Memory {
    get_memory(CONTENT_ITEMS_MEM_ID)
}

pub fn get_content_index_memory() -> Memory {
    get_memory(CONTENT_INDEX_MEM_ID)
}

pub fn get_audit_log_index_memory() -> Memory {
    get_memory(AUDIT_LOG_INDEX_MEM_ID)
}

pub fn get_audit_log_data_memory() -> Memory {
    get_memory(AUDIT_LOG_DATA_MEM_ID)
}

pub fn get_metrics_memory() -> Memory {
    get_memory(METRICS_MEM_ID)
}

pub fn get_staging_buffer_memory() -> Memory {
    get_memory(STAGING_BUFFER_MEM_ID)
}

pub fn get_cursor_memory() -> Memory {
    get_memory(CURSOR_MEM_ID)
} 
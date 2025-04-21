// src/backend/storage/content.rs
use crate::error::VaultError;
use crate::models::VaultContentItem;
use crate::storage::storable::Cbor;
use crate::storage::memory::{Memory, get_content_counter_memory, get_content_items_memory, get_content_principal_idx_memory};
use ic_stable_structures::{StableCell, StableBTreeMap, DefaultMemoryImpl};
use std::cell::RefCell;
use candid::Principal;

type StorableContent = Cbor<VaultContentItem>;
type PrincipalBytes = Vec<u8>; // Key for secondary index

thread_local! {
    // Counter for generating internal content IDs
    static CONTENT_COUNTER: RefCell<StableCell<u64, Memory>> = RefCell::new(
        StableCell::init(get_content_counter_memory(), 0)
            .expect("Failed to initialize content counter")
    );

    // Primary storage: Internal u64 -> Content Data
    // Note: Reusing CONTENT_ITEMS_MEM_ID for the primary map
    static CONTENT_MAP: RefCell<StableBTreeMap<u64, StorableContent, Memory>> = RefCell::new(
        StableBTreeMap::init(get_content_items_memory())
    );

    // Secondary index: Exposed Principal Bytes -> Internal u64 ID
    static CONTENT_PRINCIPAL_INDEX: RefCell<StableBTreeMap<PrincipalBytes, u64, Memory>> = RefCell::new(
        StableBTreeMap::init(get_content_principal_idx_memory())
    );
}

/// Gets the next available internal content ID and increments the counter.
pub fn get_next_content_id() -> Result<u64, VaultError> {
    CONTENT_COUNTER.with(|cell_ref| {
        let cell = cell_ref.borrow();
        let current_val = *cell.get();
        let next_val = current_val.checked_add(1)
            .ok_or_else(|| VaultError::InternalError("Content counter overflow".to_string()))?;
        cell_ref.borrow_mut().set(next_val)
            .map_err(|e| VaultError::StorageError(format!("Failed to update content counter: {:?}", e)))?;
        Ok(current_val)
    })
}

/// Inserts a content item into both the primary map and the secondary index.
pub fn insert_content(internal_id: u64, item: VaultContentItem, principal_id: Principal) -> Result<(), VaultError> {
    let storable_item = Cbor(item);
    let principal_bytes = principal_id.as_slice().to_vec();

    CONTENT_MAP.with(|map_ref| {
        map_ref.borrow_mut().insert(internal_id, storable_item);
    });

    CONTENT_PRINCIPAL_INDEX.with(|index_ref| {
         index_ref.borrow_mut().insert(principal_bytes, internal_id);
    });
    Ok(())
}

/// Retrieves a content item using its internal u64 ID.
pub fn get_content(internal_id: u64) -> Option<VaultContentItem> {
    CONTENT_MAP.with(|map_ref| map_ref.borrow().get(&internal_id).map(|c| c.0))
}

/// Retrieves the internal u64 ID using the exposed Principal ID.
pub fn get_internal_content_id(principal: Principal) -> Option<u64> {
    CONTENT_PRINCIPAL_INDEX.with(|index_ref| {
        index_ref.borrow().get(&principal.as_slice().to_vec())
    })
}

/// Removes a content item from both the primary map and the secondary index.
pub fn remove_content(internal_id: u64, principal_id: Principal) -> Result<(), VaultError> {
    let removed_item = CONTENT_MAP.with(|map_ref| map_ref.borrow_mut().remove(&internal_id));
    if removed_item.is_none() {
        ic_cdk::println!("WARN: remove_content called for non-existent internal ID: {}", internal_id);
    }

    let principal_bytes = principal_id.as_slice().to_vec();
    let removed_index = CONTENT_PRINCIPAL_INDEX.with(|index_ref| {
        index_ref.borrow_mut().remove(&principal_bytes)
    });
    if removed_index.is_none() {
         ic_cdk::println!("WARN: remove_content removed primary data but index entry for principal {} was already missing.", principal_id);
    }

    Ok(())
}

// TODO: Add function to update content item (if needed), ensuring secondary index is handled. 
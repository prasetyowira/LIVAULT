// src/backend/storage/content.rs
use crate::error::VaultError;
use crate::models::vault_content_item::VaultContentItem;
use crate::storage::storable::Cbor;
use crate::storage::memory::{Memory, get_content_counter_memory, get_content_items_memory, get_content_principal_idx_memory};
use ic_stable_structures::{StableCell, StableBTreeMap};
use std::cell::RefCell;
use candid::Principal;
use crate::models::common::{VaultId, ContentId};

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

/// Updates an existing content item.
/// Assumes the internal ID and principal ID do not change during update.
pub fn update_content(internal_id: u64, updated_item: VaultContentItem) -> Result<(), VaultError> {
   let storable_item = Cbor(updated_item);

   CONTENT_MAP.with(|map_ref| {
       let mut map = map_ref.borrow_mut();
       if map.contains_key(&internal_id) {
           map.insert(internal_id, storable_item);
           Ok(())
       } else {
           Err(VaultError::NotFound(format!("Content item with internal ID {} not found for update", internal_id)))
       }
   })
}

/// Removes all content items associated with a specific vault.
/// This involves fetching the index, then removing items one by one.
/// Returns the number of content items removed.
/// Note: Does NOT currently handle associated chunk deletion.
pub async fn remove_all_content_for_vault(vault_id: &VaultId) -> Result<u64, VaultError> {
    let content_principal_ids = match super::content_index::get_index(vault_id) {
        Ok(Some(ids)) => ids,
        Ok(None) => return Ok(0), // No content index for this vault
        Err(e) => return Err(VaultError::StorageError(format!("Failed to get content index for vault {}: {}", vault_id, e))),
    };

    let mut removed_count = 0u64;
    let mut errors = Vec::new();

    for principal_str in content_principal_ids {
        match Principal::from_text(&principal_str) {
            Ok(content_principal) => {
                if let Some(internal_id) = get_internal_content_id(content_principal) {
                    match remove_content(internal_id, content_principal).await {
                        Ok(_) => removed_count += 1,
                        Err(e) => {
                            ic_cdk::eprintln!("❌ ERROR: Failed removing content item {} (internal {}) for vault {}: {:?}", principal_str, internal_id, vault_id, e);
                            errors.push(format!("Failed removing {}: {:?}", principal_str, e));
                        }
                    }
                } else {
                    ic_cdk::eprintln!("⚠️ WARNING: Content principal {} from index not found in content principal index for vault {}", principal_str, vault_id);
                    errors.push(format!("Index inconsistency for {}", principal_str));
                }
            }
            Err(e) => {
                ic_cdk::eprintln!("❌ ERROR: Failed to parse content principal {} from index for vault {}: {:?}", principal_str, vault_id, e);
                errors.push(format!("Failed parsing {}: {:?}", principal_str, e));
            }
        }
    }

    // After attempting to remove all content, remove the index itself
    if let Err(e) = super::content_index::remove_index(vault_id).await {
         ic_cdk::eprintln!("❌ ERROR: Failed removing content index for vault {}: {:?}", vault_id, e);
         errors.push(format!("Failed removing index: {:?}", e));
    }

    if errors.is_empty() {
        Ok(removed_count)
    } else {
        // Combine errors into a single message
        Err(VaultError::StorageError(format!(
            "Errors during content removal for vault {}: {}",
            vault_id,
            errors.join("; ")
        )))
    }
}
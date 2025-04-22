use crate::storage::memory::{get_content_index_memory, Memory};
use crate::storage::storable::{Cbor, StorableString};
use crate::models::common::{VaultId, ContentId};
use ic_stable_structures::StableBTreeMap;
use std::cell::RefCell;
use candid::Principal;

type StorableStringVec = Cbor<Vec<String>>; // Stores Vec<ContentId.to_text()>

// Key: VaultId (Principal String)
// Value: Cbor<Vec<ContentId (Principal String)>>
type ContentIndexMap = StableBTreeMap<StorableString, Cbor<Vec<String>>, Memory>;

thread_local! {
    /// Stable storage for mapping VaultId to an ordered list of ContentIds.
    static INDEX: RefCell<ContentIndexMap> = RefCell::new(
        ContentIndexMap::init(get_content_index_memory())
    );
}

/// Generates the key for the content index map.
fn create_index_key(vault_id: &VaultId) -> StorableString {
    Cbor(vault_id.to_string()) // Use the text representation as key
}

/// Adds a content ID string to the index for a given vault ID.
pub fn add_to_index(vault_id: &VaultId, content_id: &ContentId) -> Result<(), String> {
    let key = create_index_key(vault_id);
    let content_id_str = content_id.to_string();

    INDEX.with(|map_ref| {
        let mut map = map_ref.borrow_mut();
        let mut index_vec = map.get(&key).map(|c| c.0).unwrap_or_default();
        index_vec.push(content_id_str);
        map.insert(key, Cbor(index_vec));
    });
    Ok(())
}

/// Retrieves the list of content ID strings for a given vault ID.
pub fn get_index(vault_id: &VaultId) -> Result<Option<Vec<String>>, String> {
    let key = create_index_key(vault_id);
    Ok(INDEX.with(|map_ref| map_ref.borrow().get(&key).map(|c| c.0)))
}

/// Removes a specific content ID string from the index for a given vault ID.
pub fn remove_from_index(vault_id: &VaultId, content_id: &ContentId) -> Result<(), String> {
    let key = create_index_key(vault_id);
    let content_id_str = content_id.to_string();

    INDEX.with(|map_ref| {
        let mut map = map_ref.borrow_mut();
        if let Some(mut index_vec) = map.get(&key).map(|c| c.0) {
            index_vec.retain(|id| id != &content_id_str);
            map.insert(key, Cbor(index_vec));
        }
    });
    Ok(())
}

/// Removes the entire index entry for a given vault ID.
pub async fn remove_index(vault_id: &VaultId) -> Result<(), String> {
    let key = create_index_key(vault_id);
    INDEX.with(|map_ref| {
        map_ref.borrow_mut().remove(&key);
    });
    Ok(())
} 
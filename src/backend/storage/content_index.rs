use crate::storage::memory::{get_content_index_memory, Memory};
use crate::storage::storable::{Cbor, StorableString};
use crate::models::common::{VaultId, ContentId};
use ic_stable_structures::StableBTreeMap;
use std::cell::RefCell;

type StorableStringVec = Cbor<Vec<String>>; // Stores Vec<ContentId.to_text()>

thread_local! {
    /// Content Index: Key = vault_id (String), Value = Vec<content_id: String>
    /// Stores the ordered list of content items per vault.
    pub static INDEX: RefCell<StableBTreeMap<StorableString, StorableStringVec, Memory>> = RefCell::new(
        StableBTreeMap::init(get_content_index_memory())
    );
}

/// Generates the key for the content index map.
fn create_index_key(vault_id: &VaultId) -> StorableString {
    Cbor(vault_id.to_text())
}

/// Adds a content ID string to the index for a given vault ID.
pub fn add_to_index(vault_id: &VaultId, content_id: &ContentId) -> Result<(), String> {
    INDEX.with(|map_ref| {
        let key = create_index_key(vault_id);
        let mut map = map_ref.borrow_mut();

        let mut index_vec = map.get(&key).map_or_else(Vec::new, |cbor| cbor.0);
        index_vec.push(content_id.to_text());

        map.insert(key, Cbor(index_vec));
        Ok(())
    })
}

/// Retrieves the list of content ID strings for a given vault ID.
pub fn get_index(vault_id: &VaultId) -> Option<Vec<String>> {
    INDEX.with(|map_ref| {
        let key = create_index_key(vault_id);
        map_ref.borrow().get(&key).map(|cbor| cbor.0)
    })
}

/// Removes a specific content ID string from the index for a given vault ID.
pub fn remove_from_index(vault_id: &VaultId, content_id: &ContentId) -> Result<(), String> {
    INDEX.with(|map_ref| {
        let key = create_index_key(vault_id);
        let mut map = map_ref.borrow_mut();

        if let Some(mut index_vec) = map.get(&key).map(|cbor| cbor.0) {
            let content_id_str = content_id.to_text();
            if let Some(pos) = index_vec.iter().position(|x| *x == content_id_str) {
                index_vec.remove(pos);
                map.insert(key, Cbor(index_vec)); // Update the map
            } else {
                ic_cdk::println!("WARN: Content ID {} not found in index for vault {}", content_id_str, vault_id);
            }
        } else {
             ic_cdk::println!("WARN: Index not found for vault {}", vault_id);
        }
        Ok(())
    })
} 
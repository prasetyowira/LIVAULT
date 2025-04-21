// src/backend/storage/uploads.rs
// Manages storage related to file upload sessions.

use crate::error::VaultError;
// TODO: Define an UploadSession struct in models/ if needed to hold state like expected size, chunks received, associated Principal ID, etc.
// use crate::models::UploadSession;
use crate::storage::storable::Cbor;
use crate::storage::memory::{Memory, get_upload_counter_memory, get_staging_buffer_memory, get_upload_principal_idx_memory};
use ic_stable_structures::{StableCell, StableBTreeMap, DefaultMemoryImpl};
use std::cell::RefCell;
use candid::Principal;

// Placeholder type for upload session data
// Replace with actual UploadSession struct when defined
type UploadSessionData = (); // Placeholder
type StorableUploadSession = Cbor<UploadSessionData>;
type PrincipalBytes = Vec<u8>;

thread_local! {
    // Counter for generating internal upload session IDs
    static UPLOAD_COUNTER: RefCell<StableCell<u64, Memory>> = RefCell::new(
        StableCell::init(get_upload_counter_memory(), 0)
            .expect("Failed to initialize upload counter")
    );

    // Primary storage: Internal u64 -> Upload Session Data
    // TODO: Determine the appropriate memory ID. Using STAGING_BUFFER_MEM_ID as placeholder
    static UPLOAD_SESSIONS_MAP: RefCell<StableBTreeMap<u64, StorableUploadSession, Memory>> = RefCell::new(
        StableBTreeMap::init(get_staging_buffer_memory()) // Placeholder memory
    );

    // Secondary index: Exposed Principal Bytes -> Internal u64 ID
    static UPLOAD_PRINCIPAL_INDEX: RefCell<StableBTreeMap<PrincipalBytes, u64, Memory>> = RefCell::new(
        StableBTreeMap::init(get_upload_principal_idx_memory())
    );
}

/// Gets the next available internal upload ID and increments the counter.
pub fn get_next_upload_id() -> Result<u64, VaultError> {
    UPLOAD_COUNTER.with(|cell_ref| {
        let cell = cell_ref.borrow();
        let current_val = *cell.get();
        let next_val = current_val.checked_add(1)
            .ok_or_else(|| VaultError::InternalError("Upload counter overflow".to_string()))?;
        cell_ref.borrow_mut().set(next_val)
            .map_err(|e| VaultError::StorageError(format!("Failed to update upload counter: {:?}", e)))?;
        Ok(current_val)
    })
}

/// Inserts an upload session into both the primary map and the secondary index.
/// TODO: Update signature to take actual UploadSessionData
pub fn insert_upload_session(internal_id: u64, session_data: UploadSessionData, principal_id: Principal) -> Result<(), VaultError> {
    let storable_session = Cbor(session_data); // Placeholder
    let principal_bytes = principal_id.as_slice().to_vec();

    UPLOAD_SESSIONS_MAP.with(|map_ref| {
        map_ref.borrow_mut().insert(internal_id, storable_session);
    });

    UPLOAD_PRINCIPAL_INDEX.with(|index_ref| {
         index_ref.borrow_mut().insert(principal_bytes, internal_id);
    });
    Ok(())
}

/// Retrieves upload session data using its internal u64 ID.
/// TODO: Update return type
pub fn get_upload_session(internal_id: u64) -> Option<UploadSessionData> { // Placeholder return
    UPLOAD_SESSIONS_MAP.with(|map_ref| map_ref.borrow().get(&internal_id).map(|c| c.0))
}

/// Retrieves the internal u64 ID using the exposed Principal ID.
pub fn get_internal_upload_id(principal: Principal) -> Option<u64> {
    UPLOAD_PRINCIPAL_INDEX.with(|index_ref| {
        index_ref.borrow().get(&principal.as_slice().to_vec())
    })
}

/// Removes an upload session from both the primary map and the secondary index.
pub fn remove_upload_session(internal_id: u64, principal_id: Principal) -> Result<(), VaultError> {
    let removed_session = UPLOAD_SESSIONS_MAP.with(|map_ref| map_ref.borrow_mut().remove(&internal_id));
    if removed_session.is_none() {
        ic_cdk::println!("WARN: remove_upload_session called for non-existent internal ID: {}", internal_id);
    }

    let principal_bytes = principal_id.as_slice().to_vec();
    let removed_index = UPLOAD_PRINCIPAL_INDEX.with(|index_ref| {
        index_ref.borrow_mut().remove(&principal_bytes)
    });
     if removed_index.is_none() {
        ic_cdk::println!("WARN: remove_upload_session removed primary data but index entry for principal {} was already missing.", principal_id);
    }

    Ok(())
}

// TODO: Add functions for managing upload chunks (likely need separate storage structure). 
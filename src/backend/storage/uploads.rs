// src/backend/storage/uploads.rs
// Manages storage related to file upload sessions.

use crate::error::VaultError;
use crate::models::upload_session::UploadSession;
use crate::storage::storable::Cbor;
use crate::storage::memory::{Memory, get_upload_session_memory, get_upload_counter_memory, get_upload_principal_idx_memory, get_upload_chunks_memory};
use ic_stable_structures::{StableCell, StableBTreeMap};
use std::cell::RefCell;
use candid::Principal;

// Key for secondary index
type PrincipalBytes = Vec<u8>;
type StorableUploadSession = Cbor<UploadSession>;
// Type for storing chunk data - using Vec<u8> directly assuming chunks fit within bounds
// If chunks are large, consider a different approach (e.g., StableVec<u8>) or use the blob pattern.
type ChunkData = Vec<u8>; // Using raw bytes for chunks

thread_local! {
    // Counter for generating internal upload IDs
    static UPLOAD_COUNTER: RefCell<StableCell<u64, Memory>> = RefCell::new(
        StableCell::init(get_upload_counter_memory(), 0)
            .expect("Failed to initialize upload counter")
    );

    // Primary storage: Internal u64 -> Upload Session Data
    static UPLOAD_SESSIONS_MAP: RefCell<StableBTreeMap<u64, StorableUploadSession, Memory>> = RefCell::new(
        StableBTreeMap::init(get_upload_session_memory())
    );

    // Secondary index: Exposed Principal Bytes -> Internal u64 ID
    static UPLOAD_PRINCIPAL_INDEX: RefCell<StableBTreeMap<PrincipalBytes, u64, Memory>> = RefCell::new(
        StableBTreeMap::init(get_upload_principal_idx_memory())
    );

    // Storage for actual upload chunks: Key = (InternalUploadId, ChunkIndex), Value = ChunkData
    static UPLOAD_CHUNKS_MAP: RefCell<StableBTreeMap<(u64, u64), ChunkData, Memory>> = RefCell::new(
        StableBTreeMap::init(get_upload_chunks_memory())
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

/// Inserts or updates an upload session in both the primary map and the secondary index.
pub fn insert_upload_session(internal_id: u64, session: UploadSession, principal_id: Principal) -> Result<(), VaultError> {
    let storable_session = Cbor(session);
    let principal_bytes = principal_id.as_slice().to_vec();

    UPLOAD_SESSIONS_MAP.with(|map_ref| {
        map_ref.borrow_mut().insert(internal_id, storable_session);
    });

    UPLOAD_PRINCIPAL_INDEX.with(|index_ref| {
         index_ref.borrow_mut().insert(principal_bytes, internal_id);
    });
    Ok(())
}

/// Retrieves an upload session using its internal u64 ID.
pub fn get_upload_session(internal_id: u64) -> Option<UploadSession> {
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

/// Saves a chunk of data for a specific upload session.
pub fn save_chunk(internal_upload_id: u64, chunk_index: u64, data: ChunkData) -> Result<(), VaultError> {
    if data.is_empty() {
        return Err(VaultError::StorageError("Chunk data cannot be empty".to_string()));
    }
    let key = (internal_upload_id, chunk_index);
    UPLOAD_CHUNKS_MAP.with(|map_ref| {
        map_ref.borrow_mut().insert(key, data);
    });
    Ok(())
}

/// Retrieves a specific chunk of data.
pub fn get_chunk(internal_upload_id: u64, chunk_index: u64) -> Option<ChunkData> {
    let key = (internal_upload_id, chunk_index);
    UPLOAD_CHUNKS_MAP.with(|map_ref| {
        map_ref.borrow().get(&key)
    })
}

/// Deletes all chunks associated with a specific upload session.
/// Note: Iterates over keys, potentially less efficient for huge number of chunks per upload.
pub fn delete_chunks(internal_upload_id: u64) -> Result<(), VaultError> {
    UPLOAD_CHUNKS_MAP.with(|map_ref| {
        let mut map = map_ref.borrow_mut();
        let keys_to_remove: Vec<_> = map.iter()
            .filter(|((upload_id, _chunk_idx), _data)| *upload_id == internal_upload_id)
            .map(|(key, _data)| key)
            .collect();

        for key in keys_to_remove {
            map.remove(&key);
        }
    });
    Ok(())
}

// src/backend/services/upload_service.rs
// Placeholder for chunked upload logic 

use crate::{
    error::VaultError,
    models::{common::*, VaultContentItem},
    storage::{self, Cbor, StorableString, CONTENT_INDEX, CONTENT_ITEMS},
    utils::crypto::generate_ulid,
};
use ic_cdk::api::time;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::HashMap;

type UploadId = String;
const MAX_CHUNK_SIZE_BYTES: usize = 512 * 1024; // 512 KB
const MAX_TOTAL_UPLOAD_SIZE_BYTES: u64 = 10 * 1024 * 1024; // Example: 10 MB limit per file initially

// Represents metadata provided when starting an upload
#[derive(Clone, Debug, candid::CandidType, serde::Deserialize)]
pub struct FileMeta {
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: u64,     // Total expected size
    pub content_type: ContentType, // Should be File, Password, or Letter
    pub title: Option<String>,
    pub description: Option<String>,
}

// In-memory state for an ongoing chunked upload
#[derive(Clone, Debug)]
struct UploadState {
    vault_id: VaultId,
    upload_id: UploadId,
    file_meta: FileMeta,
    chunks: Vec<Vec<u8>>, // Store chunks in memory for now
    expected_chunks: usize,
    received_chunks: usize,
    created_at: Timestamp,
}

thread_local! {
    // In-memory map to store ongoing uploads. Cleared on upgrade.
    // Key: UploadId, Value: UploadState
    // TODO: Consider moving to stable memory if uploads need to survive upgrades.
    static ACTIVE_UPLOADS: RefCell<HashMap<UploadId, UploadState>> = RefCell::new(HashMap::new());
}

// --- Service Functions ---

/// Begins a new chunked upload session.
///
/// # Arguments
/// * `vault_id` - The vault to upload content into.
/// * `file_meta` - Metadata about the file/content being uploaded.
/// * `caller` - Principal initiating the upload (for auth & quota checks).
///
/// # Returns
/// * `Result<UploadId, VaultError>` - The unique ID for this upload session.
pub fn begin_chunked_upload(
    vault_id: VaultId,
    file_meta: FileMeta,
    caller: PrincipalId, // TODO: Use caller for auth/quota
) -> Result<UploadId, VaultError> {
    // 1. Validate Vault and Permissions (Simplified)
    let vault_config = super::vault_service::get_vault_config(&vault_id)?;
    if vault_config.owner != caller { // Simplistic check, might need more roles
        return Err(VaultError::NotAuthorized("Caller cannot upload to this vault".to_string()));
    }

    // 2. Validate FileMeta
    if file_meta.size_bytes == 0 {
        return Err(VaultError::UploadError("File size cannot be zero".to_string()));
    }
    if file_meta.size_bytes > MAX_TOTAL_UPLOAD_SIZE_BYTES {
        // TODO: Check against vault_config.storage_quota_bytes instead?
        return Err(VaultError::UploadError(format!(
            "Upload size {} exceeds limit {}",
            file_meta.size_bytes,
            MAX_TOTAL_UPLOAD_SIZE_BYTES
        )));
    }
    // TODO: Validate mime_type based on content_type?

    // 3. Calculate expected chunks
    let expected_chunks = (file_meta.size_bytes as usize + MAX_CHUNK_SIZE_BYTES - 1) / MAX_CHUNK_SIZE_BYTES;

    // 4. Create Upload State
    let upload_id = generate_ulid();
    let current_time = time();
    let state = UploadState {
        vault_id,
        upload_id: upload_id.clone(),
        file_meta,
        chunks: Vec::with_capacity(expected_chunks),
        expected_chunks,
        received_chunks: 0,
        created_at: current_time,
    };

    // 5. Store upload state in memory
    ACTIVE_UPLOADS.with(|map| {
        map.borrow_mut().insert(upload_id.clone(), state);
    });

    ic_cdk::print(format!(
        "📝 INFO: Begin upload {} for vault {} initiated by {}. Expecting {} chunks.",
        upload_id, state.vault_id, caller, expected_chunks
    ));

    Ok(upload_id)
}

/// Uploads a single chunk for an ongoing session.
///
/// # Arguments
/// * `upload_id` - The ID of the upload session.
/// * `chunk_index` - The 0-based index of the chunk being uploaded.
/// * `data` - The byte data of the chunk.
///
/// # Returns
/// * `Result<(), VaultError>` - Success or an error.
pub fn upload_next_chunk(
    upload_id: &UploadId,
    chunk_index: u32,
    data: Vec<u8>,
) -> Result<(), VaultError> {
    ACTIVE_UPLOADS.with(|map| {
        let mut active_map = map.borrow_mut();
        let state = active_map
            .get_mut(upload_id)
            .ok_or_else(|| VaultError::UploadError("Upload session not found".to_string()))?;

        // 1. Validate chunk index
        let expected_index = state.received_chunks as u32;
        if chunk_index != expected_index {
            return Err(VaultError::UploadChunkOutOfOrder);
        }
        if chunk_index as usize >= state.expected_chunks {
             return Err(VaultError::UploadError("Chunk index exceeds expected count".to_string()));
        }

        // 2. Validate chunk size
        if data.len() > MAX_CHUNK_SIZE_BYTES {
            return Err(VaultError::UploadError(format!(
                "Chunk size {} exceeds limit {}",
                data.len(), MAX_CHUNK_SIZE_BYTES
            )));
        }
        // Check last chunk size if possible
        if (chunk_index as usize == state.expected_chunks - 1) {
            let expected_last_chunk_size = state.file_meta.size_bytes as usize % MAX_CHUNK_SIZE_BYTES;
            if expected_last_chunk_size > 0 && data.len() != expected_last_chunk_size {
                 return Err(VaultError::UploadError("Incorrect size for the last chunk".to_string()));
            }
        }

        // 3. Store chunk (in memory for now)
        state.chunks.push(data);
        state.received_chunks += 1;

        ic_cdk::print(format!(
            "📝 INFO: Received chunk {}/{} for upload {}",
            state.received_chunks, state.expected_chunks, upload_id
        ));

        Ok(())
    })
}

/// Finalizes an upload after all chunks are received.
/// Verifies checksum and commits the content item to stable storage.
///
/// # Arguments
/// * `upload_id` - The ID of the upload session.
/// * `sha256_checksum` - The SHA256 checksum of the complete file, calculated client-side.
///
/// # Returns
/// * `Result<ContentId, VaultError>` - The ID of the newly created content item or an error.
pub fn finish_chunked_upload(
    upload_id: &UploadId,
    sha256_checksum: String,
) -> Result<ContentId, VaultError> {
    // 1. Retrieve and remove upload state from memory
    let state = ACTIVE_UPLOADS.with(|map| {
        map.borrow_mut()
            .remove(upload_id)
            .ok_or_else(|| VaultError::UploadError("Upload session not found or already finished".to_string()))
    })?;

    // 2. Verify all chunks received
    if state.received_chunks != state.expected_chunks {
        // Put state back if validation fails? Or just fail?
        // ACTIVE_UPLOADS.with(|map| map.borrow_mut().insert(upload_id.clone(), state));
        return Err(VaultError::UploadError(format!(
            "Upload not complete. Received {} out of {} expected chunks.",
            state.received_chunks, state.expected_chunks
        )));
    }

    // 3. Reconstruct and verify checksum
    let mut hasher = Sha256::new();
    let mut total_bytes = 0;
    for chunk in &state.chunks {
        hasher.update(chunk);
        total_bytes += chunk.len();
    }
    let calculated_checksum = format!("{:x}", hasher.finalize());

    if calculated_checksum != sha256_checksum {
        return Err(VaultError::UploadError(
            "Checksum mismatch. Calculated vs Provided.".to_string(), // Avoid logging checksums
        ));
    }
    if total_bytes as u64 != state.file_meta.size_bytes {
         return Err(VaultError::UploadError(format!(
            "Total size mismatch. Reconstructed {} vs expected {}",
            total_bytes, state.file_meta.size_bytes
        )));
    }

    // 4. Concatenate chunks into the final payload
    // For large files, this might exceed Wasm memory limits.
    // A more robust solution would write directly to stable storage or use blob store.
    let final_payload: Vec<u8> = state.chunks.concat();

    // 5. Create VaultContentItem
    let content_id = generate_ulid();
    let current_time = time();
    let content_item = VaultContentItem {
        content_id: content_id.clone(),
        vault_id: state.vault_id.clone(),
        content_type: state.file_meta.content_type,
        title: state.file_meta.title,
        description: state.file_meta.description,
        created_at: current_time,
        updated_at: current_time,
        payload: final_payload,
        payload_size_bytes: state.file_meta.size_bytes,
        payload_sha256: Some(calculated_checksum),
    };

    // 6. Store VaultContentItem
    CONTENT_ITEMS.with(|map| {
        let key = StorableString(Cbor(content_id.clone()));
        let value = Cbor(content_item);
        map.borrow_mut().insert(key, value)
            .map_err(|e| VaultError::StorageError(format!("Failed to store content item: {:?}", e)))
    })?;

    // 7. Update Content Index
    CONTENT_INDEX.with(|map| {
        let key = StorableString(Cbor(state.vault_id.clone()));
        let mut borrowed_map = map.borrow_mut();
        let mut index = match borrowed_map.get(&key) {
            Some(storable_vec) => storable_vec.0, // Get inner Vec<String>
            None => Vec::new(),
        };
        index.push(content_id.clone());
        borrowed_map.insert(key, Cbor(index))
            .map_err(|e| VaultError::StorageError(format!("Failed to update content index: {:?}", e)))
    })?;

    // 8. TODO: Update VaultConfig storage_used_bytes
    // let _ = update_storage_usage(&state.vault_id, state.file_meta.size_bytes);

    ic_cdk::print(format!(
        "📝 INFO: Finished upload {}. Content item {} created for vault {}",
        upload_id, content_id, state.vault_id
    ));

    Ok(content_id)
}

// TODO: Add cleanup mechanism for stale/abandoned uploads in ACTIVE_UPLOADS map
// TODO: Function to update vault storage usage

"" 
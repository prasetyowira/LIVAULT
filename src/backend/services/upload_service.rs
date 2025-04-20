// src/backend/services/upload_service.rs
// Placeholder for chunked upload logic 

use crate::{
    error::VaultError,
    models::{common::*, VaultConfig, VaultContentItem},
    storage::{self, Cbor, StorableString, CONTENT_INDEX, CONTENT_ITEMS, VAULT_CONFIGS},
    utils::crypto::generate_ulid,
    services::vault_service, // For getting vault config
};
use ic_cdk::api::time;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::HashMap;
use hex; // For checksum comparison

type UploadId = String;
type ContentId = String;
const MAX_CHUNK_SIZE_BYTES: usize = 512 * 1024; // 512 KB
// Removed MAX_TOTAL_UPLOAD_SIZE_BYTES, will use vault quota

// Represents metadata provided when starting an upload
#[derive(Clone, Debug, candid::CandidType, serde::Deserialize)]
pub struct FileMeta {
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: u64,     // Total expected size
    pub content_type: ContentType, // Should be File, Password, or Letter
    pub title: Option<String>,
    // Removed description, assuming VaultContentItem handles it if needed
}

// In-memory state for an ongoing chunked upload
#[derive(Clone, Debug)]
pub struct UploadState {
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
    pub static ACTIVE_UPLOADS: RefCell<HashMap<UploadId, UploadState>> = RefCell::new(HashMap::new());
}

// --- Helper Functions ---

fn validate_mime_type(mime_type: &str, content_type: &ContentType) -> Result<(), VaultError> {
    match content_type {
        ContentType::File => {
            // Allowed MIME types for files based on prd.md
            let allowed_mimes = [
                "application/pdf",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document", // .docx
                "application/msword", // .doc
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", // .xlsx
                "application/vnd.ms-excel", // .xls
                "image/jpeg",
                "image/png",
                "text/plain",
                // Add more as needed
            ];
            if !allowed_mimes.contains(&mime_type) {
                return Err(VaultError::UploadError(format!(
                    "Disallowed MIME type '{}' for File content type.",
                    mime_type
                )));
            }
        }
        ContentType::Password | ContentType::Letter => {
            // Passwords and letters are essentially text, allow text/plain or specific internal type?
            if mime_type != "text/plain" && !mime_type.is_empty() { // Allow empty mime for simplicity?
                 return Err(VaultError::UploadError(format!(
                    "Invalid MIME type '{}' for {:?} content type. Expected 'text/plain' or empty.",
                    mime_type, content_type
                )));
            }
        }
    }
    Ok(())
}

/// Helper to update the vault's storage usage directly in stable storage.
fn update_vault_storage_usage(vault_id: &VaultId, bytes_added: u64) -> Result<(), VaultError> {
    storage::VAULT_CONFIGS.with(|map_ref| {
        let key = StorableString(Cbor(vault_id.clone()));
        let mut map = map_ref.borrow_mut();
        if let Some(mut config_cbor) = map.get(&key) {
            let mut config: VaultConfig = config_cbor.0;
            config.storage_used_bytes = config.storage_used_bytes.saturating_add(bytes_added);
            config.updated_at = time();
            // Insert the updated config back
            map.insert(key, Cbor(config)).map_err(|e| {
                VaultError::StorageError(format!("Failed to update vault storage usage: {:?}", e))
            })?;
             ic_cdk::print(format!(
                "💾 INFO: Updated vault {} storage usage by {} bytes.",
                vault_id, bytes_added
            ));
            Ok(())
        } else {
            Err(VaultError::VaultNotFound(vault_id.clone())) // Should not happen if called after get_vault_config
        }
    })
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
    caller: PrincipalId,
) -> Result<UploadId, VaultError> {
    // 1. Validate Vault and Permissions
    let vault_config = vault_service::get_vault_config(&vault_id)?;
    if vault_config.owner != caller { // Simplistic check, might need more roles
        return Err(VaultError::NotAuthorized("Caller cannot upload to this vault".to_string()));
    }
    // TODO: Add check for vault status allowing uploads (e.g., Active)

    // 2. Validate FileMeta
    if file_meta.size_bytes == 0 {
        return Err(VaultError::UploadError("File size cannot be zero".to_string()));
    }

    // 3. Check against vault quota
    let available_quota = vault_config.storage_quota_bytes.saturating_sub(vault_config.storage_used_bytes);
    if file_meta.size_bytes > available_quota {
        return Err(VaultError::StorageLimitExceeded(format!(
            "Upload size {} bytes exceeds available quota {} bytes (Used: {}, Total: {}).",
            file_meta.size_bytes,
            available_quota,
            vault_config.storage_used_bytes,
            vault_config.storage_quota_bytes
        )));
    }

    // 4. Validate MIME type based on ContentType
    validate_mime_type(&file_meta.mime_type, &file_meta.content_type)?;

    // 5. Calculate expected chunks
    let expected_chunks = (file_meta.size_bytes as usize + MAX_CHUNK_SIZE_BYTES - 1) / MAX_CHUNK_SIZE_BYTES;

    // 6. Create Upload State
    let upload_id = generate_ulid();
    let current_time = time();
    let state = UploadState {
        vault_id: vault_id.clone(), // Clone vault_id here
        upload_id: upload_id.clone(),
        file_meta,
        chunks: Vec::with_capacity(expected_chunks),
        expected_chunks,
        received_chunks: 0,
        created_at: current_time,
    };

    // 7. Store upload state in memory
    ACTIVE_UPLOADS.with(|map| {
        map.borrow_mut().insert(upload_id.clone(), state.clone()); // Clone state
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
            .ok_or_else(|| VaultError::UploadError("Upload session not found or expired".to_string()))?;

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
        // Check last chunk size
        if (chunk_index as usize == state.expected_chunks - 1) {
            let expected_last_chunk_size = if state.file_meta.size_bytes == 0 { // Avoid modulo by zero if file size is 0 (should be caught earlier)
                0
            } else {
                 state.file_meta.size_bytes as usize % MAX_CHUNK_SIZE_BYTES
            };
            // If expected_last_chunk_size is 0, it means the file size is a perfect multiple of MAX_CHUNK_SIZE_BYTES
            let correct_last_chunk_size = if expected_last_chunk_size == 0 {
                 MAX_CHUNK_SIZE_BYTES
            } else {
                expected_last_chunk_size
            };

            if data.len() != correct_last_chunk_size {
                 return Err(VaultError::UploadError(format!(
                    "Incorrect size for the last chunk. Expected {}, Got {}",
                    correct_last_chunk_size,
                    data.len()
                 )));
            }
        }

        // 3. Store chunk (in memory for now)
        // Ensure chunks are added in order. Since we check index, push is safe.
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
/// * `sha256_checksum_hex` - The SHA256 checksum of the complete file (hex encoded), calculated client-side.
///
/// # Returns
/// * `Result<ContentId, VaultError>` - The ID of the newly created content item or an error.
pub fn finish_chunked_upload(
    upload_id: &UploadId,
    sha256_checksum_hex: String,
) -> Result<ContentId, VaultError> {
    // 1. Retrieve and remove upload state from memory
    let state = ACTIVE_UPLOADS.with(|map| {
        map.borrow_mut()
            .remove(upload_id)
            .ok_or_else(|| VaultError::UploadError("Upload session not found, expired, or already finished".to_string()))
    })?;

    // 2. Verify all chunks received
    if state.received_chunks != state.expected_chunks {
        // Don't put state back, it's invalid.
        return Err(VaultError::UploadError(format!(
            "Upload not complete. Received {} out of {} expected chunks.",
            state.received_chunks, state.expected_chunks
        )));
    }

    // 3. Reconstruct file and verify checksum
    let mut hasher = Sha256::new();
    let mut total_bytes = 0;
    for chunk in &state.chunks {
        hasher.update(chunk);
        total_bytes += chunk.len();
    }
    let calculated_checksum = hasher.finalize();
    let calculated_checksum_hex = hex::encode(calculated_checksum);

    if calculated_checksum_hex != sha256_checksum_hex {
         // TODO: Consider how to handle this - maybe allow retry?
         return Err(VaultError::ChecksumMismatch);
    }

    // Verify total bytes match file meta
    if total_bytes as u64 != state.file_meta.size_bytes {
         return Err(VaultError::UploadError(format!(
            "Total uploaded bytes ({}) do not match expected size ({})",
            total_bytes, state.file_meta.size_bytes
        )));
    }

    // 4. Create VaultContentItem
    let content_id = generate_ulid();
    let current_time = time();
    let content_item = VaultContentItem {
        content_id: content_id.clone(),
        vault_id: state.vault_id.clone(),
        content_type: state.file_meta.content_type,
        title: state.file_meta.title,
        filename: Some(state.file_meta.filename), // Store original filename
        mime_type: Some(state.file_meta.mime_type),
        size_bytes: state.file_meta.size_bytes,
        created_at: current_time,
        updated_at: current_time,
        payload_checksum: Some(sha256_checksum_hex),
        // The actual payload (concatenated chunks) is stored separately or managed by asset canister
        // For now, let's assume payload refers to the encrypted data blob.
        // If using stable memory directly, we store concatenated chunks here.
        // NOTE: Storing large blobs directly in stable memory BTree value might be inefficient.
        // Consider alternative storage patterns (StableVec, chunking in stable mem, etc.)
        payload: state.chunks.concat(), // Concatenate chunks for storage
    };

    // 5. Store Content Item
    storage::CONTENT_ITEMS.with(|map| {
        let key = StorableString(Cbor(content_id.clone()));
        let value = Cbor(content_item.clone()); // Clone needed for index update
        map.borrow_mut()
            .insert(key, value)
            .map_err(|e| VaultError::StorageError(format!("Failed to store content item: {:?}", e)))
    })?;

    // 6. Update Content Index for the vault
    storage::CONTENT_INDEX.with(|map| {
        let key = StorableString(Cbor(state.vault_id.clone()));
        let mut index_list = match map.borrow().get(&key) {
            Some(list_cbor) => list_cbor.0,
            None => Vec::new(),
        };
        index_list.push(content_id.clone());
        map.borrow_mut()
            .insert(key, Cbor(index_list))
            .map_err(|e| VaultError::StorageError(format!("Failed to update content index: {:?}", e)))
    })?;

    // 7. Update Vault Storage Usage
    update_vault_storage_usage(&state.vault_id, state.file_meta.size_bytes)?;

    ic_cdk::print(format!(
        "✅ INFO: Finished upload {} for vault {}. New content item {}. Size: {}",
        upload_id, state.vault_id, content_id, state.file_meta.size_bytes
    ));

    Ok(content_id)
}

// TODO: Add function to get content item details
// TODO: Add function to delete content item (requires updating index and storage usage)
// TODO: Add function to list content items for a vault (using the index)
// TODO: Add cleanup for stale/abandoned uploads (maybe in scheduler?)
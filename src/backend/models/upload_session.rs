use crate::models::common::{Timestamp, VaultId, UploadId, PrincipalId};
use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};

#[derive(CandidType, Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Copy)]
pub enum UploadStatus {
    Initiated, // Session created, waiting for chunks
    Uploading, // Chunks are being received
    Completed, // All chunks received, ready for finalization/processing
    Failed,    // An error occurred during upload
    Aborted,   // Upload cancelled by user
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub struct UploadSession {
    pub upload_id: UploadId,          // Exposed Principal ID for this session
    pub vault_id: VaultId,            // Target vault
    pub initiator: PrincipalId,       // Principal who started the upload
    pub filename: String,
    pub mime_type: String,
    pub expected_size_bytes: u64,
    pub received_bytes: u64,
    pub expected_chunk_count: u64,
    pub received_chunk_count: u64,
    pub status: UploadStatus,
    pub created_at: Timestamp,
    pub last_chunk_received_at: Option<Timestamp>,
    // Optional: Store chunk hashes or other metadata if needed
    // pub chunk_hashes: Vec<Vec<u8>>,
}

impl Default for UploadSession {
    fn default() -> Self {
        Self {
            upload_id: Principal::anonymous(),
            vault_id: Principal::anonymous(),
            initiator: Principal::anonymous(),
            filename: String::new(),
            mime_type: String::new(),
            expected_size_bytes: 0,
            received_bytes: 0,
            expected_chunk_count: 0,
            received_chunk_count: 0,
            status: UploadStatus::Initiated,
            created_at: 0,
            last_chunk_received_at: None,
        }
    }
} 
// src/backend/models/vault_content_item.rs
use crate::models::common::{ContentId, ContentType, Timestamp, VaultId};
use candid::CandidType;
use serde::{Deserialize, Serialize};

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub struct VaultContentItem {
    // Internal ID, used as primary key in storage, NOT exposed in API directly
    #[serde(skip_serializing)]
    pub internal_id: u64,

    // Exposed ID (Principal)
    pub content_id: ContentId,
    pub vault_id: VaultId,
    pub content_type: ContentType,
    pub title: Option<String>,     // User-provided title, e.g., "Last Will" or "Bank Passwords"
    pub description: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub payload: Vec<u8>, // file: file blob, Password and Letter: json string in Vec<u8>
    pub payload_size_bytes: u64,
    pub payload_sha256: Option<String>, // Optional checksum for verification
}
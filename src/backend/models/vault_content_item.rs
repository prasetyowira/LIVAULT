// src/backend/models/vault_content_item.rs
use crate::models::common::{ContentId, ContentType, Timestamp, VaultId};
use candid::CandidType;
use serde::{Deserialize, Serialize};

#[derive(CandidType, Deserialize, Serialize, Clone, Debug, Default)]
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
    pub payload: Vec<u8>, // The client-side encrypted content blob
    pub payload_size_bytes: u64,
    pub payload_sha256: Option<String>, // Optional checksum for verification
}

// Implement Default if needed
/*
impl Default for VaultContentItem {
    fn default() -> Self {
        Self {
            content_id: String::new(),
            vault_id: String::new(),
            content_type: ContentType::File,
            title: None,
            description: None,
            created_at: 0,
            updated_at: 0,
            payload: Vec::new(),
            payload_size_bytes: 0,
            payload_sha256: None,
        }
    }
}
*/ 
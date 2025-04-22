// src/backend/models/vault_invite_token.rs
use crate::models::common::{InviteStatus, InviteTokenId, Role, Timestamp, VaultId};
use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};

// Vault Invite Token Status
#[derive(CandidType, Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub enum TokenStatus {
    Issued,
    Claimed,
    Expired,
    Revoked, // Add revoked status
}

impl Default for TokenStatus {
    fn default() -> Self {
        TokenStatus::Issued
    }
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
pub struct VaultInviteToken {
    // Internal ID, used as primary key in storage, NOT exposed in API directly
    #[serde(skip_serializing)] // Skip serialization if never needed externally
    pub internal_id: u64,

    // Exposed ID (Principal), used in API - Type alias resolves to Principal
    pub token_id: InviteTokenId,
    pub vault_id: VaultId, // Now Principal
    pub role: Role,
    pub status: TokenStatus,
    pub created_at: Timestamp,
    pub expires_at: Timestamp,
    pub claimed_at: Option<Timestamp>,
    pub claimed_by: Option<Principal>,
    pub shamir_share_index: u8,
    pub share_data: Vec<u8>,    // Serialized Shamir share data
}

// Implement Default for easier initialization if needed
impl Default for VaultInviteToken {
    fn default() -> Self {
        Self {
            internal_id: 0,
            token_id: InviteTokenId::anonymous(),
            vault_id: VaultId::anonymous(),
            role: Role::Heir,
            status: TokenStatus::Issued,
            created_at: 0,
            expires_at: 0,
            claimed_at: None,
            claimed_by: None,
            shamir_share_index: 0, // Default to 0, must be assigned properly
            share_data: Vec::new(), // Default to empty vec
        }
    }
}

/* // Removed old Default implementation, using derive now
impl Default for VaultInviteToken {
    fn default() -> Self {
        Self {
            token_id: String::new(),
            vault_id: String::new(),
            role: Role::Heir,
            shamir_share_index: 0,
            status: InviteStatus::Pending,
            created_at: 0,
            expires_at: 0,
            claimed_at: None,
            claimed_by: None,
        }
    }
}
*/ 
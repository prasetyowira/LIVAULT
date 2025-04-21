// src/backend/models/vault_invite_token.rs
use crate::models::common::{InviteStatus, InviteTokenId, Role, Timestamp, VaultId};
use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, Default)]
pub struct VaultInviteToken {
    // Internal ID, used as primary key in storage, NOT exposed in API directly
    #[serde(skip_serializing)] // Skip serialization if never needed externally
    pub internal_id: u64,

    // Exposed ID (Principal), used in API - Type alias resolves to Principal
    pub token_id: InviteTokenId,
    pub vault_id: VaultId, // Now Principal
    pub role: Role,
    pub status: InviteStatus,
    pub created_at: Timestamp,
    pub expires_at: Timestamp,
    pub claimed_at: Option<Timestamp>,
    pub claimed_by: Option<Principal>,
    pub shamir_share_index: u8,
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
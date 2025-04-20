// src/backend/models/vault_invite_token.rs
use crate::models::common::{InviteStatus, InviteTokenId, Role, Timestamp, VaultId};
use candid::CandidType;
use serde::{Deserialize, Serialize};

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub struct VaultInviteToken {
    pub token_id: InviteTokenId, // The unique token string (e.g., ULID or random bytes encoded)
    pub vault_id: VaultId,
    pub role: Role,
    pub shamir_share_index: u8, // Assigned share index for the invitee (1-255)
    pub status: InviteStatus,
    pub created_at: Timestamp,
    pub expires_at: Timestamp, // Typically 24 hours from creation
    pub claimed_at: Option<Timestamp>,
    pub claimed_by: Option<candid::Principal>, // Principal of the user who claimed it
}

// Implement Default if needed for initialization scenarios
impl Default for VaultInviteToken {
    fn default() -> Self {
        Self {
            token_id: String::new(),
            vault_id: String::new(),
            role: Role::Heir,
            shamir_share_index: 0, // Should be assigned properly
            status: InviteStatus::Pending,
            created_at: 0,
            expires_at: 0, // Should be calculated (e.g., now + 24h)
            claimed_at: None,
            claimed_by: None,
        }
    }
} 
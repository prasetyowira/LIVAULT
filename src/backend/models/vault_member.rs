// src/backend/models/vault_member.rs
use crate::models::common::{MemberId, MemberStatus, PrincipalId, Role, Timestamp, VaultId};
use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub struct AccessControl {
    pub last_accessed_at: Option<Timestamp>,
    pub download_limit_per_day: u8, // e.g., 3 as per PRD
    pub daily_downloads_count: u8,
    pub last_download_day_index: u64, // To track daily reset
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub struct VaultMember {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_id: Option<u64>,

    pub member_id: MemberId,
    pub vault_id: VaultId,
    pub role: Role,
    pub status: MemberStatus,
    pub name: Option<String>,      // Set during invite claim
    pub relation: Option<String>,  // Set during invite claim
    pub email: Option<String>,  // Set during invite claim
    pub shamir_share_index: u8,    // Assigned during invite generation (1-255)
    pub added_at: Timestamp,
    pub updated_at: Timestamp,
    pub access_control: AccessControl, // Manages access after unlock
    pub has_approved_unlock: bool, // Track approval status
}

#[derive(Clone, Debug, candid::CandidType, serde::Deserialize, serde::Serialize)]
pub struct MemberProfile {
    pub member_id: MemberId,
    pub vault_id: VaultId,
    pub principal: PrincipalId,
    pub role: Role,
    pub status: MemberStatus,
    pub shamir_share_index: u8,
    pub name: Option<String>,
    pub relation: Option<String>,
    pub added_at: Timestamp,
}

// Implement Default if needed
impl Default for VaultMember {
    fn default() -> Self {
        Self {
            internal_id: None,
            member_id: Principal::anonymous(),
            vault_id: Principal::anonymous(),
            role: Role::Heir,
            status: MemberStatus::Pending,
            name: None,
            relation: None,
            email: None,
            shamir_share_index: 0, // Should be assigned properly
            added_at: 0,
            updated_at: 0,
            access_control: AccessControl {
                last_accessed_at: None,
                download_limit_per_day: 3,
                daily_downloads_count: 0,
                last_download_day_index: 0,
            },
            has_approved_unlock: false,
        }
    }
}

impl Default for AccessControl {
    fn default() -> Self {
        Self {
            last_accessed_at: None,
            download_limit_per_day: 3,
            daily_downloads_count: 0,
            last_download_day_index: 0,
        }
    }
}

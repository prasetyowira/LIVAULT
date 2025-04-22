// src/backend/models/vault_config.rs
use crate::models::common::{PrincipalId, Timestamp, VaultId, VaultStatus};
use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};

/// Represents the main configuration and state of a LiVault.
#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub struct VaultConfig {
    pub vault_id: VaultId,
    pub owner: PrincipalId,
    pub name: String,
    pub description: Option<String>,
    pub status: VaultStatus,
    pub plan: String, // e.g., "Basic", "Premium"
    pub storage_quota_bytes: u64,
    pub storage_used_bytes: u64,
    pub unlock_conditions: UnlockConditions,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub expires_at: Timestamp, // Calculated at creation (e.g., 10 years)
    pub unlocked_at: Option<Timestamp>,
    pub last_accessed_by_owner: Option<Timestamp>, // Track owner activity
}

/// Defines the conditions required to unlock a vault.
#[derive(CandidType, Deserialize, Serialize, Clone, Debug, Default)]
pub struct UnlockConditions {
    /// Specific epoch timestamp (seconds) when the vault becomes unlockable.
    pub time_based_unlock_epoch_sec: Option<u64>,
    /// Duration of owner inactivity (seconds) after which the vault becomes unlockable.
    pub inactivity_duration_sec: Option<u64>,
    /// Number of heir approvals required.
    pub required_heir_approvals: Option<u32>,
    /// Number of witness approvals required.
    pub required_witness_approvals: Option<u32>,
    // TODO: Add field for recovery QR configuration if needed
    // pub recovery_qr_config: Option<RecoveryQrConfig>,
}

/// Represents the counts of approvals received.
#[derive(CandidType, Deserialize, Serialize, Clone, Debug, Default)]
pub struct ApprovalCounts {
    pub heir_approvals: u32,
    pub witness_approvals: u32,
}

// Example Recovery QR Config (if needed later)
// #[derive(Clone, Debug, Default, CandidType, Serialize, Deserialize, PartialEq)]
// pub struct RecoveryQrConfig {
//     pub enabled: bool,
//     /// If true, QR bypasses the threshold check.
//     pub bypass_threshold: bool,
//     /// If true, QR is only valid if no heirs/witnesses have joined.
//     pub valid_if_no_members: bool,
// }

// Implement Default if needed for initialization
impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            vault_id: Principal::anonymous(),
            owner: Principal::anonymous(),
            name: String::from("My Vault"),
            description: None,
            status: VaultStatus::Draft,
            plan: String::from("Basic"),
            storage_quota_bytes: 5 * 1024 * 1024, // Example: 5MB
            storage_used_bytes: 0,
            unlock_conditions: UnlockConditions::default(),
            created_at: 0,
            updated_at: 0,
            expires_at: 0, // Needs proper calculation
            unlocked_at: None,
            last_accessed_by_owner: None,
            // schema_version: 1,
        }
    }
}

impl PartialEq for UnlockConditions {
    fn eq(&self, other: &Self) -> bool {
        self.time_based_unlock_epoch_sec == other.time_based_unlock_epoch_sec
            && self.inactivity_duration_sec == other.inactivity_duration_sec
            && self.required_heir_approvals == other.required_heir_approvals
            && self.required_witness_approvals == other.required_witness_approvals
    }
    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

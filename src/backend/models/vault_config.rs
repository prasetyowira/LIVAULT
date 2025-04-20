// src/backend/models/vault_config.rs
use crate::models::common::{PrincipalId, Timestamp, VaultId, VaultStatus};
use candid::CandidType;
use serde::{Deserialize, Serialize};

// Define unlock condition sub-structs if needed, or keep simple for now
#[derive(CandidType, Deserialize, Serialize, Clone, Debug, Default)]
pub struct UnlockConditions {
    pub time_based_unlock_epoch_sec: Option<Timestamp>,
    pub inactivity_duration_sec: Option<u64>, // e.g., 90 days = 90 * 24 * 60 * 60
    pub required_heir_approvals: u8,
    pub required_witness_approvals: u8,
    // pub recovery_qr_config: Option<RecoveryQrConfig>, // Add later if complex
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub struct VaultConfig {
    pub vault_id: VaultId,
    pub owner: PrincipalId,
    pub name: String,
    pub description: Option<String>,
    pub status: VaultStatus,
    pub plan: String, // e.g., "Basic", "Standard"
    pub storage_quota_bytes: u64,
    pub storage_used_bytes: u64,
    pub unlock_conditions: UnlockConditions,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub expires_at: Timestamp, // Calculated based on plan/creation (e.g., 10 years)
    pub unlocked_at: Option<Timestamp>,
    pub last_accessed_by_owner: Option<Timestamp>,
    // pub schema_version: u16, // For future migrations
}

// Implement Default if needed for initialization
impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            vault_id: String::new(),
            owner: PrincipalId::anonymous(),
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
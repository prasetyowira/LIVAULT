use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};

/// Represents a single entry in the audit log for a vault.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AuditLogEntry {
    /// Nanoseconds since epoch.
    pub timestamp: u64,
    /// Principal ID of the actor performing the action.
    pub actor: Principal,
    /// The specific action performed.
    pub action: LogAction,
    /// Optional details about the action (e.g., target item ID, invitee email).
    pub details: Option<String>,
    /// The vault this log entry pertains to.
    pub vault_id: String,
}

/// Enum representing the different types of actions that can be logged.
#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum LogAction {
    VaultCreated,
    VaultUpdated,
    VaultUnlocked,
    VaultExpired,
    VaultDeleted,
    MemberInvited,
    MemberJoined,
    MemberRemoved,
    MemberApprovedUnlock,
    ContentUploaded,
    ContentDownloaded,
    ContentDeleted,
    InviteGenerated,
    InviteClaimed,
    InviteRevoked,
    PaymentVerified,
    MaintenanceRun,
    // Add more actions as needed
} 
// src/backend/error.rs
use candid::{CandidType, Deserialize};
use thiserror::Error;
use crate::models::common::{VaultId, InviteTokenId, MemberId, ContentId};

#[derive(CandidType, Deserialize, Error, Debug, PartialEq, Eq)]
pub enum VaultError {
    #[error("Not authorized: {0}")]
    NotAuthorized(String),

    #[error("Object not found: {0}")]
    NotFound(String),

    #[error("Vault not found: {0}")]
    VaultNotFound(String),

    #[error("Token expired or invalid: {0}")]
    TokenInvalid(String),

    #[error("Payment error: {0}")]
    PaymentError(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Upload error: {0}")]
    UploadError(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Internal canister error: {0}")]
    InternalError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("HTTP outcall error: {0}")]
    HttpError(String),

    // Add more specific errors based on PRD and tech docs
    #[error("Approval quorum not met")]
    ApprovalQuorumNotMet,

    #[error("Storage limit exceeded for plan")]
    StorageLimitExceeded,

    #[error("Recovery QR cannot be used after setup completion")]
    RecoveryQrBlockedPostSetup,

    #[error("Upload chunk out of order or invalid index")]
    UploadChunkOutOfOrder,

    #[error("Canister cycle balance too low for operation")]
    CycleLow(u128),

    // Phase 5: Security & Guards specific
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Caller is not the designated admin principal")]
    AdminGuardFailed,

    // Phase 6: Metrics & Admin APIs specific
    #[error("Billing record not found")]
    BillingRecordNotFound,

    // Other potential errors
    #[error("Invalid vault state for operation: {0}")]
    InvalidState(String),

    #[error("Member not found: {0}")]
    MemberNotFound(String),

    #[error("Checksum mismatch during upload finalization")]
    ChecksumMismatch,

    #[error("Vault already exists")]
    AlreadyExists(VaultId),

    #[error("Invite token not found")]
    InviteNotFound,

    #[error("Invite token has expired")]
    InviteExpired,

    #[error("Invite token has already been claimed")]
    InviteAlreadyClaimed,

    #[error("Invite token is not in pending state")]
    InviteNotPending,

    #[error("Principal is already a member of the vault")]
    AlreadyMember,

    #[error("Content item not found in vault")]
    ContentNotFound(ContentId),

    #[error("Quota exceeded")]
    QuotaExceeded,

    #[error("Ledger interaction error")]
    LedgerError(String),

    #[error("Invalid state transition requested: {0}")]
    InvalidStateTransition(String),

    #[error("Vault is not in an unlockable state")]
    NotUnlockable,

    #[error("Vault unlock conditions have not been met")]
    UnlockConditionsNotMet,
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VaultError::AlreadyExists(id) => write!(f, "Vault '{}' already exists", id),
            VaultError::VaultNotFound(id) => write!(f, "Vault '{}' not found", id),
            VaultError::InviteNotFound => write!(f, "Invite token not found"),
            VaultError::InviteExpired => write!(f, "Invite token has expired"),
            VaultError::InviteAlreadyClaimed => write!(f, "Invite token has already been claimed"),
            VaultError::InviteNotPending => write!(f, "Invite token is not in pending state"),
            VaultError::AlreadyMember => write!(f, "Principal is already a member of the vault"),
            VaultError::MemberNotFound(id) => write!(f, "Member '{}' not found in vault", id),
            VaultError::ContentNotFound(id) => write!(f, "Content item '{}' not found in vault", id),
            VaultError::StorageLimitExceeded => write!(f, "Vault storage limit exceeded"),
            VaultError::QuotaExceeded => write!(f, "Operation quota exceeded"),
            VaultError::PaymentError(s) => write!(f, "Payment processing error: {}", s),
            VaultError::LedgerError(s) => write!(f, "Ledger interaction error: {}", s),
            VaultError::UploadError(s) => write!(f, "Upload error: {}", s),
            VaultError::UploadChunkOutOfOrder => write!(f, "Upload chunk received out of order"),
            VaultError::StorageError(s) => write!(f, "Stable storage error: {}", s),
            VaultError::NotAuthorized(s) => write!(f, "Authorization failed: {}", s),
            VaultError::InvalidInput(s) => write!(f, "Invalid input: {}", s),
            VaultError::InvalidStateTransition => write!(f, "Invalid state transition requested"),
            VaultError::InternalError(s) => write!(f, "Internal canister error: {}", s),
            VaultError::SerializationError(s) => write!(f, "Serialization error: {}", s),
            VaultError::HttpError(s) => write!(f, "HTTP outcall error: {}", s),
            VaultError::NotUnlockable => write!(f, "Vault is not in an unlockable state"),
            VaultError::UnlockConditionsNotMet => write!(f, "Vault unlock conditions have not been met"),
        }
    }
} 
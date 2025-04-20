// src/backend/error.rs
use candid::CandidType;
use serde::Deserialize;
use thiserror::Error;

#[derive(CandidType, Deserialize, Error, Debug, PartialEq, Eq)]
pub enum VaultError {
    #[error("Not authorized: {0}")]
    NotAuthorized(String),

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
    CycleLow,

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
} 
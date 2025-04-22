// src/backend/models/common.rs
use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};
use std::time::Duration;

// Define ID types using Principal where appropriate
pub type VaultId = Principal;        // Unique identifier for a vault
pub type MemberId = Principal;       // Principal of a vault member (owner, heir, witness)
pub type InviteTokenId = Principal;  // Exposed unique ID for an invite token
pub type ContentId = Principal;      // Exposed unique ID for a content item
pub type UploadId = Principal;       // Exposed unique ID for an upload session
pub type PrincipalId = Principal;    // General Principal identifier (can keep for clarity or remove if redundant)

pub type Timestamp = u64; // Epoch seconds
pub type TimestampNs = u64; // Nanoseconds since epoch
pub type StorageBytes = u64;
pub type Cycles = u128;
pub type InternalId = u64; // Internal counter/ID for storage
pub type ShamirShareIndex = u8; // 1-based index for Shamir shares

#[derive(CandidType, Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Copy)]
pub enum VaultStatus {
    Draft,          // Initial state upon creation, before payment confirmation
    NeedSetup,      // Payment confirmed, vault needs content and member setup
    SetupComplete,  // Owner finalized setup, invite tokens might still be pending claim
    Active,         // Vault is active and operational, expiry countdown may start
    GraceMaster,    // Expiry date reached, 14-day grace for master user action
    GraceHeir,      // Master grace period passed, 14-day grace for heirs/witnesses
    Unlockable,     // Unlock conditions met (quorum/time/inactivity), content accessible to heirs
    Unlocked,       // Vault has been explicitly unlocked by heirs/witnesses
    Expired,        // Unlock window passed, or grace period ended without renewal/unlock
    Deleted,        // Vault permanently deleted after expiry/purge
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Copy)]
pub enum Role {
    Master,
    Heir,
    Witness,
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Copy)]
pub enum ContentType {
    File,
    Password,
    Letter,
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Copy)]
pub enum InviteStatus {
    Pending,
    Claimed,
    Expired,
    Revoked,
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Copy)]
pub enum MemberStatus {
    Pending, // Invite claimed, but maybe needs confirmation?
    Verified, // Member confirmed/verified (e.g., after claiming invite)
    Active,
    Revoked, // Access revoked by master
}

// TODO: Define specific storage plan tiers if needed 
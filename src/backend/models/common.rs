// src/backend/models/common.rs
use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};

pub type VaultId = String;
pub type MemberId = String;
pub type InviteTokenId = String;
pub type ContentId = String;
pub type Timestamp = u64; // Epoch seconds
pub type PrincipalId = Principal;

#[derive(CandidType, Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Copy)]
pub enum VaultStatus {
    Draft,          // Initial state upon creation, before payment confirmation
    NeedSetup,      // Payment confirmed, vault needs content and member setup
    SetupComplete,  // Owner finalized setup, invite tokens might still be pending claim
    Active,         // Vault is active and operational, expiry countdown may start
    GraceMaster,    // Expiry date reached, 14-day grace for master user action
    GraceHeir,      // Master grace period passed, 14-day grace for heirs/witnesses
    Unlockable,     // Unlock conditions met, content accessible to heirs
    Expired,        // Unlock window passed, content no longer accessible
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
    Active,
    Revoked, // Access revoked by master
} 
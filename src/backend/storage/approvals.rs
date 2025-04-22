// src/backend/storage/approvals.rs

use crate::models::{common::*, vault_config::ApprovalCounts}; // Use VaultId, PrincipalId etc.
use crate::error::VaultError;
use crate::storage::{
    storable::Cbor,
    memory::{get_approvals_memory, Memory},
};
use ic_stable_structures::{StableBTreeMap};
use std::cell::RefCell;

type ApprovalsMap = StableBTreeMap<VaultId, Cbor<ApprovalCounts>, Memory>;

thread_local! {
    /// Stable storage for vault approval counts.
    /// Key: VaultId (Principal)
    /// Value: Cbor<ApprovalCounts>
    static APPROVALS: RefCell<ApprovalsMap> = RefCell::new(
        ApprovalsMap::init(get_approvals_memory())
    );
}

/// Stores or updates the approval counts for a vault.
pub fn update_approval_counts(vault_id: &VaultId, counts: ApprovalCounts) -> Result<(), VaultError> {
    APPROVALS.with(|map_ref| {
        map_ref.borrow_mut().insert(*vault_id, Cbor(counts));
    });
    Ok(())
}

/// Retrieves the current approval status (counts) for a vault.
/// Returns default counts (0) if no record exists.
pub async fn get_approval_status(vault_id: &VaultId) -> Result<ApprovalCounts, VaultError> {
    let counts = APPROVALS.with(|map_ref| {
        map_ref.borrow().get(vault_id).map(|c| c.0)
    });
    Ok(counts.unwrap_or_default()) // Return default (0 counts) if not found
}

/// Removes the approval record for a vault during deletion.
pub async fn remove_approvals(vault_id: &VaultId) -> Result<(), VaultError> {
    APPROVALS.with(|map_ref| {
        map_ref.borrow_mut().remove(vault_id);
    });
    Ok(())
}

/// Records an approval for a specific role within a vault.
/// Increments the corresponding counter.
pub async fn record_approval(vault_id: &VaultId, role: Role) -> Result<(), VaultError> {
    let mut counts = get_approval_status(vault_id).await?;
    match role {
        Role::Heir => counts.heir_approvals = counts.heir_approvals.saturating_add(1),
        Role::Witness => counts.witness_approvals = counts.witness_approvals.saturating_add(1),
        _ => return Err(VaultError::InvalidInput("Cannot record approval for Master or Admin role".to_string())),
    }
    update_approval_counts(vault_id, counts)
} 
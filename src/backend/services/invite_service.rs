// src/backend/services/invite_service.rs
// Placeholder for Invite token generation and management 

use crate::{
    error::VaultError,
    models::{
        common::*,
        VaultInviteToken,
        VaultMember,
        VaultConfig, // Needed for state check
        MemberRole, // Add MemberRole
        MemberStatus,
    },
    storage::{self, create_member_key, Cbor, StorableString, get_vault_member_prefix, tokens as token_storage, vault_members as member_storage, content as content_storage},
    services::vault_service, // Needed for state updates
    utils::crypto::{generate_ulid, generate_unique_principal}, // Import generate_ulid and generate_unique_principal
};
use ic_cdk::api::{time, management_canister::main::raw_rand}; // Import raw_rand
use std::collections::BTreeSet; // For checking used Shamir indices
use std::time::Duration;
use hex;
use candid::{Principal as PrincipalId, CandidType}; // Import PrincipalId and CandidType
use serde::{Deserialize, Serialize}; // Import Deserialize & Serialize
use crate::models::member::MemberProfileData;
use crate::models::invite::InviteStatus;
use crate::models::member::MemberStatus;
use crate::models::vault_member::AccessInfo;
use crate::models::vault_member::AccessControl;

// Placeholder for profile data returned after claiming invite
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

// Placeholder for invite claim data from API
#[derive(Clone, Debug, candid::CandidType, serde::Deserialize)]
pub struct InviteClaimData {
    pub name: Option<String>,
    pub relation: Option<String>,
    // Passphrase handling is client-side/off-chain primarily
}

// --- Helper Functions ---

/// Retrieves all members for a specific vault.
/// Iterates through the stable storage based on the vault ID prefix.
fn get_members_for_vault(vault_id: &VaultId) -> Result<Vec<VaultMember>, VaultError> {
    let mut members = Vec::new();
    let prefix = get_vault_member_prefix(vault_id);

    storage::VAULT_MEMBERS.with(|map_ref| {
        let map = map_ref.borrow();
        // Iterate over keys starting with the prefix
        // Note: This requires careful handling of key encoding and iteration range.
        // Assuming StorableString encodes simply for prefix matching.
        // This is a basic iteration, performance might degrade with many members.
        for (_key_bytes, value) in map.iter() {
            // This check is inefficient as iter() goes through the whole map.
            // A proper range scan or prefix scan method would be better if available.
            // For now, we filter after getting the value.
            let member: VaultMember = value.0;
            if member.vault_id == *vault_id {
                members.push(member);
            }
        }
    });
    Ok(members)
}

// --- Service Functions ---

/// Generates a new invite token for a specific vault and role.
/// Assigns the next available Shamir share index.
/// Uses secure randomness from the IC for the token's Principal ID.
///
/// # Arguments
/// * `vault_id` - The ID of the vault to invite to.
/// * `role` - The role (`Heir` or `Witness`) of the invitee.
/// * `caller` - The principal initiating the invite (must be vault owner).
///
/// # Returns
/// * `Result<VaultInviteToken, VaultError>` - The generated invite token or an error.
pub async fn generate_new_invite(
    vault_id: VaultId, // Pass Principal directly
    role: Role,
    caller: PrincipalId,
) -> Result<VaultInviteToken, VaultError> {
    // 1. Get Vault Config and check authorization
    let vault_config = vault_service::get_vault_config(&vault_id)?;
    if vault_config.owner != caller {
        return Err(VaultError::NotAuthorized(
            "Only the vault owner can generate invites.".to_string(),
        ));
    }

    // 2. Check if vault is in a state that allows invites
    if vault_config.status != VaultStatus::Active && vault_config.status != VaultStatus::SetupComplete && vault_config.status != VaultStatus::NeedSetup {
        return Err(VaultError::InternalError(format!(
            "Vault status {:?} does not allow generating invites.",
            vault_config.status
        )));
    }

    // 3. Determine next available Shamir index
    let existing_members = get_members_for_vault(&vault_id)?;
    let used_indices: BTreeSet<u8> = existing_members
        .iter()
        .map(|m| m.shamir_share_index)
        .collect();

    let shamir_share_index = (1..=255) // Shamir index 0 is typically reserved
        .find(|i| !used_indices.contains(i))
        .ok_or(VaultError::InternalError(
            "No available Shamir share indices left.".to_string(),
        ))?;

    // 4. Generate internal u64 ID and exposed Principal ID
    let internal_id = token_storage::get_next_token_id()?;
    let token_principal = generate_unique_principal().await?;

    let current_time = time();
    // Use Duration for clarity
    let duration_24h = Duration::from_secs(24 * 60 * 60);
    let expires_at = current_time.saturating_add(duration_24h.as_nanos() as u64);

    let token = VaultInviteToken {
        internal_id, // Store internal ID
        token_id: token_principal, // Store exposed Principal ID
        vault_id: vault_id.clone(), // Store vault Principal
        role,
        shamir_share_index,
        status: InviteStatus::Pending,
        created_at: current_time,
        expires_at,
        claimed_at: None,
        claimed_by: None,
    };

    // 5. Store the token using the new storage function
    token_storage::insert_token(internal_id, token.clone(), token_principal)?;

    ic_cdk::print(format!(
        "✉️ INFO: Invite token {} generated for vault {} (Role: {:?}, Index: {}) by owner {}",
        token_principal.to_text(), vault_id.to_text(), role, shamir_share_index, caller.to_text()
    ));

    Ok(token)
}

/// Claims an invite token, creating a VaultMember entry.
pub async fn claim_existing_invite(
    token_principal: InviteTokenId, // Now Principal
    claimer_principal: PrincipalId,
    claim_data: InviteClaimData,
) -> Result<MemberProfile, VaultError> {
    let current_time = time();

    // 1. Find internal ID using the secondary index
    let internal_id = token_storage::get_internal_token_id(token_principal)
        .ok_or(VaultError::TokenInvalid("Token not found.".to_string()))?;

    // 2. Get token data using the internal ID
    let mut token = token_storage::get_token(internal_id)
        .ok_or(VaultError::InternalError("Token data missing for internal ID".to_string()))?;

    // 3. Perform validation (check status, expiry etc.)
    if token.status != InviteStatus::Pending {
        return Err(VaultError::TokenInvalid(format!(
            "Token already {:?}",
            token.status
        )));
    }
    if current_time > token.expires_at {
        token.status = InviteStatus::Expired;
        // Update token status in storage (using internal ID and principal for remove/insert)
        token_storage::insert_token(internal_id, token.clone(), token_principal)?;
        return Err(VaultError::TokenInvalid("Token has expired.".to_string()));
    }

    // Fetch vault config needed for state update later
    // TODO: Update vault_service::get_vault_config to accept Principal
    let mut vault_config = vault_service::get_vault_config(&token.vault_id)?;

    // 4. Mark token as claimed
    token.status = InviteStatus::Claimed;
    token.claimed_at = Some(current_time);
    token.claimed_by = Some(claimer_principal);

    // 5. Update the token in storage (remove & re-insert)
    // Note: A dedicated update function in storage might be better long-term.
    token_storage::insert_token(internal_id, token.clone(), token_principal)?;

    // 6. Create the VaultMember
    // Member ID is the claimer's Principal
    let member_id = claimer_principal;
    let member = VaultMember {
        internal_id: None, // VaultMember might not need a separate internal u64 ID if keyed by Principal
        member_id, // Principal
        vault_id: token.vault_id, // Principal
        principal: claimer_principal,
        role: token.role,
        status: MemberStatus::Active,
        name: claim_data.name,
        relation: claim_data.relation,
        shamir_share_index: token.shamir_share_index,
        added_at: current_time,
        updated_at: current_time,
        access_control: Default::default(), // Use AccessControl default
        has_approved_unlock: false,
    };

    // 7. Store the new member
    // TODO: Refactor to use a dedicated members storage module.
    // Assuming storage::VAULT_MEMBERS still exists and uses a composite string key for now.
    // This needs alignment with how VAULT_MEMBERS keys are actually handled/refactored.
    storage::VAULT_MEMBERS.with(|map| {
        let key_string = storage::create_member_key(&member.vault_id.to_text(), &member.member_id.to_text());
        let key = Cbor(key_string); // Assume StorableString is wrapper for String
        let value = Cbor(member.clone());
        map.borrow_mut().insert(key, value);
        Ok::<(), VaultError>(())
    })?;

    // 8. Update Vault state if needed
    let member_count = get_members_for_vault(&token.vault_id)?.len();
    let mut config_updated = false;
    if vault_config.status == VaultStatus::NeedSetup && member_count > 0 {
        vault_config.status = VaultStatus::SetupComplete;
        config_updated = true;
    }
    // Potentially transition to Active if conditions met (e.g., min heirs)
    // TODO: Add logic based on finalize_setup requirements from arch doc

    if config_updated {
        // TODO: Update vault_service::save_vault_config if it becomes async or needs Principal
        vault_service::save_vault_config(&vault_config)?; // Assuming sync for now
        ic_cdk::print!(format!("✅ INFO: Vault {} status updated after member claim.", token.vault_id.to_text()));
    }

    // 9. Construct and return the member profile
    Ok(MemberProfile {
        member_id: member.member_id, // Principal
        vault_id: token.vault_id,
        principal: claimer_principal,
        role: token.role,
        status: MemberStatus::Active,
        shamir_share_index: token.shamir_share_index,
        name: member.name,
        relation: member.relation,
        added_at: member.added_at,
    })
}

/// Revokes a pending invite token.
/// Only the vault owner can revoke tokens.
pub fn revoke_invite_token(token_principal: InviteTokenId /* Now Principal */, caller: PrincipalId) -> Result<(), VaultError> {
    // 1. Find internal ID using the secondary index
    let internal_id = token_storage::get_internal_token_id(token_principal)
        .ok_or(VaultError::TokenInvalid("Token not found.".to_string()))?;

    // 2. Get token data using the internal ID
    let mut token = token_storage::get_token(internal_id)
        .ok_or(VaultError::InternalError("Token data missing for internal ID".to_string()))?;

    // 3. Check authorization - only owner can revoke
    // TODO: Refactor vault_service::get_vault_config call
    let vault_config = match vault_service::get_vault_config(&token.vault_id) {
        Ok(config) => config,
        Err(e) => return Err(e), // Propagate error
    };
    if vault_config.owner != caller {
        return Err(VaultError::NotAuthorized(
            "Only the vault owner can revoke invites.".to_string(),
        ));
    }

    // 4. Check Token Status: Only pending tokens can be revoked
    if token.status != InviteStatus::Pending {
        return Err(VaultError::TokenInvalid(format!(
            "Cannot revoke token with status {:?}.",
            token.status
        )));
    }

    // 5. Update Status and Save (using remove & insert via storage module)
    token.status = InviteStatus::Revoked;
    token_storage::insert_token(internal_id, token.clone(), token_principal)?;

    ic_cdk::print(format!(
        "🚫 INFO: Invite token {} for vault {} revoked by owner {}",
        token_principal.to_text(), token.vault_id.to_text(), caller.to_text()
    ));
    Ok(())
}

/// Checks if a principal is already a member of a vault.
async fn is_member(vault_id: &VaultId, principal: PrincipalId) -> Result<bool, VaultError> {
    let members = get_members_for_vault(vault_id).await?;
    Ok(members.iter().any(|m| m.principal == principal))
}

/// Adds a new member to the vault (internal helper).
async fn add_new_member_to_vault(
    vault_id: &VaultId,
    principal: PrincipalId,
    vault_config: &VaultConfig, // Pass config to avoid re-fetching
) -> Result<MemberId, VaultError> {
    // Generate a unique ID for the new member
    let member_id = generate_ulid().await;
    let new_member = VaultMember {
        member_id: member_id.clone(),
        vault_id: vault_id.clone(),
        principal,
        name: "New Member".to_string(), // Default name, user can update later
        relation: "Invited".to_string(), // Default relation
        role: MemberRole::Heir, // Default role for invited members
        status: MemberStatus::Active,
        added_at: time(),
        added_by: vault_config.owner, // Assume added by owner via invite system
    };

    // Store the new member
    storage::VAULT_MEMBERS.with(|map| {
        let key = Cbor(create_member_key(vault_id, &member_id));
        let value = Cbor(new_member.clone());
        // Correct match for Option<V> returned by insert
        match map.borrow_mut().insert(key, value) {
            Some(_) => Ok(()), // Overwrite ok?
            None => Ok(()),
        }
    })?;

    ic_cdk::print(format!(
        "➕ INFO: Added new member {} ({}) to vault {}",
        member_id, principal, vault_id
    ));

    // If vault was NeedSetup, move to Active
    if vault_config.status == VaultStatus::NeedSetup {
        vault_service::set_vault_status(vault_id, VaultStatus::Active, Some(principal)).await?;
        ic_cdk::print(format!("✅ INFO: Vault {} status updated to Active after first member claim.", vault_id));
    }

    Ok(member_id)
}

/// Retrieves the details of a specific member within a vault.
///
/// # Arguments
/// * `vault_id` - The ID of the vault.
/// * `member_id` - The ID of the member.
/// * `caller` - The principal requesting the details (for authorization check).
///
/// # Returns
/// * `Result<VaultMember, VaultError>` - The member details or an error.
pub async fn get_member_details(
    vault_id: &VaultId,
    member_id: &MemberId,
    caller: PrincipalId,
) -> Result<VaultMember, VaultError> {
    // 1. Authorization: Check if caller is owner or the member themselves
    let vault_config = vault_service::get_vault_config(vault_id).await?;
    let key = Cbor(create_member_key(vault_id, member_id)); // Use Cbor directly

    let member = storage::VAULT_MEMBERS.with(|map_ref| {
        map_ref.borrow().get(&key).map(|v| v.0).ok_or_else(|| {
            // Pass only the member ID as per the enum variant definition
            VaultError::MemberNotFound(member_id.clone())
        })
    })?;

    if vault_config.owner != caller && member.principal != caller {
         return Err(VaultError::NotAuthorized(
            "Caller is not authorized to view this member's details.".to_string(),
        ));
    }

    Ok(member)
}

/// Lists all members associated with a specific vault.
///
/// # Arguments
/// * `vault_id` - The ID of the vault.
/// * `caller` - The principal requesting the list (for authorization check).
///
/// # Returns
/// * `Result<Vec<VaultMember>, VaultError>` - A list of members or an error.
pub async fn list_vault_members(vault_id: &VaultId, caller: PrincipalId) -> Result<Vec<VaultMember>, VaultError> {
    // 1. Authorization: Check if caller is owner or perhaps a member?
    let vault_config = vault_service::get_vault_config(vault_id).await?;
    let members = get_members_for_vault(vault_id).await?;

    let is_owner = vault_config.owner == caller;
    let is_member = members.iter().any(|m| m.principal == caller);

    if !is_owner && !is_member {
        return Err(VaultError::NotAuthorized(
            "Caller is not authorized to list members for this vault.".to_string(),
        ));
    }

    // 2. Return the fetched members
    Ok(members)
}
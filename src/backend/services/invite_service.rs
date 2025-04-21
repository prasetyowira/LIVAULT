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
    storage::{self, create_member_key, Cbor, StorableString, get_vault_member_prefix},
    services::vault_service, // Needed for state updates
    utils::crypto::generate_ulid, // Import generate_ulid
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
use crate::storage::{INVITE_TOKENS, VAULT_MEMBERS};

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
/// Uses secure randomness from the IC for the token ID.
///
/// # Arguments
/// * `vault_id` - The ID of the vault to invite to.
/// * `role` - The role (`Heir` or `Witness`) of the invitee.
/// * `caller` - The principal initiating the invite (must be vault owner).
///
/// # Returns
/// * `Result<VaultInviteToken, VaultError>` - The generated invite token or an error.
pub async fn generate_new_invite(
    vault_id: &VaultId,
    role: Role,
    caller: PrincipalId,
) -> Result<VaultInviteToken, VaultError> {
    // 1. Get Vault Config and check authorization
    let vault_config = vault_service::get_vault_config(vault_id)?;
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
    let existing_members = get_members_for_vault(vault_id)?;
    let used_indices: BTreeSet<u8> = existing_members
        .iter()
        .map(|m| m.shamir_share_index)
        .collect();

    let shamir_share_index = (1..=255) // Shamir index 0 is typically reserved
        .find(|i| !used_indices.contains(i))
        .ok_or(VaultError::InternalError(
            "No available Shamir share indices left.".to_string(),
        ))?;

    // 4. Generate token details using raw_rand
    // Get 16 random bytes (128 bits) for the token ID
    let (random_bytes,) = raw_rand().await.map_err(|(code, msg)| {
        VaultError::InternalError(format!(
            "Failed to get random bytes for token ID: code={}, message={}",
            code as u8, msg
        ))
    })?;

    // Encode bytes as hex string for the token_id
    let token_id = hex::encode(&random_bytes); // Pass as slice reference

    let current_time = time();
    // Use Duration for clarity
    let duration_24h = Duration::from_secs(24 * 60 * 60);
    let expires_at = current_time.saturating_add(duration_24h.as_nanos() as u64);

    let token = VaultInviteToken {
        token_id: token_id.clone(), // Use the hex-encoded random string
        vault_id: vault_id.clone(),
        role,
        shamir_share_index,
        status: InviteStatus::Pending,
        created_at: current_time,
        expires_at,
        claimed_at: None,
        claimed_by: None,
    };

    // 5. Store the token
    storage::INVITE_TOKENS.with(|map| {
        let key = Cbor(token_id.clone()); // Use Cbor directly if StorableString is just a wrapper
        let value = Cbor(token.clone());
        let mut token_map = map.borrow_mut();

        // Correct match for Option<V> returned by insert
        match token_map.insert(key, value) {
            Some(_previous_value) => {
                ic_cdk::print(format!(
                    "✉️ INFO: Invite token {} generated for vault {} (Role: {:?}, Index: {}) by owner {}",
                    token_id, vault_id, role, shamir_share_index, caller
                ));
                Ok(token)
            }
            None => {
                // This case means a token with the same ID was overwritten, which shouldn't happen with random IDs.
                // Log a warning, but consider it success for the purpose of returning the new token.
                ic_cdk::print(format!(
                    "⚠️ WARNING: Overwrote existing invite token {} for vault {}. This might indicate a random ID collision.",
                    token_id, vault_id
                ));
                Ok(token)
            }
        }
    })
}

/// Claims an invite token, creating a VaultMember entry.
/// Uses secure randomness from the IC for the new member ID.
///
/// # Arguments
/// * `token_id` - The invite token string to claim.
/// * `claimer_principal` - The principal of the user claiming the invite.
/// * `claim_data` - Additional profile data provided by the claimer.
///
/// # Returns
/// * `Result<MemberProfile, VaultError>` - The profile of the newly created member or an error.
pub async fn claim_existing_invite(
    token_id: &InviteTokenId,
    claimer_principal: PrincipalId,
    claim_data: InviteClaimData,
) -> Result<MemberProfile, VaultError> {
    let current_time = time();
    let mut claimed_token_details: Option<(VaultInviteToken, VaultConfig)> = None;

    // --- Transactional Block (Conceptual) ---
    // Ideally, these steps should be atomic. If storage supported transactions,
    // we'd wrap this. For now, proceed step-by-step, handling potential inconsistencies.

    // 1. Retrieve and validate the token, get vault config
    // Note: This storage operation itself is synchronous
    storage::INVITE_TOKENS.with(|token_map_ref| {
        let key = Cbor(token_id.clone());
        let mut token_map = token_map_ref.borrow_mut();

        // We need to get mutable access to update status later
        // Use get() first, then clone and use insert() to update.
        let current_token_opt = token_map.get(&key);

        let mut token = match current_token_opt {
            Some(storable_token) => storable_token.0, // Assuming Cbor<T>(T)
            None => return Err(VaultError::TokenInvalid("Token not found.".to_string())),
        };

        if token.status != InviteStatus::Pending {
            return Err(VaultError::TokenInvalid(format!(
                "Token already {:?}",
                token.status
            )));
        }

        if current_time > token.expires_at {
            token.status = InviteStatus::Expired;
            // Use insert to update the value
            match token_map.insert(key.clone(), Cbor(token)) {
                 Ok(_) => {},
                 // Handle the case where insert might return the previous value or an error
                 Ok(Some(_)) => { /* Overwriting is fine here if needed, or log */ },
                 Err(e) => return Err(VaultError::StorageError(format!("Failed to update expired token: {:?}", e)))
            };
            return Err(VaultError::TokenInvalid("Token has expired.".to_string()));
        }

        // Fetch vault config needed for state update later (synchronous call)
        let vault_config = vault_service::get_vault_config(&token.vault_id)?;

        // Mark token as claimed
        token.status = InviteStatus::Claimed;
        token.claimed_at = Some(current_time);
        token.claimed_by = Some(claimer_principal);

        // Update the token in storage
        // Correct match for Option<V> returned by insert
        match token_map.insert(key, Cbor(token.clone())) {
             Ok(_) => {
                // Store details for next steps - Need to clone config from outside
                // claimed_token_details = Some((token, vault_config));
                Ok(token) // Return the claimed token to be used outside
             },
             Ok(Some(_)) => Ok(token), // Overwriting is ok
             Err(e) => Err(VaultError::StorageError(format!("Failed to claim token: {:?}", e)))
        }
    })?;

    let (token, vault_config) = claimed_token_details.ok_or(VaultError::InternalError(
        "Claimed token details lost unexpectedly".to_string(),
    ))?;

    // 2. Create the VaultMember using raw_rand for member_id
    // Get 16 random bytes (128 bits) for the member ID
    let (random_bytes,) = raw_rand().await.map_err(|(code, msg)| {
        VaultError::InternalError(format!(
            "Failed to get random bytes for member ID: code={}, message={}",
            code as u8, msg
        ))
    })?;
    // Encode bytes as hex string for the member_id
    let member_id = hex::encode(&random_bytes); // Pass as slice reference

    let member = VaultMember {
        member_id: member_id.clone(), // Use the hex-encoded random string
        vault_id: token.vault_id.clone(),
        principal: claimer_principal,
        role: token.role,
        status: MemberStatus::Active, // Member is active immediately upon claim
        name: claim_data.name,
        relation: claim_data.relation,
        shamir_share_index: token.shamir_share_index,
        added_at: current_time,
        updated_at: current_time,
        access: Default::default(),
        has_approved_unlock: false,
    };

    // 3. Store the new member
    storage::VAULT_MEMBERS.with(|map| {
        let key = Cbor(create_member_key(vault_id, &member_id));
        let value = Cbor(member.clone());
        // Correct match for Option<V> returned by insert
        match map.borrow_mut().insert(key, value) {
            Some(_) => Ok(()), // Overwrite ok?
            None => Ok(()),
        }
    })?;

    // 4. Update Vault state if needed (e.g., transition from NEED_SETUP or SETUP_COMPLETE)
    let member_count = get_members_for_vault(&token.vault_id)?.len();
    if vault_config.status == VaultStatus::NeedSetup && member_count > 0 {
        vault_config.status = VaultStatus::SetupComplete;
        vault_service::save_vault_config(&vault_config).await?; // Save updated config (make save_vault_config async)
        ic_cdk::print!(format!("✅ INFO: Vault {} status updated to SetupComplete after first member claim.", token.vault_id));
    }

    // 5. Construct and return the member profile
    Ok(MemberProfile {
        member_id, // Use the generated member_id
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
///
/// # Arguments
/// * `token_id` - The ID of the token to revoke.
/// * `caller` - The principal attempting the revocation (must be vault owner).
///
/// # Returns
/// * `Result<(), VaultError>` - Success or an error.
pub fn revoke_invite_token(token_id: &InviteTokenId, caller: PrincipalId) -> Result<(), VaultError> {
    storage::INVITE_TOKENS.with(|map_ref| {
        let key = Cbor(token_id.clone()); // Use Cbor directly
        let mut map = map_ref.borrow_mut();

        // Get the token first to check ownership
        let current_token_opt = map.get(&key);
        let mut token = match current_token_opt {
            Some(storable_token) => storable_token.0,
            None => return Err(VaultError::TokenInvalid("Token not found.".to_string())),
        };

        // Check authorization - only owner can revoke
        // This requires fetching the vault config, which might be async.
        // Let's assume get_vault_config is sync for now, or needs refactoring like claim_invite.
        // (Refactoring needed similar to claim_invite if get_vault_config is async)
        let vault_config = match vault_service::get_vault_config(&token.vault_id) {
            Ok(config) => config,
            Err(e) => return Err(e),
        };

        if vault_config.owner != caller {
            return Err(VaultError::NotAuthorized(
                "Only the vault owner can revoke invites.".to_string(),
            ));
        }

        // 3. Check Token Status: Only pending tokens can be revoked
        if token.status != InviteStatus::Pending {
            return Err(VaultError::TokenInvalid(format!(
                "Cannot revoke token with status {:?}.",
                token.status
            )));
        }

        // 4. Update Status and Save
        token.status = InviteStatus::Revoked;

        // Update the token in storage
        // Correct match for Option<V> returned by insert
        match map.insert(key, Cbor(token)) {
            Some(_) => {
                ic_cdk::print(format!("🚫 INFO: Invite token {} for vault {} revoked by owner {}", token_id, token.vault_id, caller));
                Ok(())
            }
            None => {
                // This case means the key didn't exist before insert, which is unexpected here.
                // If we just fetched the token, it should exist. Treat as storage error.
                 Err(VaultError::StorageError("Failed to update token during revoke (token disappeared?)".to_string()))
            }
        }
    })
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
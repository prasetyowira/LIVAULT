// src/backend/services/invite_service.rs
// Placeholder for Invite token generation and management 

use crate::{
    error::VaultError,
    models::{
        common::*,
        VaultInviteToken,
        VaultMember,
        // Add MemberProfile struct if needed for claim return
    },
    storage::{self, create_member_key, Cbor, StorableString},
    utils::crypto::generate_ulid,
};
use ic_cdk::api::time;
use std::collections::BTreeSet; // For checking used Shamir indices

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

// --- Service Functions ---

/// Generates a new invite token for a specific vault and role.
/// Assigns the next available Shamir share index.
///
/// # Arguments
/// * `vault_id` - The ID of the vault to invite to.
/// * `role` - The role (`Heir` or `Witness`) of the invitee.
/// * `caller` - The principal initiating the invite (must be vault owner).
///
/// # Returns
/// * `Result<VaultInviteToken, VaultError>` - The generated invite token or an error.
pub fn generate_new_invite(
    vault_id: &VaultId,
    role: Role,
    caller: PrincipalId,
) -> Result<VaultInviteToken, VaultError> {
    // 1. Get Vault Config and check authorization
    let vault_config = super::vault_service::get_vault_config(vault_id)?;
    if vault_config.owner != caller {
        return Err(VaultError::NotAuthorized(
            "Only the vault owner can generate invites.".to_string(),
        ));
    }

    // TODO: Check if vault is in a state that allows invites (e.g., Active, SetupComplete)
    // if vault_config.status != VaultStatus::Active && vault_config.status != VaultStatus::SetupComplete {
    //     return Err(VaultError::InternalError("Vault not in correct state to invite".to_string()));
    // }

    // 2. Determine next available Shamir index
    // TODO: This needs a proper way to fetch existing members for the vault
    // let existing_members = get_members_for_vault(vault_id)?;
    let used_indices: BTreeSet<u8> = BTreeSet::new(); // Placeholder
                                                      // .iter()
                                                      // .map(|m: &VaultMember| m.shamir_share_index)
                                                      // .collect();

    let shamir_share_index = (1..=255)
        .find(|i| !used_indices.contains(i))
        .ok_or(VaultError::InternalError(
            "No available Shamir share indices left.".to_string(),
        ))?;

    // 3. Generate token details
    let token_id = generate_ulid(); // Using ULID for the token itself
    let current_time = time();
    let expires_at = current_time + (24 * 60 * 60 * 1_000_000_000); // 24 hours in nanoseconds

    let token = VaultInviteToken {
        token_id: token_id.clone(),
        vault_id: vault_id.clone(),
        role,
        shamir_share_index,
        status: InviteStatus::Pending,
        created_at: current_time,
        expires_at,
        claimed_at: None,
        claimed_by: None,
    };

    // 4. Store the token
    storage::INVITE_TOKENS.with(|map| {
        let key = StorableString(Cbor(token_id.clone()));
        let value = Cbor(token.clone());
        map.borrow_mut()
            .insert(key, value)
            .map_err(|e| VaultError::StorageError(format!("Failed to store invite token: {:?}", e)))?;
        Ok(token)
    })
}

/// Claims an invite token, creating a VaultMember entry.
///
/// # Arguments
/// * `token_id` - The invite token string to claim.
/// * `claimer_principal` - The principal of the user claiming the invite.
/// * `claim_data` - Additional profile data provided by the claimer.
///
/// # Returns
/// * `Result<MemberProfile, VaultError>` - The profile of the newly created member or an error.
pub fn claim_existing_invite(
    token_id: &InviteTokenId,
    claimer_principal: PrincipalId,
    claim_data: InviteClaimData,
) -> Result<MemberProfile, VaultError> {
    let current_time = time();
    let mut claimed_token: Option<VaultInviteToken> = None;

    // 1. Retrieve and validate the token
    storage::INVITE_TOKENS.with(|map| {
        let key = StorableString(Cbor(token_id.clone()));
        let mut borrowed_map = map.borrow_mut();

        let mut token = match borrowed_map.get(&key) {
            Some(storable_token) => storable_token.0,
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
            // Update the token status in storage
            borrowed_map.insert(key.clone(), Cbor(token)).map_err(|e| {
                VaultError::StorageError(format!("Failed to update expired token: {:?}", e))
            })?;
            return Err(VaultError::TokenInvalid("Token has expired.".to_string()));
        }

        // Mark token as claimed
        token.status = InviteStatus::Claimed;
        token.claimed_at = Some(current_time);
        token.claimed_by = Some(claimer_principal);

        // Update the token in storage
        borrowed_map
            .insert(key, Cbor(token.clone())) // Clone token for later use
            .map_err(|e| VaultError::StorageError(format!("Failed to claim token: {:?}", e)))?;

        claimed_token = Some(token); // Store the claimed token details
        Ok(())
    })?;

    // This should always be Some if the above block succeeded without error
    let token = claimed_token.ok_or(VaultError::InternalError(
        "Claimed token reference lost unexpectedly".to_string(),
    ))?;

    // 2. Create the VaultMember
    let member_id = generate_ulid();
    let member = VaultMember {
        member_id: member_id.clone(),
        vault_id: token.vault_id.clone(),
        principal: claimer_principal,
        role: token.role,
        status: MemberStatus::Active, // Member is active immediately upon claim
        name: claim_data.name,
        relation: claim_data.relation,
        shamir_share_index: token.shamir_share_index,
        added_at: current_time,
        updated_at: current_time,
        access_control: Default::default(),
        has_approved_unlock: false,
    };

    // 3. Store the VaultMember
    storage::VAULT_MEMBERS.with(|map| {
        let key = StorableString(Cbor(create_member_key(&token.vault_id, &member_id)));
        let value = Cbor(member.clone());
        map.borrow_mut()
            .insert(key, value)
            .map_err(|e| VaultError::StorageError(format!("Failed to store vault member: {:?}", e)))
    })?;

    // 4. TODO: Update VaultConfig state if needed (e.g., if first heir claims, move from NeedSetup to SetupComplete)
    // let _ = super::vault_service::maybe_update_vault_status_on_claim(&token.vault_id);

    ic_cdk::print(format!(
        "📝 INFO: Invite token {} claimed by {}. Member {} created for vault {}",
        token_id, claimer_principal, member_id, token.vault_id
    ));

    // 5. Return the profile of the new member
    Ok(MemberProfile {
        member_id,
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

// TODO: Add function to revoke an invite token (set status to Revoked)
// TODO: Add function to list members for a vault (needed for Shamir index check)
// TODO: Add function to get member details

"" 
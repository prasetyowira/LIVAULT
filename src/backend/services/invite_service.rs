// src/backend/services/invite_service.rs
// Placeholder for Invite token generation and management 

// --- Implementation Plan (Detailed) ---
//
// Handles vault invitation token lifecycle using `sharks` for Shamir Secret Sharing.
// Assumes a globally initialized RNG (e.g., ChaCha8Rng) is available via a utility function.
//
// --- Dependencies ---
// Storage Modules:
// - storage::vault_configs::{get_vault_config}
// - storage::tokens::{get_next_token_id, insert_token, get_internal_token_id, get_token, remove_token}
// - storage::members::{get_members_by_vault, is_member, insert_member}
// Models:
// - models::vault_invite_token::VaultInviteToken
// - models::vault_member::VaultMember
// - models::vault_config::VaultConfig
// - models::common::{Role, VaultStatus, PrincipalId, ShamirShareIndex, InternalId, InviteTokenId, VaultId}
// Error Handling:
// - error::VaultError
// External Crates:
// - sharks::{Sharks, Share}
// - rand_chacha::ChaCha8Rng // Or the specific RNG type initialized globally
// - rand_core::RngCore // For using the RNG
// - ic_cdk // For caller, time
// - core::convert::TryFrom // For Share::try_from
// Utility Functions:
// - utils::rng::get_global_rng // Assumed function to borrow the global RNG
// - utils::time::get_current_time_ns // Assumed function for consistent time
// - utils::crypto::generate_unique_principal // Assumed function using raw_rand internally
// Logging:
// - ic_cdk::print

// Models, utils, & Storage function that need to be created or implement first:
// - crate::models::common::{ShamirShareIndex, InternalId};
// - crate::utils::time::get_current_time_ns
// - crate::utils::rng::get_global_rng

// --- Function Definitions Outline ---

pub mod invite_service {

    use crate::models::vault_invite_token::{TokenStatus, VaultInviteToken, VaultInviteRequest};
    use crate::models::vault_member::{AccessControl, VaultMember};
    use crate::models::vault_config::VaultConfig;
    use crate::models::common::{Role, VaultStatus, MemberStatus, PrincipalId, ShamirShareIndex, InternalId, InviteTokenId, VaultId};
    use crate::storage::{tokens, members, vault_configs};
    use crate::error::VaultError;
    use crate::utils; // Using rng::with_internal_rng, time::get_current_time_ns, crypto::generate_unique_principal
    use ic_cdk::print;
    use candid::Principal;
    use sharks::{Sharks, Share};
    use rand_chacha::ChaCha8Rng; // For type matching with global RNG
// use rand_core::RngCore;
    use core::convert::TryFrom;
    use std::collections::HashSet;
    use serde_json::Value::String;
    // Added for Shamir index check

    const INVITE_TOKEN_DURATION_NS: u64 = 24 * 60 * 60 * 1_000_000_000; // 24 hours in nanoseconds

    // Placeholder for the actual secret generation/retrieval logic (must return bytes).
    // This MUST be implemented based on where the master secret/key is stored/derived.
    fn get_vault_secret_for_sharing_bytes(_vault_id: &VaultId) -> Result<Vec<u8>, VaultError> {
        // print(format!("Placeholder: Retrieving secret bytes for vault {}", vault_id));
        // Example: Fetch a master key from vault config or a dedicated key service.
        // let config = vault_configs::get_vault_config(vault_id).ok_or(VaultError::VaultNotFound)?; // Example dependency
        // let secret_bytes = config.master_key; // Hypothetical field
        // Ok(secret_bytes)
        Err(VaultError::NotImplemented("Secret retrieval (bytes) for SSS".to_string()))
    }

    // Helper to get the next available Shamir index (1-based) for a member.
    fn get_next_available_shamir_index(vault_id: &VaultId, total_shares: u8) -> Result<ShamirShareIndex, VaultError> {
        print(format!("Finding next Shamir index for vault {} (total shares: {})", vault_id, total_shares));
        let members_result = members::get_members_by_vault(vault_id);
        // Handle potential error from storage if needed, though current signature returns Vec
        let members = members_result; // Assuming success for now

        let used_indices: HashSet<ShamirShareIndex> = members.into_iter()
            .filter_map(|m| m.shamir_index)
            .collect();

        (1..=total_shares)
            .find(|&index| !used_indices.contains(&index))
            .ok_or_else(|| VaultError::InternalError(format!("No available Shamir indices left for vault {}", vault_id)))
    }

    /// Generates a new invitation token for a specific vault and role.
    pub async fn generate_invite(
        vault_id: VaultId,
        role: Role,
        inviter: PrincipalId,
        req: VaultInviteRequest
    ) -> Result<(InviteTokenId, Vec<u8>), VaultError> {
        print(format!("generate_invite called for vault {} by inviter {}", vault_id, inviter));

        // --- 1. Precondition Checks ---
        let config = match vault_configs::get_vault_config(&vault_id) {
            Ok(_config) => _config,
            Err(e) => return Err(VaultError::VaultNotFound(e))
        };
        if config.owner != inviter {
            return Err(VaultError::NotAuthorized("Only vault owner can generate invites".to_string()));
        }
        if !matches!(config.status, VaultStatus::NeedSetup | VaultStatus::Active) {
            return Err(VaultError::InvalidState("Vault not in a state to allow invites".to_string()));
        }

        // TODO: Access shamir_config fields safely. Assuming VaultConfig has shamir_config: ShamirConfig
        let threshold = config.shamir_config.threshold;
        let total_shares = config.shamir_config.total_shares;
        if threshold == 0 || total_shares == 0 || threshold > total_shares {
            return Err(VaultError::InternalError("Invalid Shamir configuration in vault".to_string()));
        }
        print(format!("Vault {} checks passed. SSS Params t={}, n={}", vault_id, threshold, total_shares));

        // --- 2. Get Secret & Shamir Index ---
        let secret_bytes = get_vault_secret_for_sharing_bytes(&vault_id)?;
        let shamir_index = get_next_available_shamir_index(&vault_id, total_shares)?;
        print(format!("Using Shamir index {} for new invite", shamir_index));

        // --- 3. Generate Token IDs ---
        let internal_id: InternalId = tokens::get_next_token_id()?;
        let external_principal_id: InviteTokenId = utils::crypto::generate_unique_principal().await?;
        print(format!("Generated token IDs: internal={}, external={}", internal_id, external_principal_id));

        // --- 4. Split Secret using sharks ---
        let share_bytes = utils::rng::with_internal_rng(|rng| { // Use the helper to access global RNG
            let sharks_instance = Sharks(threshold);
            let dealer = sharks_instance.dealer_rng(&secret_bytes, rng);

            // Generate shares up to the required index
            // dealer.take() returns an iterator, collect ensures all are generated
            let shares: Vec<Share> = dealer.take(shamir_index as usize).collect();
            let specific_share = shares.get(shamir_index as usize - 1) // 0-based access for Vec
                .ok_or_else(|| VaultError::InternalError(format!("Failed to generate share for index {}", shamir_index)))?;

            // Serialize the share to bytes using From trait
            Ok(Vec::from(specific_share))
        })?; // Propagate potential error from Ok/Err wrapping
        print(format!("Secret split, generated share of size {} bytes", share_bytes.len()));

        // --- 5. Create & Store Token ---
        let current_time_ns = utils::time::get_current_time_ns();
        let expires_at = current_time_ns + INVITE_TOKEN_DURATION_NS;
        let token_data = VaultInviteToken {
            // Assuming VaultInviteToken struct fields match this
            internal_id, // Consider if this field is actually needed in the struct or just map key
            token_id: external_principal_id,
            vault_id,
            role,
            name: req.name,
            relation: req.relation,
            email: req.email,
            status: TokenStatus::Issued,
            created_at: current_time_ns,
            expires_at,
            shamir_share_index: shamir_index.clone(), // Store the assigned index
            share_data: share_bytes.clone(),
            claimed_by: None,
            claimed_at: None,
        };
        tokens::insert_token(internal_id, token_data, external_principal_id)?;
        print(format!("Invite token {} stored successfully", external_principal_id));

        // --- 6. Return ---
        Ok((external_principal_id, share_bytes))
    }

    /// Claims an invitation token, converting it into a vault membership.
    pub fn claim_invite(
        token_principal: InviteTokenId,
        claimer: PrincipalId
    ) -> Result<VaultMember, VaultError> {
        print(format!("claim_invite called for token {} by claimer {}", token_principal, claimer));
        let current_time_ns = utils::time::get_current_time_ns();

        // --- 1. Get Token ---
        let internal_id = tokens::get_internal_token_id(token_principal)
            .ok_or(VaultError::InviteNotFound)?;
        let mut token = tokens::get_token(internal_id)
            .ok_or(VaultError::InviteNotFound)?;
        print(format!("Retrieved token data for internal ID {}", internal_id));

        // --- 2. Validate Token ---
        if token.status != TokenStatus::Issued {
            return Err(VaultError::InvalidState("Invite token already claimed or revoked".to_string()));
        }
        if current_time_ns > token.expires_at {
            // Note: Consider calling remove_token here or rely on scheduler
            return Err(VaultError::InviteExpired);
        }
        print!("Token validation passed (pending, not expired)");

        // --- 3. Check Existing Membership ---
        if members::is_member(&token.vault_id, &claimer) { // is_member returns Result
            return Err(VaultError::AlreadyMember);
        }
        print(format!("Claimer {} is not already a member of vault {}", claimer, token.vault_id));

        // --- 4. Create & Store Member ---
        let acl = AccessControl::default();
        let internal_id: InternalId = tokens::get_next_token_id()?;
        let new_member = VaultMember {
            internal_id: Some(internal_id),
            member_id: claimer,
            vault_id: token.vault_id.clone(),
            name: Some(token.name.clone()),
            relation: token.relation.clone(),
            email: Some(token.email.clone()),
            role: token.role.clone(),
            status: MemberStatus::Active, // Assuming Active is the status post-claim
            added_at: current_time_ns,
            updated_at: current_time_ns,
            shamir_share_index: token.shamir_share_index.clone(), // Assign index from token
            has_approved_unlock: false,
            access_control: acl
        };
        members::insert_member(&new_member);
        print(format!("Stored new member {} for vault {}", claimer, token.vault_id.clone()));

        // --- 5. Update Token Status (Instead of Removing) ---
        token.status = TokenStatus::Claimed;
        token.claimed_by = Some(claimer);
        token.claimed_at = Some(current_time_ns);
        // Update the token in storage - Assuming insert overwrites
        tokens::insert_token(internal_id, token, token_principal)?;
        print(format!("Updated token {} status to Claimed", token_principal));

        // --- 6. Post-Claim Actions ---
        // Placeholder: Logic to check quorum, update vault status, etc.,
        // should likely reside in VaultService or be triggered via an event/callback.
        print("Post-claim actions (e.g., quorum check) skipped in InviteService");

        // --- 7. Return ---
        Ok(new_member)
    }

    /// Revokes a pending invitation token.
    pub fn revoke_invite(
        token_principal: InviteTokenId,
        revoker: PrincipalId
    ) -> Result<(), VaultError> {
        print(format!("revoke_invite called for token {} by revoker {}", token_principal, revoker));

        // --- 1. Get Token ---
        let internal_id = tokens::get_internal_token_id(token_principal)
            .ok_or(VaultError::InviteNotFound)?;
        let token = tokens::get_token(internal_id)
            .ok_or(VaultError::InviteNotFound)?;
        print(format!("Retrieved token data for internal ID {}", internal_id));

        // --- 2. Check Permissions ---
        let config = match vault_configs::get_vault_config(&token.vault_id) {
            Ok(_config) => _config,
            Err(e) => return Err(VaultError::VaultNotFound(e))
        };
        if config.owner != revoker {
            return Err(VaultError::NotAuthorized("Only vault owner can revoke invites".to_string()));
        }
        print("Revoker permission check passed");

        // --- 3. Validate Token State ---
        if token.status != TokenStatus::Issued {
            // Technically already covered by removal, but good explicit check
            return Err(VaultError::InvalidState("Invite token already claimed or revoked".to_string()));
        }
        print("Token state check passed (Pending)");

        // --- 4. Remove Token ---
        tokens::remove_token(internal_id, token_principal)?;
        print(format!("Removed token {} successfully", token_principal));

        // --- 5. Return ---
        Ok(())
    }

    /// Lists all members for a given vault.
    pub fn list_members(
        vault_id: VaultId,
        requester: PrincipalId
    ) -> Result<Vec<VaultMember>, VaultError> {
        print(format!("list_members called for vault {} by requester {}", vault_id, requester));

        // --- 1. Check Permissions ---
        let config = match vault_configs::get_vault_config(&vault_id) {
            Ok(_config) => _config,
            Err(e) => return Err(VaultError::VaultNotFound(e))
        };
        // Allow owner or any existing member to list members
        if config.owner != requester && !members::is_member(&vault_id, &requester) {
            return Err(VaultError::NotAuthorized("Only vault owner or members can list members".to_string()));
        }
        print("Requester permission check passed");

        // --- 2. Fetch Members ---
        let member_list = members::get_members_by_vault(&vault_id);
        print(format!("Retrieved {} members for vault {}", member_list.len(), vault_id));

        // --- 3. Return ---
        Ok(member_list)
    }

    /// Gets details for a specific member of a vault.
    pub fn get_member_details(
        vault_id: VaultId,
        member_id: PrincipalId,
        requester: PrincipalId
    ) -> Result<Option<VaultMember>, VaultError> {
        print(format!("get_member_details called for member {} in vault {} by requester {}", member_id, vault_id, requester));

        // --- 1. Check Permissions ---
        let config = match vault_configs::get_vault_config(&vault_id) {
            Ok(_config) => _config,
            Err(e) => return Err(VaultError::VaultNotFound(e))
        };
        // Allow owner or any existing member to view details
        if config.owner != requester && !members::is_member(&vault_id, &requester) {
            return Err(VaultError::NotAuthorized("Only vault owner or members can view member details".to_string()));
        }
        // Optional stricter check: Allow only self-view unless owner
        // if requester != member_id && config.owner != requester {
        //     return Err(VaultError::NotAuthorized("Members can only view their own details".to_string()));
        // }
        print("Requester permission check passed");

        // --- 2. Fetch Member ---
        let member_option = members::get_member(&vault_id, &member_id); // get_member returns Result
        print(format!("Member details {} found", if member_option.is_some() {"were"} else {"were not"}));

        // --- 3. Return ---
        Ok(member_option)
    }

} // end mod invite_service

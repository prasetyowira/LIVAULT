// src/backend/storage/members.rs
use crate::storage::memory::{get_vault_members_memory, Memory};
use crate::storage::storable::Cbor;
use crate::models::{common::{VaultId, PrincipalId, Role}, vault_config, vault_member::VaultMember};
use ic_stable_structures::StableBTreeMap;
use std::cell::RefCell;
use candid::Principal;
use crate::error::VaultError;
use crate::models::vault_config::VaultConfig;
use crate::storage;
// Needed for Principal::min_id / max_id

type StorableVaultMember = Cbor<VaultMember>;

thread_local! {
    /// Vault Members: Key = (VaultId, PrincipalId), Value = VaultMember
    pub static MEMBERS: RefCell<StableBTreeMap<(VaultId, PrincipalId), StorableVaultMember, Memory>> = RefCell::new(
        StableBTreeMap::init(get_vault_members_memory())
    );
}

/// Inserts or updates a vault member.
pub fn insert_member(member: &VaultMember) -> Option<VaultMember> {
    let key = (member.vault_id, member.principal);
    let storable_member = Cbor(member.clone()); // Clone member for insertion

    MEMBERS.with(|map_ref| {
        map_ref.borrow_mut()
            .insert(key, storable_member)
            .map(|prev_cbor| prev_cbor.0) // Return previous value if any
    })
}

/// Retrieves a specific vault member by vault ID and principal ID.
pub fn get_member(vault_id: &VaultId, principal_id: &PrincipalId) -> Option<VaultMember> {
    let key = (*vault_id, *principal_id);
    MEMBERS.with(|map_ref| {
        map_ref.borrow()
            .get(&key)
            .map(|cbor| cbor.0) // Return cloned VaultMember
    })
}

/// Removes a specific vault member.
pub fn remove_member(vault_id: &VaultId, principal_id: &PrincipalId) -> Option<VaultMember> {
    let key = (*vault_id, *principal_id);
    MEMBERS.with(|map_ref| {
        map_ref.borrow_mut()
            .remove(&key)
            .map(|cbor| cbor.0) // Return removed value if any
    })
}

/// Retrieves all members associated with a specific vault ID.
pub fn get_members_by_vault(vault_id: &VaultId) -> Vec<VaultMember> {
    MEMBERS.with(|map_ref| {
        let map = map_ref.borrow();

        map.iter()
            .filter(|((entry_vault_id, _principal_id), _value)| entry_vault_id == vault_id)
            .map(|(_, member_cbor)| member_cbor.0)
            .collect()
    })
}

pub fn get_vaults_by_member(member_principal: PrincipalId) -> Vec<VaultConfig> {
    let mut member_vaults = Vec::new();
    let mut vault_ids = std::collections::HashSet::new(); // Avoid duplicates if member of multiple vaults

    MEMBERS.with(|map_ref| {
        let map = map_ref.borrow();
        for (_key, value) in map.iter() {
            let member: VaultMember = value.0;
            if member.principal == member_principal {
                vault_ids.insert(member.vault_id);
            }
        }
    });

    // Fetch config for each unique vault ID
    for vault_id in vault_ids {
        let vault_config = storage::get_vault_config(&vault_id).unwrap();
        member_vaults.push(vault_config);
    }
    member_vaults
}

/// Checks if a principal is a member of a specific vault.
pub fn is_member(vault_id: &VaultId, principal_id: &PrincipalId) -> bool {
    let key = (*vault_id, *principal_id);
    MEMBERS.with(|map_ref| {
        map_ref.borrow().contains_key(&key)
    })
}

/// Checks if a principal is a member of a specific vault with the expected role.
pub async fn is_member_with_role(vault_id: &VaultId, principal_id: &PrincipalId, expected_role: Role) -> Result<bool, VaultError> {
    match get_member(vault_id, principal_id) {
        Some(member) => Ok(member.role == expected_role),
        None => Ok(false), // Not a member
    }
}

/// Removes all members associated with a specific vault.
/// Returns the number of members removed.
pub async fn remove_members_by_vault(vault_id: &VaultId) -> Result<u64, VaultError> {
    let mut members_to_remove = Vec::new();
    MEMBERS.with(|map_ref| {
        for ((vid, pid), _) in map_ref.borrow().iter() {
            if vid == *vault_id {
                members_to_remove.push((vid, pid));
            }
        }
    });

    let mut removed_count = 0u64;
    MEMBERS.with(|map_ref| {
        let mut borrowed_map = map_ref.borrow_mut();
        for key in members_to_remove {
            if borrowed_map.remove(&key).is_some() {
                removed_count += 1;
            }
        }
    });

    Ok(removed_count)
} 
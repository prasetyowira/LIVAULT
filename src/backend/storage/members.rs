// src/backend/storage/members.rs
use crate::storage::memory::{get_vault_members_memory, Memory};
use crate::storage::storable::Cbor;
use crate::models::{
    common::{VaultId, PrincipalId},
    vault_member::VaultMember,
};
use ic_stable_structures::StableBTreeMap;
use std::cell::RefCell;
use candid::Principal; // Needed for Principal::min_id / max_id

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
/// Note: This iterates over a range and collects into a Vec, potentially memory-intensive for large vaults.
pub fn get_members_by_vault(vault_id: &VaultId) -> Vec<VaultMember> {
    MEMBERS.with(|map_ref| {
        let map = map_ref.borrow();
        let range_start = (*vault_id, Principal::min_id());
        let range_end = (*vault_id, Principal::max_id());

        map.range(range_start..=range_end)
           .map(|(_key, member_cbor)| member_cbor.0) // Map to VaultMember
           .collect() // Collect into a Vec
    })
}

/// Checks if a principal is a member of a specific vault.
pub fn is_member(vault_id: &VaultId, principal_id: &PrincipalId) -> bool {
    let key = (*vault_id, *principal_id);
    MEMBERS.with(|map_ref| {
        map_ref.borrow().contains_key(&key)
    })
} 
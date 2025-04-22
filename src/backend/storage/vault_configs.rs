// src/backend/storage/vault_configs.rs
use crate::storage::memory::{get_vault_config_memory, Memory};
use crate::storage::storable::{Cbor, StorableString};
use crate::models::{common::VaultId, vault_config::VaultConfig, PrincipalId};
use ic_stable_structures::StableBTreeMap;
use std::cell::RefCell;

type StorableVaultConfig = Cbor<VaultConfig>;

thread_local! {
    /// Vault Configurations: Key = vault_id (Principal serialized as String), Value = VaultConfig
    /// Note: Using String key as it was in structures.rs. Consider migrating to Principal key if beneficial.
    pub static CONFIGS: RefCell<StableBTreeMap<StorableString, StorableVaultConfig, Memory>> = RefCell::new(
        StableBTreeMap::init(get_vault_config_memory())
    );
}

/// Inserts or updates a vault configuration.
pub fn insert_vault_config(config: &VaultConfig) -> Option<VaultConfig> {
    // Assuming config.vault_id is the Principal to be used as key
    let key = Cbor(config.vault_id.to_text());
    let storable_config = Cbor(config.clone());

    CONFIGS.with(|map_ref| {
        map_ref.borrow_mut()
            .insert(key, storable_config)
            .map(|prev_cbor| prev_cbor.0)
    })
}

/// Retrieves a vault configuration by its ID (Principal).
pub fn get_vault_config(vault_id: &VaultId) -> Option<VaultConfig> {
    let key = Cbor(vault_id.to_text());
    CONFIGS.with(|map_ref| {
        map_ref.borrow()
            .get(&key)
            .map(|cbor| cbor.0)
    })
}

pub fn get_vaults_config_by_owner(owner: PrincipalId) -> Vec<VaultConfig> {
    let mut owned_vaults = Vec::new();
    CONFIGS.with(|map_ref| {
        let map = map_ref.borrow();
        for (_key, value) in map.iter() {
            let config: VaultConfig = value.0;
            if config.owner == owner {
                owned_vaults.push(config);
            }
        }
    });
    owned_vaults
}

/// Removes a vault configuration.
pub fn remove_vault_config(vault_id: &VaultId) -> Option<VaultConfig> {
    let key = Cbor(vault_id.to_text());
    CONFIGS.with(|map_ref| {
        map_ref.borrow_mut()
            .remove(&key)
            .map(|cbor| cbor.0)
    })
}

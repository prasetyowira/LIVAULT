// src/backend/storage/tokens.rs
use crate::error::VaultError;
use crate::models::VaultInviteToken;
use crate::storage::storable::Cbor;
use crate::storage::memory::{Memory, get_token_counter_memory, get_invite_tokens_memory, get_token_principal_idx_memory};
use ic_stable_structures::{StableCell, StableBTreeMap, DefaultMemoryImpl};
use std::cell::RefCell;
use candid::Principal;

type StorableToken = Cbor<VaultInviteToken>;
type PrincipalBytes = Vec<u8>; // Key for secondary index

thread_local! {
    // Counter for generating internal token IDs
    static TOKEN_COUNTER: RefCell<StableCell<u64, Memory>> = RefCell::new(
        StableCell::init(get_token_counter_memory(), 0)
            .expect("Failed to initialize token counter")
    );

    // Primary storage: Internal u64 -> Token Data
    static TOKENS_MAP: RefCell<StableBTreeMap<u64, StorableToken, Memory>> = RefCell::new(
        StableBTreeMap::init(get_invite_tokens_memory()) // Reuse existing memory ID for the map
    );

    // Secondary index: Exposed Principal Bytes -> Internal u64 ID
    static TOKEN_PRINCIPAL_INDEX: RefCell<StableBTreeMap<PrincipalBytes, u64, Memory>> = RefCell::new(
        StableBTreeMap::init(get_token_principal_idx_memory()) // Use new memory ID for the index
    );
}

/// Gets the next available internal token ID and increments the counter.
pub fn get_next_token_id() -> Result<u64, VaultError> {
    TOKEN_COUNTER.with(|cell_ref| {
        let cell = cell_ref.borrow();
        let current_val = *cell.get();
        let next_val = current_val.checked_add(1)
            .ok_or_else(|| VaultError::InternalError("Token counter overflow".to_string()))?;
        // `set` is fallible
        cell_ref.borrow_mut().set(next_val)
            .map_err(|e| VaultError::StorageError(format!("Failed to update token counter: {:?}", e)))?;
        Ok(current_val) // Return the value *before* incrementing
    })
}

/// Inserts a token into both the primary map and the secondary index.
pub fn insert_token(internal_id: u64, token: VaultInviteToken, principal_id: Principal) -> Result<(), VaultError> {
    let storable_token = Cbor(token);
    let principal_bytes = principal_id.as_slice().to_vec();

    // Insert into primary map (u64 -> Token)
    TOKENS_MAP.with(|map_ref| {
        // insert returns Option<V>, we ignore the previous value if overwriting
        map_ref.borrow_mut().insert(internal_id, storable_token);
    });

    // Insert into secondary index (Principal Bytes -> u64)
    TOKEN_PRINCIPAL_INDEX.with(|index_ref| {
         // insert returns Option<V>
         index_ref.borrow_mut().insert(principal_bytes, internal_id);
    });
    // Assuming insertion errors trap, otherwise map Result/Option
    Ok(())
}

/// Retrieves a token using its internal u64 ID.
pub fn get_token(internal_id: u64) -> Option<VaultInviteToken> {
    TOKENS_MAP.with(|map_ref| map_ref.borrow().get(&internal_id).map(|c| c.0))
}

/// Retrieves the internal u64 ID using the exposed Principal ID.
pub fn get_internal_token_id(principal: Principal) -> Option<u64> {
    TOKEN_PRINCIPAL_INDEX.with(|index_ref| {
        index_ref.borrow().get(&principal.as_slice().to_vec())
    })
}

/// Removes a token from both the primary map and the secondary index.
pub fn remove_token(internal_id: u64, principal_id: Principal) -> Result<(), VaultError> {
    // Remove from primary map
    let removed_token = TOKENS_MAP.with(|map_ref| map_ref.borrow_mut().remove(&internal_id));

    // Optional: Check if the token actually existed
    if removed_token.is_none() {
        // Depending on requirements, log a warning or return an error
        ic_cdk::println!("WARN: remove_token called for non-existent internal ID: {}", internal_id);
        // return Err(VaultError::NotFound(format!("Token with internal ID {} not found", internal_id)));
    }

    // Remove from secondary index
    let principal_bytes = principal_id.as_slice().to_vec();
    let removed_index = TOKEN_PRINCIPAL_INDEX.with(|index_ref| {
        index_ref.borrow_mut().remove(&principal_bytes)
    });

    // Optional: Check if the index entry existed
    if removed_index.is_none() {
        ic_cdk::println!("WARN: remove_token removed primary data but index entry for principal {} was already missing.", principal_id);
    }

    Ok(())
} 
# Plan: Refactor Internal IDs to Dual `u64`/`Principal` Strategy

**Date:** 2024-07-27

**Author:** AI Assistant (Gemini)

**Goal:** Refactor specific internal identifiers (`token_id`, `content_id`, `upload_id`) to use a dual ID strategy: an internal, auto-incrementing `u64` for storage efficiency and ordering, and an exposed, unique `Principal` for external API interactions. This addresses Wasm compatibility concerns with ULID libraries and avoids exposing sequential internal IDs.

**Related Documentation:**

*   `plans/backend.architecture.md` (Section 4 describes the updated strategy)
*   `plans/vault_invite_token.schema.json`
*   `plans/vault_content_item.schema.json`

**Key Code Files Involved:**

*   `src/backend/models/vault_invite_token.rs`
*   `src/backend/models/vault_content_item.rs`
*   `src/backend/models/common.rs` (ID Type Aliases)
*   `src/backend/storage/memory.rs` (Memory ID Definitions)
*   `src/backend/storage/structures.rs` (Or new submodules like `storage/tokens.rs`, `storage/content.rs`)
*   `src/backend/utils/crypto.rs` (Principal Generation)
*   `src/backend/services/invite_service.rs`
*   `src/backend/services/vault_service.rs`
*   `src/backend/services/upload_service.rs`
*   `src/backend/api.rs` (Candid Interface)

## Implementation Steps

### 1. Define Memory IDs

*   **Location:** `src/backend/storage/memory.rs`
*   **Action:** Define new `MemoryId` constants for the counters and secondary indexes, ensuring they don't conflict with existing IDs.
    ```rust
    // Example additions to existing constants
    pub const TOKEN_COUNTER_MEM_ID: MemoryId = MemoryId::new(11); // Was BILLING_LOG_DATA
    pub const CONTENT_COUNTER_MEM_ID: MemoryId = MemoryId::new(12);
    pub const UPLOAD_COUNTER_MEM_ID: MemoryId = MemoryId::new(13);
    // Reserve 14-19
    pub const TOKEN_PRINCIPAL_IDX_MEM_ID: MemoryId = MemoryId::new(22); // After CURSOR_MEM_ID
    pub const CONTENT_PRINCIPAL_IDX_MEM_ID: MemoryId = MemoryId::new(23);
    pub const UPLOAD_PRINCIPAL_IDX_MEM_ID: MemoryId = MemoryId::new(24);

    // Add corresponding get_memory functions for these new IDs
    pub fn get_token_counter_memory() -> Memory { get_memory(TOKEN_COUNTER_MEM_ID) }
    pub fn get_content_counter_memory() -> Memory { get_memory(CONTENT_COUNTER_MEM_ID) }
    pub fn get_upload_counter_memory() -> Memory { get_memory(UPLOAD_COUNTER_MEM_ID) }
    pub fn get_token_principal_idx_memory() -> Memory { get_memory(TOKEN_PRINCIPAL_IDX_MEM_ID) }
    pub fn get_content_principal_idx_memory() -> Memory { get_memory(CONTENT_PRINCIPAL_IDX_MEM_ID) }
    pub fn get_upload_principal_idx_memory() -> Memory { get_memory(UPLOAD_PRINCIPAL_IDX_MEM_ID) }
    ```
    *Note: Adjust existing IDs (like BILLING_LOG_DATA) if necessary to avoid conflicts.* 

### 2. Define Storage Structures (Modular Approach Recommended)

*   **Recommendation:** Create new modules like `src/backend/storage/tokens.rs`, `storage/content.rs`, etc., to encapsulate the logic for each entity type.
*   **Location:** New modules (e.g., `src/backend/storage/tokens.rs`) and `src/backend/storage/mod.rs`.
*   **Action:**
    *   Define the `StableCell<u64>` for the counter.
    *   Define the primary `StableBTreeMap<u64, Storable<EntityType>, Memory>`.
    *   Define the secondary `StableBTreeMap<PrincipalBytes, u64, Memory>`.
    *   Implement helper functions for getting the next ID, inserting/getting/deleting primary data, and inserting/getting/deleting secondary index entries. Ensure functions return `Result<_, VaultError>` where appropriate.

    ```rust
    // Example for src/backend/storage/tokens.rs
    use crate::error::VaultError;
    use crate::models::VaultInviteToken;
    use crate::storage::storable::Cbor;
    use crate::storage::memory::{Memory, get_token_counter_memory, get_invite_tokens_memory, get_token_principal_idx_memory};
    use ic_stable_structures::{StableCell, StableBTreeMap, DefaultMemoryImpl};
    use std::cell::RefCell;
    use candid::Principal;

    type StorableToken = Cbor<VaultInviteToken>;
    type PrincipalBytes = Vec<u8>;

    thread_local! {
        static TOKEN_COUNTER: RefCell<StableCell<u64, Memory>> = RefCell::new(
            StableCell::init(get_token_counter_memory(), 0).expect("Failed to initialize token counter")
        );

        // Primary storage: Internal u64 -> Token Data
        static TOKENS_MAP: RefCell<StableBTreeMap<u64, StorableToken, Memory>> = RefCell::new(
            StableBTreeMap::init(get_invite_tokens_memory())
        );

        // Secondary index: Exposed Principal Bytes -> Internal u64 ID
        static TOKEN_PRINCIPAL_INDEX: RefCell<StableBTreeMap<PrincipalBytes, u64, Memory>> = RefCell::new(
            StableBTreeMap::init(get_token_principal_idx_memory())
        );
    }

    // Helper to get and increment the counter atomically
    fn get_next_id_internal() -> Result<u64, VaultError> {
        TOKEN_COUNTER.with(|cell_ref| {
            let cell = cell_ref.borrow();
            let current_val = *cell.get();
            let next_val = current_val.checked_add(1).ok_or(VaultError::InternalError("Token counter overflow".to_string()))?;
            // Note: `set` in StableCell is fallible
            cell_ref.borrow_mut().set(next_val)
                .map_err(|e| VaultError::StorageError(format!("Failed to update token counter: {:?}", e)))?;
            Ok(current_val) // Return the value *before* incrementing
        })
    }

    pub fn get_next_token_id() -> Result<u64, VaultError> {
        get_next_id_internal()
    }

    pub fn insert_token(internal_id: u64, token: VaultInviteToken, principal_id: Principal) -> Result<(), VaultError> {
        let storable_token = Cbor(token);
        let principal_bytes = principal_id.as_slice().to_vec();

        // Insert into primary map
        TOKENS_MAP.with(|map_ref| {
            map_ref.borrow_mut().insert(internal_id, storable_token);
        });

        // Insert into secondary index
        TOKEN_PRINCIPAL_INDEX.with(|index_ref| {
             index_ref.borrow_mut().insert(principal_bytes, internal_id);
        });
        Ok(())
    }

    pub fn get_token(internal_id: u64) -> Option<VaultInviteToken> {
        TOKENS_MAP.with(|map_ref| map_ref.borrow().get(&internal_id).map(|c| c.0))
    }

    pub fn get_internal_token_id(principal: Principal) -> Option<u64> {
        TOKEN_PRINCIPAL_INDEX.with(|index_ref| {
            index_ref.borrow().get(&principal.as_slice().to_vec())
        })
    }

     pub fn remove_token(internal_id: u64, principal_id: Principal) -> Result<(), VaultError> {
        // Remove from primary map
        let removed_token = TOKENS_MAP.with(|map_ref| map_ref.borrow_mut().remove(&internal_id));

        if removed_token.is_none() {
            // Optional: Log or return error if token didn't exist
            // return Err(VaultError::NotFound(...));
        }

        // Remove from secondary index
        let principal_bytes = principal_id.as_slice().to_vec();
        TOKEN_PRINCIPAL_INDEX.with(|index_ref| {
            index_ref.borrow_mut().remove(&principal_bytes);
        });

        Ok(())
    }

    // ... Add similar modules/functions for content items and upload sessions ...
    ```
    *Modify `src/backend/storage/mod.rs` to pub use functions from these new modules.* 

### 3. Modify Model Structs & ID Types

*   **Location:** `src/backend/models/common.rs`
*   **Action:** Update the type aliases for affected IDs to `Principal`.
    ```rust
    use candid::Principal;

    pub type VaultId = Principal; // Changed from String
    pub type MemberId = Principal; // Changed from String
    pub type InviteTokenId = Principal; // Changed from String
    pub type ContentId = Principal; // Changed from String
    pub type UploadId = Principal; // Add if not present, type Principal
    // ... other aliases like Timestamp, PrincipalId remain ...
    ```
*   **Location:** `src/backend/models/vault_invite_token.rs`, `vault_content_item.rs`, `vault_member.rs`, `vault_config.rs` (and potentially a new `upload_session.rs`)
*   **Action:**
    *   Change ID fields (e.g., `token_id`, `content_id`, `vault_id`, `member_id`) to use the updated `Principal` type aliases.
    *   Add the internal `u64` ID field to `VaultInviteToken`, `VaultContentItem`, etc. Decide on naming (e.g., `internal_id` or `_internal_id`) and whether it should derive `CandidType` (likely not, if never exposed).
    ```rust
    // Example for models/vault_invite_token.rs
    use crate::models::common::{InviteTokenId, VaultId, Timestamp, InviteStatus, Role};
    use candid::{CandidType, Principal};
    use serde::{Deserialize, Serialize};

    #[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
    pub struct VaultInviteToken {
        // Internal ID, used as primary key in storage, NOT exposed in API directly
        #[serde(skip_serializing)] // Skip serialization if never needed externally
        pub internal_id: u64,

        // Exposed ID (Principal), used in API - Type alias resolves to Principal
        pub token_id: InviteTokenId,
        pub vault_id: VaultId, // Now Principal
        pub role: Role,
        pub status: InviteStatus,
        pub created_at: Timestamp,
        pub expires_at: Timestamp,
        pub claimed_at: Option<Timestamp>,
        pub claimed_by: Option<Principal>,
        pub shamir_share_index: u8,
    }

    // Apply similar changes to VaultContentItem (content_id: ContentId, internal_id: u64, vault_id: VaultId)
    // Apply similar changes to VaultMember (member_id: MemberId, internal_id: u64?, vault_id: VaultId) - internal ID might be optional here if member is directly identified by Principal
    // Apply similar changes to VaultConfig (vault_id: VaultId, owner: Principal)
    ```

### 4. Implement Principal Generation Utility

*   **Location:** `src/backend/utils/crypto.rs`
*   **Action:** Create a function `generate_unique_principal` using the *existing* `generate_random_bytes` utility.
    ```rust
    use crate::error::VaultError;
    use crate::utils::crypto::generate_random_bytes; // Use existing util
    use candid::Principal;

    // Generates a new, unique Principal based on raw_rand via generate_random_bytes
    // Ensure this is called from an async context if generate_random_bytes remains async
    pub async fn generate_unique_principal() -> Result<Principal, VaultError> {
        // Generate 29 bytes for a self-authenticating ID
        let rand_bytes = generate_random_bytes(29).await?;

        // Add the self-authenticating suffix (0x02)
        let principal_bytes = [&rand_bytes[..], &[0x02]].concat();

        Ok(Principal::from_slice(&principal_bytes))
    }
    ```

### 5. Update Service Logic (Creation)

*   **Location:** `src/backend/services/*.rs` (e.g., `invite_service.rs`, `upload_service.rs`, `vault_service.rs`)
*   **Action:**
    *   Modify creation functions (e.g., `generate_new_invite`, `finish_upload` in `upload_service`) to use the new flow:
        1.  Call the appropriate `storage::...::get_next_..._id()` to get the internal `u64` ID.
        2.  Call `utils::crypto::generate_unique_principal().await` to get the exposed `Principal` ID.
        3.  Populate the model struct with *both* IDs.
        4.  Call the appropriate `storage::...::insert_...()` function, passing the internal `u64` ID, the model struct, and the exposed `Principal` ID.
    *   **Remove** old ID generation logic (e.g., hex-encoding random bytes or using `generate_ulid`).
    ```rust
    // Example update in services/invite_service.rs create_invite function
    // Ensure the function is marked `async`
    pub async fn generate_new_invite(
        vault_id: &VaultId, // Now Principal
        role: Role,
        caller: Principal,
    ) -> Result<VaultInviteToken, VaultError> {
        // ... validation using vault_id (Principal) ...

        let internal_id = storage::tokens::get_next_token_id()?;
        let token_principal = utils::crypto::generate_unique_principal().await?;

        let new_token = VaultInviteToken {
            internal_id: internal_id, // Store internal ID
            token_id: token_principal, // Store exposed Principal ID (type alias)
            vault_id: vault_id.clone(), // Store vault Principal
            // ... fill other fields ...
        };

        // Call the combined storage function
        storage::tokens::insert_token(internal_id, new_token.clone(), token_principal)?;

        // Log using the exposed token_id (Principal)
        ic_cdk::print(format!(
            "✉️ INFO: Invite token {} generated for vault {} ...",
            token_principal.to_text(), vault_id.to_text()
        ));

        Ok(new_token)
    }
    ```

### 6. Update Service Logic (Lookup/Modification/Deletion)

*   **Location:** `src/backend/services/*.rs`
*   **Action:** Modify functions that operate on existing entities based on their exposed ID (now `Principal`):
    1.  Accept the `Principal` ID as input.
    2.  Call the appropriate `storage::...::get_internal_..._id(principal)` function to get the internal `u64` ID from the secondary index.
    3.  Handle the `Option<u64>` result (e.g., return `VaultError::NotFound`).
    4.  Use the retrieved `u64` ID to perform actions (get/update/delete) on the primary storage map.
    5.  If deleting, call the appropriate `storage::...::remove_...(internal_id, principal_id)` function to remove entries from *both* the primary map and the secondary index.
    ```rust
    // Example update in services/invite_service.rs claim_existing_invite
    // Ensure function is marked `async` if needed for sub-calls
    pub async fn claim_existing_invite(
        token_principal: InviteTokenId, // Now Principal
        claimer_principal: Principal,
        claim_data: InviteClaimData,
    ) -> Result<MemberProfile, VaultError> {
        let current_time = time();

        // 1. Find internal ID using the secondary index
        let internal_id = storage::tokens::get_internal_token_id(token_principal)
            .ok_or(VaultError::TokenInvalid("Token not found.".to_string()))?;

        // 2. Get token data using the internal ID
        let mut token = storage::tokens::get_token(internal_id)
            .ok_or(VaultError::InternalError("Token data missing for internal ID".to_string()))?; // Should not happen if index is consistent

        // 3. Perform validation (check status, expiry etc. on `token`)
        // ... (existing validation logic) ...
        if token.status != InviteStatus::Pending { /* ... */ }
        if current_time > token.expires_at { /* ... */ }

        // 4. Update token state
        token.status = InviteStatus::Claimed;
        token.claimed_at = Some(current_time);
        token.claimed_by = Some(claimer_principal);

        // 5. Create the VaultMember (potentially requires generating member principal ID if not using claimer directly)
        // ... (logic to create VaultMember) ...
        let new_member = VaultMember { /* ... */ member_id: claimer_principal, /* ... */ };
        // Store member (using its own ID strategy - likely Principal as key)
        // storage::members::insert_member(...) ?;

        // 6. Update the VaultInviteToken in storage (using internal ID)
        // Need an update function in storage::tokens, or remove and re-insert
        // For simplicity, let's assume remove/re-insert for now
        storage::tokens::remove_token(internal_id, token_principal)?; // Remove old entry
        storage::tokens::insert_token(internal_id, token.clone(), token_principal)?; // Insert updated entry

        // 7. Update vault state if needed (e.g., finalize setup)
        // ...

        Ok(MemberProfile { /* ... */ })
    }

    // Similar updates for functions like revoke_invite_token, get_content_item, finish_upload (for associating content), etc.
    ```

### 7. Update API Layer

*   **Location:** `src/backend/api.rs`
*   **Action:** Change the type of ID parameters (`vault_id`, `token_id`, `content_id`, `upload_id`, `member_id`) in function signatures and request/response structs from the old `String` aliases to the new `Principal` aliases (e.g., `VaultId`, `InviteTokenId`, etc.).
    ```rust
    // Example changes in api.rs
    use crate::models::common::{VaultId, InviteTokenId, ContentId, UploadId, Role};
    use candid::Principal;

    // Request struct update
    #[derive(CandidType, Deserialize, Clone, Debug, Validate)]
    pub struct ClaimInviteRequest {
        #[validate(custom = "validate_principal")] // Add validation for Principal
        pub token: InviteTokenId, // Type alias now resolves to Principal
        // ... other fields ...
    }
    // Add a validator function for Principal if needed via `validator` crate
    fn validate_principal(p: &Principal) -> Result<(), ValidationError> { Ok(()) /* Basic check */ }

    // Endpoint signature update
    #[ic_cdk::update]
    async fn claim_invite(req: ClaimInviteRequest) -> Result<MemberProfile, VaultError> {
        validate_request(&req)?;
        check_cycles()?;
        let claimer = caller();
        // Pass the Principal directly
        services::invite_service::claim_existing_invite(req.token, claimer, /* map claim_data */).await
    }

    // Another example
    #[ic_cdk::query(guard = "check_cycles")]
    async fn get_vault(vault_id: VaultId /* Now Principal */) -> Result<VaultConfig, VaultError> {
        // Apply rate limiting if needed
        // rate_guard(caller())?;
        // Use owner_guard or appropriate check based on vault_id
        owner_guard(&vault_id)?; // Example guard needing Principal
        services::vault_service::get_vault_config(&vault_id)
    }

    // Apply similar changes to all endpoints handling these IDs.
    ```

## Testing Considerations

*   Thoroughly test the Principal generation uniqueness (statistically improbable collisions).
*   Verify secondary index consistency after creates, updates, and deletes.
*   Test edge cases like counter overflow (unlikely with `u64`).
*   Ensure correct handling of `Principal -> u64` lookups (not found errors).
*   Update existing unit/integration tests to use `Principal` where ID types have changed.

## Rollback Strategy

*   If issues arise, revert changes commit by commit.
*   A canister upgrade could potentially migrate data back to a single ID strategy if necessary, but this would be complex and require careful planning of the migration logic in `post_upgrade`. 
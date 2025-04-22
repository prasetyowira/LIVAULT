# Backend Storage Layer Documentation

This document details the structure and usage of the stable storage layer for the Livault backend canister, implemented using `ic-stable-structures`.

## Overview

The storage layer is modularized, with each primary data type (or closely related group) residing in its own Rust module within `src/backend/storage`. This promotes organization and maintainability. All interactions with stable storage should ideally go through the helper functions provided by these modules, which are re-exported via `src/backend/storage/mod.rs`.

## Core Concepts

-   **Stable Memory:** Data is stored in the canister's stable memory, ensuring persistence across upgrades.
-   **Memory Manager:** `storage/memory.rs` defines a `MemoryManager` responsible for allocating virtual memory regions (`VirtualMemory`) to different data structures using unique `MemoryId`s.
-   **Storable Types:** Data is serialized/deserialized using CBOR via the `Cbor<T>` wrapper defined in `storage/storable.rs`. This wrapper implements the `Storable` trait required by `ic-stable-structures`. `StorableString` (`Cbor<String>`) is a common key type.
-   **Data Structures:**
    -   `StableBTreeMap`: Used for key-value stores.
    -   `StableCell`: Used for storing single, global values (like counters or configuration).
    -   `StableLog`: Used for append-only logs (like billing).
-   **Modular Design:** Each module typically defines:
    -   A `thread_local!` static variable holding the stable data structure (`StableBTreeMap`, `StableCell`, etc.).
    -   Helper functions for interacting with the data (e.g., `insert_X`, `get_X`, `remove_X`, `get_next_X_id`).

## Modules

### 1. `memory.rs`

-   **Purpose:** Manages the allocation of stable memory regions.
-   **Key Components:**
    -   `MemoryId` constants: Unique IDs for each stable data structure (e.g., `VAULT_CONFIG_MEM_ID`, `VAULT_MEMBERS_MEM_ID`, `TOKEN_COUNTER_MEM_ID`, `APPROVALS_MEM_ID`).
    -   `MEMORY_MANAGER`: The central `MemoryManager<DefaultMemoryImpl>`.
    -   `get_memory(id: MemoryId)`: Retrieves a `VirtualMemory` instance for a given `MemoryId`.
    -   Specific getter functions (e.g., `get_vault_config_memory()`, `get_token_counter_memory()`, `get_approvals_memory()`) for each memory region.
-   **Usage:** Primarily used internally by other storage modules to obtain the correct memory instance for initializing their stable structures.

### 2. `storable.rs`

-   **Purpose:** Provides generic wrappers to make data types compatible with `ic-stable-structures`.
-   **Key Components:**
    -   `Cbor<T>`: A wrapper struct that implements the `Storable` trait for any type `T` that supports `Serialize + DeserializeOwned`. It uses CBOR for encoding/decoding. `Bound::Unbounded` is used.
    -   `StorableString`: A type alias for `Cbor<String>`, often used as a key type in `StableBTreeMap`.
    -   `PrincipalBytes`: A type alias for `Vec<u8>`, used for storing Principal bytes as keys in indexes.
-   **Usage:** Used throughout the storage layer to wrap data models before storing them (e.g., `Cbor<VaultConfig>`, `Cbor<VaultMember>`).

### 3. `cursor.rs`

-   **Purpose:** Implements a generic, persistent cursor (counter).
-   **Data Structure:** `CURSOR_POSITION: StableCell<u64, Memory>` (using `CURSOR_MEM_ID`).
-   **Functions:**
    -   `get_cursor() -> u64`: Returns the current cursor value.
    -   `set_cursor(position: u64) -> Result<(), String>`: Sets the cursor to a specific value.
    -   `increment_cursor() -> Result<u64, String>`: Increments the cursor and returns the *new* value.
-   **Usage:** Can be used for pagination offsets, sequence numbers, or other simple persistent counters.

### 4. `config.rs`

-   **Purpose:** Stores global canister configuration values set during initialization.
-   **Data Structures:**
    -   `ADMIN_PRINCIPAL: StableCell<Cbor<Principal>, Memory>` (using `ADMIN_PRINCIPAL_MEM_ID`).
    -   `CRON_PRINCIPAL: StableCell<Cbor<Principal>, Memory>` (using `CRON_PRINCIPAL_MEM_ID`).
    -   `MIN_CYCLES_THRESHOLD: StableCell<u128, Memory>` (using `MIN_CYCLES_THRESHOLD_MEM_ID`).
-   **Functions:**
    -   `init_config(admin: Principal, cron: Principal, threshold: u128)`: Sets the configuration values (called by `lib.rs#init`).
    -   `get_admin_principal() -> Principal`: Retrieves the admin principal.
    -   `get_cron_principal() -> Principal`: Retrieves the cron principal.
    -   `get_min_cycles_threshold() -> u128`: Retrieves the minimum cycle threshold.
-   **Usage:** Provides access to essential configuration parameters throughout the canister, primarily used by guards.

### 5. `vault_configs.rs`

-   **Purpose:** Stores the main configuration data for each vault.
-   **Data Structure:** `CONFIGS: StableBTreeMap<VaultId, Cbor<VaultConfig>, Memory>` (using `VAULT_CONFIG_MEM_ID`).
-   **Key:** `VaultId` (Principal).
-   **Value:** `Cbor<VaultConfig>`.
-   **Functions:**
    -   `insert_vault_config(config: &VaultConfig) -> Option<VaultConfig>`: Inserts or updates a vault config. Returns the previous value if any.
    -   `get_vault_config(vault_id: &VaultId) -> Option<VaultConfig>`: Retrieves a vault config by its `VaultId`.
    -   `remove_vault_config(vault_id: &VaultId) -> Option<VaultConfig>`: Removes a vault config. Returns the removed value if any.
    -   `get_vaults_config_by_owner(owner: Principal) -> Vec<VaultConfig>`: Retrieves all vaults owned by a principal (inefficient iteration).
-   **Usage:** Central repository for vault settings, status, owner, etc.

### 6. `members.rs`

-   **Purpose:** Stores information about members (heirs, witnesses) associated with vaults.
-   **Data Structure:** `MEMBERS: StableBTreeMap<(VaultId, PrincipalId), Cbor<VaultMember>, Memory>` (using `VAULT_MEMBERS_MEM_ID`).
-   **Key:** `(VaultId, PrincipalId)` (Composite key).
-   **Value:** `Cbor<VaultMember>`.
-   **Functions:**
    -   `insert_member(member: &VaultMember) -> Option<VaultMember>`: Inserts or updates a member.
    -   `get_member(vault_id: &VaultId, principal_id: &PrincipalId) -> Option<VaultMember>`: Retrieves a specific member.
    -   `remove_member(vault_id: &VaultId, principal_id: &PrincipalId) -> Option<VaultMember>`: Removes a member.
    -   `get_members_by_vault(vault_id: &VaultId) -> Vec<VaultMember>`: Retrieves all members for a specific vault (uses iteration).
    -   `is_member(vault_id: &VaultId, principal_id: &PrincipalId) -> bool`: Checks if a principal is a member of a vault.
    -   `get_vaults_by_member(member_principal: PrincipalId) -> Vec<VaultMember>`: Retrieves all vaults a principal is a member of (highly inefficient iteration).
    -   `is_member_with_role(vault_id: &VaultId, principal_id: &PrincipalId, expected_role: Role) -> Result<bool, VaultError>`: Checks if a principal is a member with a specific role.
    -   `remove_members_by_vault(vault_id: &VaultId) -> Result<u64, VaultError>`: Removes all members for a specific vault (returns count).
-   **Usage:** Managing vault membership and roles.

### 7. `tokens.rs` (Invite Tokens)

-   **Purpose:** Manages invite tokens using the dual-ID strategy (internal `u64`, external `Principal`).
-   **Data Structures:**
    -   `TOKEN_COUNTER: StableCell<u64, Memory>` (using `TOKEN_COUNTER_MEM_ID`).
    -   `TOKENS_MAP: StableBTreeMap<u64, Cbor<VaultInviteToken>, Memory>` (using `INVITE_TOKENS_MEM_ID`).
    -   `TOKEN_PRINCIPAL_INDEX: StableBTreeMap<PrincipalBytes, u64, Memory>` (using `TOKEN_PRINCIPAL_IDX_MEM_ID`).
-   **Key/Value (Primary):** `u64` (Internal ID) -> `Cbor<VaultInviteToken>`.
-   **Key/Value (Index):** `Vec<u8>` (Principal Bytes) -> `u64` (Internal ID).
-   **Functions:**
    -   `get_next_token_id() -> Result<u64, VaultError>`: Gets the next internal ID and increments the counter.
    -   `insert_token(internal_id: u64, token: VaultInviteToken, principal_id: Principal) -> Result<(), VaultError>`: Inserts token data into both maps.
    -   `get_token(internal_id: u64) -> Option<VaultInviteToken>`: Retrieves token by internal ID.
    -   `get_internal_token_id(principal: Principal) -> Option<u64>`: Looks up internal ID using the external Principal ID (via index).
    -   `remove_token(internal_id: u64, principal_id: Principal) -> Result<(), VaultError>`: Removes token data from both maps.
    -   `remove_tokens_by_vault(vault_id: &VaultId) -> Result<u64, VaultError>`: Removes all tokens associated with a specific vault (returns count).
-   **Usage:** Storing and managing vault invitation tokens.

### 8. `content.rs`

-   **Purpose:** Manages vault content metadata (not the chunk data itself) using the dual-ID strategy.
-   **Data Structures:**
    -   `CONTENT_COUNTER: StableCell<u64, Memory>` (using `CONTENT_COUNTER_MEM_ID`).
    -   `CONTENT_MAP: StableBTreeMap<u64, Cbor<VaultContentItem>, Memory>` (using `CONTENT_ITEMS_MEM_ID`).
    -   `CONTENT_PRINCIPAL_INDEX: StableBTreeMap<PrincipalBytes, u64, Memory>` (using `CONTENT_PRINCIPAL_IDX_MEM_ID`).
-   **Key/Value (Primary):** `u64` (Internal ID) -> `Cbor<VaultContentItem>`.
-   **Key/Value (Index):** `Vec<u8>` (Principal Bytes) -> `u64` (Internal ID).
-   **Functions:**
    -   `get_next_content_id() -> Result<u64, VaultError>`: Gets the next internal ID.
    -   `insert_content(internal_id: u64, item: VaultContentItem, principal_id: Principal) -> Result<(), VaultError>`: Inserts content metadata.
    -   `get_content(internal_id: u64) -> Option<VaultContentItem>`: Retrieves content metadata by internal ID.
    -   `get_internal_content_id(principal: Principal) -> Option<u64>`: Looks up internal ID by external Principal ID.
    -   `remove_content(internal_id: u64, principal_id: Principal) -> Result<(), VaultError>`: Removes content metadata.
    -   `update_content(internal_id: u64, updated_item: VaultContentItem) -> Result<(), VaultError>`: Updates content metadata (by internal ID).
    -   `remove_all_content_for_vault(vault_id: &VaultId) -> Result<u64, VaultError>`: Removes all content metadata for a vault (returns count).
-   **Usage:** Storing metadata about files, passwords, letters stored in vaults.

### 9. `content_index.rs`

-   **Purpose:** Stores an ordered list of content item *external* IDs for each vault.
-   **Data Structure:** `INDEX: StableBTreeMap<StorableString, Cbor<Vec<String>>, Memory>` (using `CONTENT_INDEX_MEM_ID`).
-   **Key:** `StorableString` (VaultId Principal serialized to text).
-   **Value:** `Cbor<Vec<String>>` (Vector of ContentId Principals serialized to text).
-   **Functions:**
    -   `add_to_index(vault_id: &VaultId, content_id: &ContentId) -> Result<(), String>`: Appends a content ID string to the vault's index.
    -   `get_index(vault_id: &VaultId) -> Result<Option<Vec<String>>, String>`: Retrieves the list of content ID strings for a vault.
    -   `remove_from_index(vault_id: &VaultId, content_id: &ContentId) -> Result<(), String>`: Removes a specific content ID string from a vault's index.
    -   `remove_index(vault_id: &VaultId) -> Result<(), String>`: Removes the entire index entry for a vault.
-   **Usage:** Used to list the content items belonging to a specific vault in a defined order.

### 10. `uploads.rs`

-   **Purpose:** Manages upload session metadata and the actual chunk data using the dual-ID strategy for sessions.
-   **Data Structures:**
    -   `UPLOAD_COUNTER: StableCell<u64, Memory>` (using `UPLOAD_COUNTER_MEM_ID`).
    -   `UPLOAD_SESSIONS_MAP: StableBTreeMap<u64, Cbor<UploadSession>, Memory>` (using `UPLOAD_SESSIONS_MEM_ID`).
    -   `UPLOAD_PRINCIPAL_INDEX: StableBTreeMap<PrincipalBytes, u64, Memory>` (using `UPLOAD_PRINCIPAL_IDX_MEM_ID`).
    -   `UPLOAD_CHUNKS_MAP: StableBTreeMap<(u64, u64), ChunkData, Memory>` (using `UPLOAD_CHUNKS_MEM_ID`).
-   **Key/Value (Session Primary):** `u64` (Internal Session ID) -> `Cbor<UploadSession>`.
-   **Key/Value (Session Index):** `Vec<u8>` (Session Principal Bytes) -> `u64` (Internal Session ID).
-   **Key/Value (Chunks):** `(u64, u64)` (Internal Session ID, Chunk Index) -> `Vec<u8>` (Raw Chunk Data).
-   **Functions:**
    -   `get_next_upload_id() -> Result<u64, VaultError>`: Gets the next internal session ID.
    -   `insert_upload_session(internal_id: u64, session: UploadSession, principal_id: Principal) -> Result<(), VaultError>`: Inserts session metadata.
    -   `get_upload_session(internal_id: u64) -> Option<UploadSession>`: Retrieves session metadata by internal ID.
    -   `get_internal_upload_id(principal: Principal) -> Option<u64>`: Looks up internal session ID by external Principal ID.
    -   `remove_upload_session(internal_id: u64, principal_id: Principal) -> Result<(), VaultError>`: Removes session metadata (from primary map and index).
    -   `save_chunk(internal_upload_id: u64, chunk_index: u64, data: ChunkData) -> Result<(), VaultError>`: Stores chunk data.
    -   `get_chunk(internal_upload_id: u64, chunk_index: u64) -> Option<ChunkData>`: Retrieves chunk data.
    -   `delete_chunks(internal_upload_id: u64) -> Result<(), VaultError>`: Removes all chunks associated with an upload session.
-   **Usage:** Managing the process of uploading chunked data.

### 11. `audit_logs.rs`

-   **Purpose:** Stores audit log entries per vault.
-   **Data Structure:** `LOGS: StableBTreeMap<StorableString, Cbor<Vec<AuditLogEntry>>, Memory>` (using `AUDIT_LOG_DATA_MEM_ID`).
-   **Key:** `StorableString` (Formatted string: `"audit:{vault_id}"`).
-   **Value:** `Cbor<Vec<AuditLogEntry>>`.
-   **Functions:**
    -   `add_entry(vault_id_str: &str, entry: AuditLogEntry) -> Result<(), String>`: Appends an entry to a vault's log (retrieves Vec, appends, inserts Vec).
    -   `get_entries(vault_id_str: &str) -> Option<Vec<AuditLogEntry>>`: Retrieves all entries for a vault.
    -   `compact_log(vault_id_str: &str, max_entries: usize) -> Result<(), String>`: Reduces the log size for a vault, keeping the most recent `max_entries`.
    -   `remove_logs(vault_id: &VaultId) -> Result<(), String>`: Removes the audit log entry for a vault.
-   **Usage:** Recording significant actions performed on vaults. *Note: Current `add_entry` reads/writes the entire vector, potentially inefficient for very long logs.* Consider using `StableLog` if strict append-only is sufficient.

### 12. `billing.rs`

-   **Purpose:** Stores billing events in an append-only log.
-   **Data Structure:** `BILLING_LOG: StableLog<Cbor<BillingEntry>, Memory, Memory>` (using `BILLING_LOG_INDEX_MEM_ID` and `BILLING_LOG_DATA_MEM_ID`).
-   **Functions:**
    -   `add_billing_entry(entry: BillingEntry) -> Result<u64, String>`: Appends a billing entry. Returns the index of the appended entry.
    -   `get_all_billing_entries() -> Vec<BillingEntry>`: Retrieves all entries from the log (potentially inefficient).
    -   `query_billing_entries(offset: usize, limit: usize) -> Vec<BillingEntry>`: Retrieves a paginated subset of entries.
-   **Usage:** Recording payments, charges, or other billing-related events.

### 13. `metrics.rs`

-   **Purpose:** Stores global canister metrics.
-   **Data Structure:** `METRICS_CELL: StableCell<Cbor<VaultMetrics>, Memory>` (using `METRICS_MEM_ID`).
-   **Functions:**
    -   `get_metrics() -> VaultMetrics`: Retrieves the current metrics struct.
    -   `update_metrics<F>(update_fn: F) -> Result<(), String>`: Updates the metrics struct using a closure.
    -   `increment_vault_count() -> Result<(), String>`: Specific helper to increment total vaults.
    -   `decrement_vault_count() -> Result<(), String>`: Specific helper to decrement total vaults.
    -   `update_active_vault_count(delta: i64) -> Result<(), String>`: Specific helper to adjust active vault count.
-   **Usage:** Tracking overall canister state and usage statistics.

### 14. `approvals.rs`

-   **Purpose:** Stores approval counts for vaults.
-   **Data Structure:** `APPROVALS: StableBTreeMap<VaultId, Cbor<ApprovalCounts>, Memory>` (using `APPROVALS_MEM_ID`).
-   **Key:** `VaultId` (Principal).
-   **Value:** `Cbor<ApprovalCounts>`.
-   **Functions:**
    -   `update_approval_counts(vault_id: &VaultId, counts: ApprovalCounts) -> Result<(), VaultError>`: Stores or updates the approval counts.
    -   `get_approval_status(vault_id: &VaultId) -> Result<ApprovalCounts, VaultError>`: Retrieves the current approval counts (defaults to 0).
    -   `remove_approvals(vault_id: &VaultId) -> Result<(), VaultError>`: Removes the approval record for a vault.
    -   `record_approval(vault_id: &VaultId, role: Role) -> Result<(), VaultError>`: Increments the approval count for a specific role.
-   **Usage:** Tracking heir and witness approvals required for unlocking a vault.

### 15. `structures.rs` (Legacy/Utils)

-   **Purpose:** Currently holds only the generic `get_value` helper function. Most data structures have been moved to dedicated modules.
-   **Functions:**
    -   `get_value<T>(result: Option<Cbor<T>>) -> Option<T>`: Unwraps the inner value `T` from `Option<Cbor<T>>`.
-   **Usage:** Provides a utility function. Should eventually be fully refactored/emptied. 
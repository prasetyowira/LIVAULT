# Progress Log

## 2024-07-25 17:50: Backend Phase 0 Completion

### Overview
Completed the initial scaffolding for the backend canister as outlined in Phase 0 of the `backend.plan.md`. This sets up the foundational structure for subsequent development phases.

### Key Components Implemented
1.  **Cargo Workspace (`src/backend/Cargo.toml`):** Created the `Cargo.toml` file, defining the canister name, version, edition, and specifying all required dependencies with pinned versions (`ic-cdk`, `ic-stable-structures`, `ic-cbor`, `ic-utils`, `serde`, `candid`, `thiserror`, `ulid`, `aes-gcm`, `sha2`, `rand`, `shamir-secret-sharing`). Configured release profile for size optimization (`lto = true`, `opt-level = 'z'`).
2.  **Rust Toolchain (`src/backend/rust-toolchain.toml`):** Pinned the Rust nightly toolchain (`nightly-2024-05-20`) and specified the necessary components (`rust-src`, `rustfmt`) and target (`wasm32-unknown-unknown`).
3.  **Module Structure:** Created the main library file (`src/backend/lib.rs`) with basic `init`, `post_upgrade`, and a test `greet` query function. Established the planned module structure by creating placeholder files and `mod.rs` files for `api`, `error`, `models`, `services`, `storage`, and `utils`.
4.  **Basic Error Handling (`src/backend/error.rs`):** Implemented a basic `VaultError` enum using `thiserror` and `CandidType` based on initial requirements from PRD/Tech Docs.

### Dependencies
- All dependencies listed in `src/backend/Cargo.toml` were successfully added and the project structure compiles targeting `wasm32-unknown-unknown`.

### Related Documentation
- [Backend Plan](mdc:plans/backend.plan.md) (Phase 0)
- [Backend Architecture](mdc:plans/backend.architecture.md)
- [Technical Design](mdc:plans/tech.docs.md)

---

## 2024-07-25 18:10: Backend Phase 1 Completion

### Overview
Implemented the core data models and the stable storage layer for the backend canister, following Phase 1 of the `backend.plan.md`.

### Key Components Implemented
1.  **Core Models (`src/backend/models/`):**
    *   Defined common types and enums (`VaultStatus`, `Role`, `ContentType`, etc.) in `common.rs`.
    *   Implemented Rust structs (`VaultConfig`, `VaultMember`, `VaultInviteToken`, `VaultContentItem`) derived from documentation and schemas, including necessary traits (`CandidType`, `Serialize`, `Deserialize`, `Clone`, `Debug`, `Default`).
2.  **Storage Layer (`src/backend/storage/`):**
    *   Set up memory management (`memory.rs`) using `ic-stable-structures` `MemoryManager` and defined `MemoryId` constants for different data structures.
    *   Created a generic CBOR-based `Storable` wrapper (`storable.rs`) for serializing/deserializing models into stable memory.
    *   Defined `StableBTreeMap` instances (`structures.rs`) for primary data (`VAULT_CONFIGS`, `VAULT_MEMBERS`, `INVITE_TOKENS`, `CONTENT_ITEMS`) and index maps (`CONTENT_INDEX`). Included key generation helpers.
    *   Implemented a basic stable cursor (`cursor.rs`) using `StableCell` for pagination or other sequential tracking.

### Dependencies
- Leveraged `candid`, `serde`, `ic-stable-structures`, `ic-cbor`.
- No new external dependencies added.

### Related Documentation
- [Backend Plan](mdc:plans/backend.plan.md) (Phase 1)
- [Backend Architecture](mdc:plans/backend.architecture.md) (Sections 4, Stable Memory Layout)
- [Technical Design](mdc:plans/tech.docs.md) (Section 5, Data Model & Storage)
- Schema files (`vault_config.schema.json`, etc.)

---

## 2024-07-25 18:30: Backend Phase 2 Completion

### Overview
Implemented the initial service layer logic for the backend canister, covering core operations for vaults, invites, uploads, and scheduling as defined in Phase 2 of the `backend.plan.md`.

### Key Components Implemented
1.  **`VaultService` (`src/backend/services/vault_service.rs`):**
    *   Implemented `create_new_vault` with basic plan -> quota mapping.
    *   Implemented `get_vault_config`.
    *   Implemented `update_existing_vault` with owner authorization check and basic field updates.
    *   Implemented `set_vault_status` with placeholder state transition validation.
    *   Added `VaultInitData` and `VaultUpdateData` structs as placeholders for API request payloads.
2.  **`InviteService` (`src/backend/services/invite_service.rs`):**
    *   Implemented `generate_new_invite` including owner check and placeholder Shamir index assignment.
    *   Implemented `claim_existing_invite` including token validation (status, expiry), member creation, and token status update.
    *   Added `MemberProfile` and `InviteClaimData` structs for API interactions.
3.  **`UploadService` (`src/backend/services/upload_service.rs`):**
    *   Implemented `begin_chunked_upload` including vault owner check, size validation, and in-memory `UploadState` creation.
    *   Implemented `upload_next_chunk` with index/size validation and in-memory chunk storage.
    *   Implemented `finish_chunked_upload` including chunk count verification, checksum validation, content item creation/storage, and content index update.
    *   Used an in-memory `HashMap` (`ACTIVE_UPLOADS`) for staging chunks.
4.  **`SchedulerService` (`src/backend/services/scheduler.rs`):**
    *   Implemented the main `perform_daily_maintenance` function.
    *   Added placeholder functions (`purge_expired_invites`, `check_vault_lifecycles`, `cleanup_stale_uploads`) for tasks to be executed by the scheduler.

### Dependencies
- Used `ic_cdk::api::time` for timestamps.
- Used `sha2` for checksum verification.
- Used `std::collections::HashMap` for in-memory upload staging.
- Relied heavily on previously defined models and storage modules.

### Notes & TODOs
- Many functions contain `TODO` comments marking areas needing refinement, such as detailed state validation, fetching related data (e.g., members for Shamir index), calculating expiry dates accurately, updating storage usage, and implementing efficient iteration for scheduler tasks.
- Error handling is basic; more specific errors might be needed.
- Authorization checks are currently minimal (mostly owner checks).

### Related Documentation
- [Backend Plan](mdc:plans/backend.plan.md) (Phase 2)
- [Backend Architecture](mdc:plans/backend.architecture.md)
- [Technical Design](mdc:plans/tech.docs.md)

---

## 2024-07-25 19:00: Backend Phases 3 & 4 Completion

### Overview
Completed the implementation of the Candid API layer (Phase 3) and the payment adapter stub (Phase 4) for the backend canister.

### Phase 3: Candid API & Entry Points
1.  **API Endpoints (`src/backend/api.rs`):**
    *   Defined Candid `#[update]` and `#[query]` functions for core vault, invite, and upload operations (`create_vault`, `get_vault`, `update_vault`, `generate_invite`, `claim_invite`, `begin_upload`, `upload_chunk`, `finish_upload`).
    *   Created corresponding request structs (`CreateVaultRequest`, `UpdateVaultRequest`, etc.) following ICP API conventions.
    *   Wired these endpoints to the respective functions in the `services` layer.
2.  **Rate Limiting (`src/backend/utils/rate_limit.rs`):**
    *   Implemented a token bucket algorithm using an in-memory `HashMap` keyed by `Principal`.
    *   Created the `rate_guard` function and applied it to all `#[update]` and `#[query]` endpoints in `api.rs`.

### Phase 4: Payment Adapter Stub
1.  **Payment Model (`src/backend/models/payment.rs`):**
    *   Defined the `PaymentSession` struct along with supporting enums (`PayMethod`, `PayState`) and types (`SessionId`, `E8s`).
    *   Included placeholder types for ICP Ledger interaction (`AccountIdentifier`, `Tokens`, `Memo`, etc.).
    *   Implemented an in-memory `HashMap` (`PAYMENT_SESSIONS`) and helper functions (`store_payment_session`, `with_payment_session_mut`, `with_payment_session`) for managing payment sessions.
2.  **Payment Service Stub (`src/backend/services/payment_service.rs`):**
    *   Implemented `initialize_payment_session` for `PayMethod::IcpDirect`, generating a session ID and storing the session state in memory.
    *   Implemented `verify_icp_payment` with state checks (expiry, status) and **placeholder logic** for querying the ICP ledger. Added logic to update the vault status via `vault_service::set_vault_status` upon successful (simulated) verification.
3.  **API Integration (`src/backend/api.rs`):**
    *   Added the `init_payment` and `verify_payment` endpoints, wiring them to the `PaymentService` functions.
    *   Defined `InitPaymentRequest` and `VerifyPaymentRequest` structs.

### Dependencies
- Used `ic_cdk::update`, `ic_cdk_macros::query` for endpoint definitions.
- Used `serde_bytes` for handling `Vec<u8>` in Candid.

### Notes & TODOs
- **Ledger Interaction:** The `verify_icp_payment` function currently uses **placeholder logic** and does **not** perform actual calls to the ICP ledger. This needs to be implemented using appropriate ledger interaction crates/methods.
- **Pay-to Principal:** The `initialize_payment_session` function uses the `caller` principal as a **placeholder** for `pay_to_principal`. A secure implementation must derive a unique subaccount/principal per session.
- Further implementation is needed for ChainFusion payments, detailed authorization logic in API endpoints, and completing TODOs in the service layer.

### Related Documentation
- [Backend Plan](mdc:plans/backend.plan.md) (Phase 3, Phase 4)
- [Backend Architecture](mdc:plans/backend.architecture.md) (Sections 6, 7, 13)
- [Technical Design](mdc:plans/tech.docs.md) (Sections 3, 4, 1.1)
- [icp-api-conventions.mdc](mdc:icp-api-conventions.mdc)

---

## 2024-07-25 19:45: Backend Phases 5 & 6 Implementation

### Overview
Implemented key security guards, metrics collection with certification, and admin API endpoints as defined in Phases 5 and 6 of the `backend.plan.md`.

### Phase 5: Security & Guards
1.  **Error Handling (`src/backend/error.rs`):** Expanded the `VaultError` enum with more specific variants for input validation (`InvalidInput`), admin guards (`AdminGuardFailed`), billing (`BillingRecordNotFound`), state issues (`InvalidState`), and checksums (`ChecksumMismatch`).
2.  **Cycle Guard (`src/backend/utils/guards.rs`):** Implemented the `check_cycles` function to verify sufficient canister balance against a threshold (`MIN_CYCLES_THRESHOLD`) before critical operations. Added `check_admin` helper.
3.  **Panic Hook (`src/backend/lib.rs`):** Configured `std::panic::set_hook` in the `init` function to log panic information, improving debuggability.
4.  **Input Validation (`src/backend/api.rs`):**
    *   Added the `validator` crate (assumption: added to Cargo.toml) and a `validate_request` helper function.
    *   Applied validation attributes (`#[validate(...)]`) to relevant request structs (`CreateVaultRequest`, `UpdateVaultRequest`, `GenerateInviteRequest`, `ClaimInviteRequest`, `BeginUploadRequest`, `FinishUploadRequest`, `RequestDownloadRequest`, `TriggerUnlockRequest`).
    *   Called `validate_request` at the beginning of corresponding API endpoint functions.

### Phase 6: Metrics & Admin APIs
1.  **Metrics Implementation (`src/backend/metrics.rs` & `storage/`):**
    *   Defined the `VaultMetrics` struct according to the architecture doc, including `Storable` implementation using CBOR.
    *   Added a `StableCell` named `METRICS` in `storage/structures.rs` to persist the global metrics.
    *   Added `get_metrics` and `update_metrics` helper functions in `storage/structures.rs` for safe access.
2.  **Certified Metrics Endpoint (`src/backend/api.rs`):**
    *   Implemented the `get_metrics` query endpoint.
    *   Fetches metrics from the `METRICS` stable cell.
    *   Retrieves the current `canister_balance128`.
    *   Serializes the combined response (`GetMetricsResponse`) using CBOR.
    *   Calls `set_certified_data` to make the metrics verifiable.
3.  **Billing Storage (`src/backend/models/billing.rs` & `storage/`):**
    *   Created the `BillingEntry` struct with `Storable` implementation.
    *   Added a `StableLog` named `BILLING_LOG` in `storage/structures.rs` for append-only billing records.
    *   Added an `add_billing_entry` helper function.
4.  **Admin API Endpoints (`src/backend/api.rs`):**
    *   Implemented the logic for `list_vaults` and `list_billing` endpoints.
    *   Used iterators (`VAULT_CONFIGS.iter()`, `BILLING_LOG.iter()`) with `skip()` and `take()` for pagination based on `ListRequest` parameters.
    *   Applied the `admin_guard` to restrict access to these endpoints.

### Dependencies
- Added (or assumes addition of) `validator` crate for input validation.

### Notes & TODOs
- The `ADMIN_PRINCIPAL` and `CRON_CALLER_PRINCIPAL` are currently hardcoded placeholders (`Principal::anonymous()`) and need to be securely configured (e.g., via init args).
- The `MIN_CYCLES_THRESHOLD` in `guards.rs` is also a placeholder.
- Placeholder `TODO` comments remain in `api.rs` for `list_vaults` and `list_billing` regarding potential improvements or alternative implementations, but the core logic is present.
- Further implementation needed for services layer logic that *updates* metrics and *adds* billing entries (e.g., in `vault_service`, `payment_service`).

### Related Documentation
- [Backend Plan](mdc:plans/backend.plan.md) (Phase 5, Phase 6)
- [Backend Architecture](mdc:plans/backend.architecture.md) (Sections 8, 11, 13)
- [Technical Design](mdc:plans/tech.docs.md)

---

## 2024-07-26 10:00: Backend Phases 7 & 8 Implementation

### Overview
Implemented the ChainFusion payment adapter integration (Phase 7) and the DFX workspace configuration (Phase 8) as outlined in the backend plan.

### Phase 7: ChainFusion Adapter & HTTP Outcalls
1.  **Billing Model (`src/backend/models/billing.rs`):** Extended `BillingEntry` struct to include fields (`original_token`, `original_amount`, `payment_method`, `swap_tx_hash`) necessary for multi-token payment tracking.
2.  **Payment Model (`src/backend/models/payment.rs`):** Updated `PaymentSession` struct to include ChainFusion-specific fields (`chainfusion_swap_address`, `chainfusion_source_token`, `chainfusion_source_amount`).
3.  **ChainFusion Adapter (`src/backend/adapter/chainfusion_adapter.rs`):**
    *   Created placeholder request/response structs (`ChainFusionInitRequest`, `ChainFusionInitResponse`, `ChainFusionStatusRequest`, `ChainFusionStatusResponse`) and status enum (`ChainFusionSwapStatus`).
    *   Implemented `initialize_chainfusion_swap` and `check_chainfusion_swap_status` async functions with **placeholder logic** simulating interaction with an external ChainFusion API (actual HTTP outcalls marked with `TODO`).
4.  **Payment Service Integration (`src/backend/services/payment_service.rs`):**
    *   Made `initialize_payment_session` async.
    *   Added logic to call `initialize_chainfusion_swap` when `PayMethod::ChainFusion` is selected.
    *   Refactored `verify_payment` to handle both `IcpDirect` and `ChainFusion` methods by calling dedicated verification functions (`verify_icp_ledger_payment`, `verify_chainfusion_payment`).
    *   Implemented `verify_chainfusion_payment`, which calls `check_chainfusion_swap_status` and includes **placeholder logic** for verifying the final ICP transaction after CF reports completion.
    *   Added logging using `ic_cdk::print` and `ic_cdk::eprintln` for payment flow visibility.

### Phase 8: Deployment & Ops Automation
1.  **DFX Configuration (`dfx.json`):** Created the `dfx.json` file at the workspace root, defining the `livault_backend` canister, build defaults, and network configurations for `local` and `ic`.

### Dependencies
- Relies on existing dependencies. No new external crates added.
- Uses `ic_cdk::api::management_canister::http_request` for planned (but not yet implemented) HTTP outcalls.

### Notes & TODOs
- **Placeholders:** Critical logic for actual HTTP outcalls in `chainfusion_adapter.rs` and actual ICP ledger verification in `payment_service.rs` (for both direct and ChainFusion methods) are still placeholders (`TODO`).
- **Integration:** Post-payment actions like updating vault status and adding billing entries in `verify_payment` are marked as `TODO` and need integration with `VaultService` and billing storage/service.
- **Configuration:** `CHAINFUSION_API_URL` needs to be set to the correct value.

### Related Documentation
- [Backend Plan](mdc:plans/backend.plan.md) (Phase 7, Phase 8)
- [Backend Architecture](mdc:plans/backend.architecture.md) (Section 7: Payment Adapter)
- [Technical Design](mdc:plans/tech.docs.md)

---

## 2024-07-26: Audit Log Implementation

**Overview:** Implemented the basic Audit Log functionality as described in the backend architecture documents.

**Key Components Implemented:**
1.  **Model:** Created `src/backend/models/audit_log.rs` defining the `AuditLogEntry` struct and `LogAction` enum.
2.  **Storage Structure:** Updated `src/backend/storage/structures.rs`:
    *   Added `AUDIT_LOGS: RefCell<StableBTreeMap<StorableString, Cbor<Vec<AuditLogEntry>>, Memory>>`.
    *   Key format: `audit:{vault_id}`.
    *   Value: `Cbor<Vec<AuditLogEntry>>`.
    *   Added helper functions: `create_audit_log_key`, `add_audit_log_entry`, `get_audit_log_entries`.
    *   The `add_audit_log_entry` function retrieves the current log vector, appends the new entry (setting timestamp and vault_id), and saves it back.
3.  **Module Exposure:** Updated `src/backend/storage/mod.rs` to re-export the new audit log structures and functions.

**Dependencies:** Relies on `ic-stable-structures`, `candid`, `serde`, `ic-cdk`.

**Architecture Links:**
*   [backend.architecture.md](plans/backend.architecture.md#4-stable-memory-storage-layout)
*   [tech.docs.md](plans/tech.docs.md#5-data-model--storage)

**Notes:**
*   The current implementation retrieves and saves the entire log vector for a vault on each `add_audit_log_entry` call. This might become inefficient for very long logs.
*   Log capping/rotation logic is not yet implemented and is marked with a `// TODO` in `structures.rs`.
*   Used the memory region previously allocated for `AUDIT_LOG_DATA_MEM_ID` for the `AUDIT_LOGS` BTreeMap.

## 2024-07-26

**Overview:** Added missing memory getter functions for billing logs to align `memory.rs` with usage in `structures.rs`.

**Key Components Implemented:**
- Added `BILLING_LOG_INDEX_MEM_ID` and `BILLING_LOG_DATA_MEM_ID` constants in `src/backend/storage/memory.rs`.
- Implemented `get_billing_log_index_memory()` and `get_billing_log_data_memory()` functions in `src/backend/storage/memory.rs`.

**Dependencies:**
- `ic-stable-structures`

**Relevant Docs:**
- [backend.architecture.md](plans/backend.architecture.md)
- [tech.docs.md](plans/tech.docs.md)

## Refactor: Internal IDs (ULID -> u64 + Principal) - 2024-07-27

**Overview:**
Initiated refactoring of internal identifiers for Invite Tokens, Content Items, and Upload Sessions. The previous approach using ULID-like strings generated via `utils::crypto::generate_ulid` presented potential Wasm compilation challenges and exposed internal details. The new strategy utilizes a dual-ID approach:

1.  **Internal ID:** A `u64` auto-incrementing counter managed via `StableCell` for efficient storage, iteration, and time-based sorting.
2.  **Exposed ID:** A unique `Principal` generated using `raw_rand` for use in all external APIs and user-facing contexts.

This aligns with the plan detailed in [`plans/refactor_internal_ids.plan.md`](mdc:plans/refactor_internal_ids.plan.md).

**Key Components to Implement/Modify:**

*   `storage/memory.rs`: Define new `MemoryId`s for counters and secondary indexes.
*   `storage/*`: Create modular storage logic (e.g., `storage/tokens.rs`) with counters, primary maps (`u64 -> Entity`), and secondary indexes (`Principal -> u64`).
*   `models/common.rs`: Update ID type aliases (`InviteTokenId`, `ContentId`, etc.) to `Principal`.
*   `models/*`: Update entity structs to use `Principal` IDs and add internal `u64` fields.
*   `utils/crypto.rs`: Implement `generate_unique_principal` using `generate_random_bytes`; remove `generate_ulid`.
*   `services/*`: Update logic for creation (get internal ID, generate Principal, store both, update index) and lookup/modification/deletion (use secondary index to find internal ID).
*   `api.rs`: Update endpoint signatures and request/response structs to use `Principal` type aliases for IDs.

**Dependencies:**

*   `ic-cdk`
*   `ic-stable-structures`
*   `candid`
*   `serde`

**Relevant Docs:**

*   Plan: [`plans/refactor_internal_ids.plan.md`](mdc:plans/refactor_internal_ids.plan.md)
*   Architecture: [`plans/backend.architecture.md`](mdc:plans/backend.architecture.md)

**Progress Log:**

*   **Step 1 (Memory IDs):** Completed adding new MemoryIDs and getters to `storage/memory.rs`.
*   **Step 2 (Storage Structures):** Completed creating modular storage files (`tokens.rs`, `content.rs`, `uploads.rs`) with counters, primary maps, secondary indexes, and helper functions. Updated `storage/mod.rs` and cleaned up `storage/structures.rs`.
*   **Step 3 (Model Structs & ID Types):** Completed updating ID type aliases in `models/common.rs` and modified relevant model structs (`VaultInviteToken`, `VaultContentItem`, `VaultConfig`, `VaultMember`) to use Principal types and include internal IDs.
*   **Step 4 (Principal Generation Utility):** Completed adding `generate_unique_principal` and removing `generate_ulid` in `utils/crypto.rs`.
*   **Step 5 (Service Logic - Creation):** Completed updating creation logic in `invite_service.rs` (`generate_new_invite`) and `upload_service.rs` (`finish_chunked_upload`, `begin_chunked_upload`).
*   **Step 6 (Service Logic - Lookup/Mod/Del):** Completed updating lookup/modification logic in `invite_service.rs` (`claim_existing_invite`, `revoke_invite_token`). Other services (`vault_service`, `upload_service`) still need updates for lookup/delete operations based on Principal IDs.
*   **Step 7 (API Layer):** Completed updating function signatures and request/response structs in `api.rs` to use Principal-based ID types.
*   **Next:** Review changes, address TODOs, compile, and test. 

---

## 2024-07-27: Review Payment Service & Models

**Overview:** Reviewed `src/backend/services/payment_service.rs` and `src/backend/models/payment.rs` against `storage.md`, `backend.architecture.md`, and `prd.md`.

**Findings:**
-   **Alignment:** The service logic, state transitions, method handling (ICP Direct, ChainFusion stubs), and interactions with vault/billing services align well with the documented architecture and requirements.
-   **Storage:**
    -   Billing entries are correctly stored using `storage::billing`.
    -   Vault status updates correctly call `vault_service`.
    -   **Discrepancy Noted:** Payment sessions (`PaymentSession`) are currently stored in volatile memory (`thread_local! HashMap` in `models/payment.rs`) via helper functions. While `PaymentSession` implements `Storable`, no stable map is defined in `storage.md`. The architecture doc mentioned volatile storage with snapshots; the current implementation uses only volatile.
    -   **Decision:** Volatile storage is acceptable for MVP given the short-lived nature of sessions, but stable storage might be needed later if sessions must survive upgrades.
-   **TODOs:** The remaining TODO in `verify_chainfusion_payment` is expected pending adapter implementation.

**Relevant Docs:**
*   [`docs/storage.md`](mdc:docs/storage.md)
*   [`plans/backend.architecture.md`](mdc:plans/backend.architecture.md)
*   [`plans/prd.md`](mdc:plans/prd.md)

## 2024-07-27: TODO Cleanup & Storage Documentation

**Overview:** Addressed several TODO items identified in the codebase and documentation, focusing on guards, configuration, and storage layer modularization. Created documentation for the storage layer.

**Key Components Implemented/Updated:**
1.  **Guards (`src/backend/utils/guards.rs`):**
    *   Implemented `owner_or_heir_guard`, `member_guard`, and `role_guard` using efficient lookups/iteration against the `MEMBERS` map.
    *   Made `MIN_CYCLES_THRESHOLD`, `ADMIN_PRINCIPAL`, and `CRON_PRINCIPAL` configurable via `InitArgs` and stored them in `StableCell`s (`storage/config.rs`).
    *   Updated guards to use the configured values.
2.  **Models (`src/backend/models/`):**
    *   Added `Verified` status to `MemberStatus` enum (`common.rs`).
    *   Created `InitArgs` struct (`init.rs`).
    *   Created `UploadSession` struct and `UploadStatus` enum (`upload_session.rs`).
3.  **Storage (`src/backend/storage/`):**
    *   **Modularization:** Created dedicated modules for `members`, `vault_configs`, `audit_logs`, `metrics`, `billing`, and `content_index`, moving definitions and helper functions out of `structures.rs`.
    *   **Members:** Refactored `MEMBERS` map key to `(VaultId, PrincipalId)` for efficient querying.
    *   **Content:** Added `update_content` function.
    *   **Uploads:** Added `UPLOAD_CHUNKS_MAP` and functions (`save_chunk`, `get_chunk`, `delete_chunks`) for managing chunk data.
    *   **Audit Logs:** Added `compact_log` function.
    *   Updated `mod.rs` to reflect the new module structure and re-exports.
4.  **Initialization (`src/backend/lib.rs`):**
    *   Updated `init` function to accept `InitArgs` and initialize configuration storage.
5.  **Documentation (`docs/`):**
    *   Created comprehensive documentation for the storage layer in `docs/storage.md`.
    *   Updated `docs/todo.md` to mark completed items and refine descriptions.

**Dependencies:** No new external dependencies.

**Relevant Docs:**
*   [`docs/storage.md`](mdc:docs/storage.md) (New)
*   [`docs/todo.md`](mdc:docs/todo.md) (Updated)

## 2024-07-27: Implement ChainFusion Adapter HTTP Outcalls

**Overview:** Implemented the actual HTTP outcall logic in `src/backend/adapter/chainfusion_adapter.rs` to replace the placeholder functions, addressing the corresponding TODO items.

**Key Components Implemented/Updated:**
-   **Imports:** Added `serde_json` for handling request/response bodies.
-   **Constants:** Defined `INIT_SWAP_PATH`, `SWAP_STATUS_PATH`, and `MAX_RESPONSE_BYTES`.
-   **`initialize_chainfusion_swap`:**
    *   Serializes `ChainFusionInitRequest` to JSON.
    *   Constructs `CanisterHttpRequestArgument` for a POST request to the `/init_swap` endpoint with appropriate headers and body.
    *   Calls `ic_cdk::api::management_canister::http_request::http_request` with configured cycles.
    *   Handles the response: checks status code, deserializes the JSON body into `ChainFusionInitResponse`, maps errors to `VaultError::HttpError` or `VaultError::SerializationError`.
-   **`check_chainfusion_swap_status`:**
    *   Serializes `ChainFusionStatusRequest` to JSON.
    *   Constructs `CanisterHttpRequestArgument` for a POST request (assuming status check requires POST, adaptable if GET is needed) to the `/swap_status` endpoint.
    *   Calls `ic_cdk::api::management_canister::http_request::http_request`.
    *   Handles the response similarly, deserializing into `ChainFusionStatusResponse`.

**Dependencies:**
-   `ic-cdk` (for `http_request`)
-   `serde`, `serde_json` (for JSON serialization/deserialization)

**Relevant Docs:**
*   [`docs/todo.md`](mdc:docs/todo.md) (Marked adapter HTTP outcall task as done)
*   [`src/backend/adapter/chainfusion_adapter.rs`](mdc:src/backend/adapter/chainfusion_adapter.rs)

---

## 2024-07-27: Implement Robust ChainFusion ICP Verification

**Overview:** Updated the `verify_chainfusion_payment` function in `src/backend/services/payment_service.rs` to perform a robust verification of the ICP transaction after the ChainFusion adapter reports completion.

**Key Components Implemented/Updated:**
-   **`verify_chainfusion_payment`:**
    *   Removed the placeholder `let icp_tx_verified = true;`.
    *   When `check_chainfusion_swap_status` returns `ChainFusionSwapStatus::Completed`, the function now calls the existing `verify_icp_ledger_payment(session).await` function.
    *   This ensures that the primary verification relies on checking the actual balance of the designated subaccount (`session.pay_to_account_id`) against the expected `session.amount_e8s`.
    *   The `icp_tx_hash` provided by the ChainFusion adapter is preserved (or generated if missing) and returned upon successful balance verification, primarily for auditing/logging purposes.
    *   Added specific error logging if the ChainFusion adapter reports completion but the `verify_icp_ledger_payment` balance check fails.

**Dependencies:**
-   Relies on existing dependencies within `payment_service.rs` and the `chainfusion_adapter`.

**Relevant Docs:**
*   [`docs/todo.md`](mdc:docs/todo.md) (Marked ChainFusion verification task as done)
*   [`src/backend/services/payment_service.rs`](mdc:src/backend/services/payment_service.rs)

---

## 2024-07-27: Vault Service TODOs (Part 1)

**Overview:** Addressed several TODO items in `src/backend/services/vault_service.rs`.

**Key Components Implemented/Updated:**
-   **State Validation:** Implemented robust state transition validation in `set_vault_status` based on expected lifecycle.
-   **Unlock Conditions:** Removed placeholder validation TODO (basic structure checked by Candid), updated `check_unlock_conditions` to use `last_accessed_by_owner`, and implemented approval check logic using a placeholder call to `storage::approvals::get_approval_status`.
-   **Plan Change:** Refined TODO comment regarding prorate calculation, noting dependency on payment service.
-   **Deletion:** Added `delete_vault` function skeleton with owner authorization and status check, including placeholder comments for cleanup steps.
-   **Get Vaults by Owner:** Added documentation note about inefficiency and need for secondary index.

**Dependencies:** Relies on placeholder `storage::approvals::get_approval_status`.

**Relevant Docs:**
*   [`docs/todo.md`](mdc:docs/todo.md) (Updated) 

---

## 2024-07-27: Fix VaultId Generation Consistency

**Overview:** Fixed an inconsistency in `vault_service.rs` where `create_new_vault` was using `generate_ulid()` instead of `generate_unique_principal()` for `vault_id` generation.

**Key Components Implemented/Updated:**
-   Updated `create_new_vault` to use `generate_unique_principal()`.
-   Verified model definitions and storage layer usage align with using `Principal` for `VaultId`.

**Relevant Docs:**
*   [`src/backend/services/vault_service.rs`](mdc:src/backend/services/vault_service.rs)

---

## 2024-07-28: Remove ChainFusion for MVP Focus

**Overview:** Removed ChainFusion-related code from the backend to align with the MVP scope, which only includes direct ICP payments.

**Key Components Implemented/Updated:**
-   **Models (`models/payment.rs`, `models/billing.rs`):**
    *   Removed `PayMethod::ChainFusion` enum variant.
    *   Removed ChainFusion-specific fields from `PaymentSession` struct (e.g., `chainfusion_swap_address`, `chainfusion_source_token`, `chainfusion_source_amount`). Updated `Storable` bound accordingly.
    *   Removed ChainFusion-specific fields from `BillingEntry` struct (e.g., `original_token`, `original_amount`, `swap_tx_hash`). Updated `payment_method` to always be "IcpDirect".
-   **Payment Service (`services/payment_service.rs`):**
    *   Removed logic handling `PayMethod::ChainFusion` during `initialize_payment_session`.
    *   Removed the `verify_chainfusion_payment` function.
    *   Simplified `verify_payment` logic to only call `verify_icp_ledger_payment`.
    *   Removed ChainFusion fields from `BillingEntry` creation.
    *   Removed imports related to `chainfusion_adapter`.
-   **ChainFusion Adapter (`adapter/chainfusion_adapter.rs`):**
    *   Deleted the entire file as it is no longer needed.
-   **Payment Init Request (`services/payment_service.rs`):**
    *   Removed the `method` field from `PaymentInitRequest` as it's implicitly `IcpDirect`.

**Dependencies:** Removed dependency on `chainfusion_adapter`.

**Relevant Docs:**
*   [`src/backend/models/payment.rs`](mdc:src/backend/models/payment.rs)
*   [`src/backend/models/billing.rs`](mdc:src/backend/models/billing.rs)
*   [`src/backend/services/payment_service.rs`](mdc:src/backend/services/payment_service.rs)
*   [`docs/progress.md`](mdc:docs/progress.md)

---

## 2024-07-28: Verify Payment Verification Logic

**Overview:** Reviewed official ICP ledger documentation regarding canister interactions and payment verification patterns.

**Findings:**
-   The recommended pattern for verifying payments received by a canister involves using the `query_blocks` method on the ledger canister, not `account_balance`.
-   The current implementation in `src/backend/services/payment_service.rs` uses `account_balance`, which is insufficient as it only checks the current balance and doesn't confirm a specific transaction occurred within the required timeframe.

**Action Item:** The `verify_icp_ledger_payment` function needs to be refactored to use `query_blocks` to search for a specific transaction matching the payment session details (amount, target subaccount, time window).

**Relevant Docs:**
*   [Using the ICP ledger - Interacting from a canister](https://internetcomputer.org/docs/current/developer-docs/defi/token-ledgers/icp-ledger/usage#interacting-with-icp-from-a-canister-inter-canister-calls-via-ic-cdk)
*   [Ledger canister specification - query_blocks](https://internetcomputer.org/docs/current/references/ledger#query_blocks)
*   [`src/backend/services/payment_service.rs`](mdc:src/backend/services/payment_service.rs)
*   [`docs/todo.md`](mdc:docs/todo.md)

--- 

## 2024-07-28: Refactor Payment Verification with ic-ledger-types

**Overview:** Refactored the payment verification logic in `src/backend/services/payment_service.rs` to use the official `ic-ledger-types` crate for interacting with the ICP ledger's `query_blocks` method.

**Key Components Implemented/Updated:**
-   **Imports:** Added necessary imports from `ic_ledger_types` (`GetBlocksArgs`, `QueryBlocksResponse`, `Block`, `Transaction`, `Operation`, etc.).
-   **Struct Definitions:** Removed manually defined structs related to `query_blocks` interaction.
-   **`verify_icp_ledger_payment` Function:**
    *   Updated function logic to use types from `ic_ledger_types` for arguments (`GetBlocksArgs`) and response handling (`QueryBlocksResponse`).
    *   Implemented decoding of `EncodedBlock` (from `QueryBlocksResponse.blocks`) into `ic_ledger_types::Block` using `candid::Decode`.
    *   Updated transaction checking logic to use the fields and methods from `ic_ledger_types::Block`, `ic_ledger_types::Transaction`, `ic_ledger_types::Operation`, and `ic_ledger_types::Tokens` (e.g., `amount.get_e8s()`).
-   **API Layer:** Updated `VerifyPaymentRequest` in `src/backend/api.rs` to accept an optional `block_index` from the frontend.
-   **Service Layer:** Updated `verify_payment` and `verify_icp_ledger_payment` signatures to accept the optional `block_index` and prioritize querying that specific block.

**Dependencies:** Relies on `ic-ledger-types` crate (assumed present in `Cargo.toml`).

**Relevant Docs:**
*   [`src/backend/services/payment_service.rs`](mdc:src/backend/services/payment_service.rs)
*   [`src/backend/api.rs`](mdc:src/backend/api.rs)
*   [`docs/progress.md`](mdc:docs/progress.md)
*   [`docs/todo.md`](mdc:docs/todo.md)
*   [ic-ledger-types Documentation](https://docs.rs/ic-ledger-types/latest/ic_ledger_types/)

--- 

## 2024-07-29: Implement Vault Plan Change Logic

**Overview:** Implemented the logic for changing a vault's plan, including handling prorated costs for upgrades and integrating with the payment system.

**Key Components Implemented/Updated:**
-   **Models (`models/payment.rs`):**
    *   Added `PaymentPurpose` enum (`InitialVaultCreation`, `PlanUpgrade { new_plan: String }`).
    *   Added `purpose: PaymentPurpose` field to `PaymentSession` struct.
-   **Payment Service (`services/payment_service.rs`):**
    *   Updated `initialize_payment_session` to accept and store an optional `PaymentPurpose`.
    *   Refactored `verify_payment`:
        *   After successful ledger verification, checks `session.purpose`.
        *   If `PlanUpgrade`, calls `vault_service::finalize_plan_change`.
        *   If `InitialVaultCreation`, calls `vault_service::set_vault_status`.
    *   Updated billing entry creation in `trigger_post_confirmation_actions` to reflect the payment purpose.
-   **Vault Service (`services/vault_service.rs`):**
    *   Added helper `get_plan_base_price_e8s` based on architecture doc pricing.
    *   Added helper `calculate_prorated_upgrade_cost` implementing the prorate formula.
    *   Refactored `update_vault_config`:
        *   Returns `Result<Option<PaymentSession>, VaultError>`.
        *   Calculates upgrade cost.
        *   If cost > 0, calls `initialize_payment_session` with `PlanUpgrade` purpose and returns the session.
        *   If cost <= 0 (downgrade/same), applies plan change directly and returns `Ok(None)`.
    *   Added internal function `finalize_plan_change` to apply plan updates after successful upgrade payment.
-   **API Layer (`api.rs`):**
    *   Updated the signature and return type of the `update_vault` endpoint to `Result<Option<PaymentSession>, VaultError>`.

**Dependencies:** Relies on existing dependencies.

**Relevant Docs:**
*   [`plans/backend.architecture.md`](mdc:plans/backend.architecture.md) (Section 5.6)
*   [`docs/todo.md`](mdc:docs/todo.md)
*   [`src/backend/models/payment.rs`](mdc:src/backend/models/payment.rs)
*   [`src/backend/services/payment_service.rs`](mdc:src/backend/services/payment_service.rs)
*   [`src/backend/services/vault_service.rs`](mdc:src/backend/services/vault_service.rs)
*   [`src/backend/api.rs`](mdc:src/backend/api.rs)

--- 
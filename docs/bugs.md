# Backend Build Issues (wasm32-unknown-unknown)

This document tracks issues encountered during the `cargo build --target wasm32-unknown-unknown --all --profile dev` process and the steps taken to resolve them.

## Initial Build Attempt (YYYY-MM-DD HH:MM GMT+7)

Command: `cargo build --target wasm32-unknown-unknown --all --profile dev`

Result: Failed with 85 errors and 32 warnings.

---

## Issues & Fixes

### 1. `serde_json` Crate Missing

-   **Error:** `error[E0432]: unresolved import serde_json` in `src/backend/adapter/chainfusion_adapter.rs`. `error: no external crate serde_json`.
-   **File:** `src/backend/adapter/chainfusion_adapter.rs`
-   **Reason:** The `serde_json` crate is used for serializing/deserializing requests to the ChainFusion API but is not declared as a dependency in the backend's `Cargo.toml`.
-   **Fix:** Added `serde_json = "1.0"` to `[dependencies]` in `src/backend/Cargo.toml`.
-   **Status:** Fixed.

### 2. Missing `Serialize` derive macro import

-   **Error:** `error: cannot find derive macro 'Serialize' in this scope` and `error[E0405]: cannot find trait 'Serialize' in this scope`.
-   **File:** `src/backend/api.rs`
-   **Reason:** Several structs derive `Serialize` but the trait/macro isn't imported.
-   **Fix:** Added `use serde::Serialize;` to `src/backend/api.rs`.
-   **Status:** Fixed.

### 3. Incorrect `validator` custom function syntax

-   **Error:** `error: Unexpected type 'string'` for `#[validate(custom = "validate_principal")]`.
-   **File:** `src/backend/api.rs`
-   **Reason:** The `validator` crate expects the function path directly, not as a string literal.
-   **Fix:** Changed `#[validate(custom = "validate_principal")]` to `#[validate(custom = validate_principal)]`.
-   **Status:** Fixed.

### 4. Redefined `MemberProfile` struct

-   **Error:** `error[E0255]: the name 'MemberProfile' is defined multiple times`.
-   **File:** `src/backend/api.rs`
-   **Reason:** `MemberProfile` was imported from `invite_service` and also defined locally.
-   **Fix:** Removed the local definition in `api.rs`. Updated imports and endpoint signatures to use `crate::models::vault_member::VaultMember` (aliased as `ApiMemberProfile`).
-   **Status:** Fixed.

### 5. Unresolved Imports in `api.rs`

-   **Errors:** Various `error[E0432]: unresolved import` and `error[E0433]: failed to resolve: use of undeclared crate or module`.
-   **File:** `src/backend/api.rs`
-   **Reason:** Incorrect paths (missing `crate::`), referencing modules directly instead of specific items, or items moved/renamed.
-   **Fixes:**
    -   Prefixed imports and direct calls with `crate::` (e.g., `crate::services::...`, `crate::storage::...`).
    -   Corrected storage imports (e.g., `crate::storage::audit_logs::add_audit_log_entry`, `crate::storage::billing::query_billing_entries`).
    -   Assumed `member_or_heir_guard` was meant to be `owner_or_heir_guard` based on context.
    -   Used `SessionId` from `crate::models::common`.
-   **Status:** Fixed.

### 6. Private Enum Imports in `api.rs`

-   **Error:** `error[E0603]: enum import 'PayMethod' is private` and `error[E0603]: enum import 'MemberStatus' is private`.
-   **File:** `src/backend/api.rs`
-   **Reason:** Trying to import enums indirectly via modules instead of their public definition path.
-   **Fix:** Imported directly: `crate::models::payment::PayMethod`, `crate::models::common::MemberStatus`.
-   **Status:** Fixed.

### 7. Incorrect `println!` Format String Argument (`payment_service.rs`)

-   **Error:** `error: 1 positional argument in format string, but no arguments were given`.
-   **File:** `src/backend/services/payment_service.rs` (line 242).
-   **Reason:** The format string `"INFO: Billing entry added at index {}."` requires an argument.
-   **Fix:** Passed `log_index` to the `print` macro: `ic_cdk::print(format!("INFO: Billing entry added at index {}.", log_index))`. Replaced `println!` with `print` for consistency.
-   **Status:** Fixed.

### 8. Unresolved Imports (`payment_service.rs`)

-   **Errors:** `error[E0432]: unresolved import crate::storage::payment`, `crate::utils::account_identifier`, `crate::adapter::ledger_adapter`, `crate::models::vault_service`.
-   **File:** `src/backend/services/payment_service.rs`.
-   **Reason:** Modules/items don't exist or path is incorrect.
-   **Fixes:**
    -   Imported payment session storage helpers from `crate::models::payment`.
    -   Removed unused imports for `account_identifier` and `ledger_adapter`.
    -   Corrected `vault_service` import to `crate::services::vault_service`.
    -   Imported `ic_ledger_types` directly.
    -   Ensured `crate::storage::billing` is used for billing calls.
-   **Status:** Fixed.

### 9. `AccountIdentifier` Parsing (`payment_service.rs`)

-   **Error:** `error[E0277]: the trait bound 'ic_ledger_types::AccountIdentifier: FromStr' is not satisfied`.
-   **File:** `src/backend/services/payment_service.rs` (line 280).
-   **Reason:** Cannot directly parse `AccountIdentifier`. It needs to be reconstructed from the canister ID and the subaccount used during session creation.
-   **Fix:**
    -   Added `pay_to_subaccount: Option<[u8; 32]>` to `PaymentSession` in `models/payment.rs`.
    -   Stored the generated subaccount bytes in `initialize_payment_session`.
    -   Reconstructed `AccountIdentifier` using `api::id()` and the stored `pay_to_subaccount` in `verify_icp_ledger_payment`, removing the `.parse()` call.
-   **Status:** Fixed.

### 10. `raw_rand()` Type Mismatch (`payment_service.rs`)

-   **Error:** `error[E0308]: '?' operator has incompatible types` (expected `[u8; 32]`, found `Vec<u8>`).
-   **File:** `src/backend/services/payment_service.rs` (line 45).
-   **Reason:** `raw_rand().await` returns `Result<(Vec<u8>,), _>`.
-   **Fix:** Modified `derive_random_subaccount` to extract the `Vec<u8>` and use `try_into()` to convert it to `[u8; 32]`, handling potential length errors.
-   **Status:** Fixed.

### 11. `BillingEntry` Field/Type Mismatches (`payment_service.rs`)

-   **Error:** `error[E0560]: struct 'BillingEntry' has no field named ...` and `error[E0308]: mismatched types`.
-   **File:** `src/backend/services/payment_service.rs` (lines 231-239).
-   **Reason:** Code assigning fields to `BillingEntry` didn't match the struct definition in `models/billing.rs`.
-   **Fix:** Updated field assignments to use correct names (`date`, `vault_id` (as String), `tx_type`, `ledger_tx_hash`, etc.) and types (converting `PayMethod` enum to String). Added missing `tx_type` and optional `related_principal`.
-   **Status:** Fixed.

### 12. Unresolved `SessionId` Import (`chainfusion_adapter.rs`)

-   **Error:** `error[E0432]: unresolved import 'crate::models::payment::SessionId'`.
-   **File:** `src/backend/adapter/chainfusion_adapter.rs`.
-   **Reason:** `SessionId` is likely defined in `models::common`.
-   **Fix:** Changed import to `use crate::models::common::{..., SessionId};`.
-   **Status:** Fixed.

### 13. Missing `VaultError` Variants (`chainfusion_adapter.rs`)

-   **Error:** `error[E0599]: no variant or associated item named 'SerializationError' / 'HttpError'`.
-   **Files:** `src/backend/adapter/chainfusion_adapter.rs`, `src/backend/error.rs`.
-   **Reason:** Adapter uses error types not defined in `VaultError` enum.
-   **Fix:** Added `SerializationError(String)` and `HttpError(String)` variants to `VaultError` enum and its `Display` impl in `error.rs`.
-   **Status:** Fixed.

### 14. Mismatched HTTP Header Type (`chainfusion_adapter.rs`)

-   **Error:** `error[E0308]: mismatched types` (expected `HttpHeader`, found `(String, String)`).
-   **File:** `src/backend/adapter/chainfusion_adapter.rs`.
-   **Reason:** `http_request` expects `Vec<HttpHeader>` where `HttpHeader` is a struct, not a tuple.
-   **Fix:** Changed `(String::from("Content-Type"), String::from("application/json"))` to `HttpHeader { name: String::from("Content-Type"), value: String::from("application/json") }`. Imported `HttpHeader` struct.
-   **Status:** Fixed.

--- 
# Backend TODO List

*Generated: 2024-07-26* -> **Validated: 2024-07-29**

This list compiles outstanding tasks, identified inconsistencies needing resolution, and areas requiring further implementation based on a review of backend documentation, plans, progress logs, and codebase structure.

## Phase 1: Core Models & Storage
-   **[X] Task 1.3:** Perform manual validation of model serialization/deserialization and storage round-trip. (**Note:** Basic storage functions implemented, manual testing not performed by AI).
-   **[X] Storage Implementation:** Ensure `src/backend/storage/structures.rs` implements all required stable memory segments as defined in `backend.architecture.md` (Section 4), including maps for Approvals, Audit Logs, Metrics, and Upload Staging. (Modules created, structures implemented).
-   **[ ] Documentation:** Update `tech.docs.md` (Section 5) storage description to align with `backend.architecture.md`. (Recommend referencing `docs/storage.md` instead).

## Phase 2: Services Layer
-   **[ ] Task 2.5:** Perform manual edge-case testing of implemented services (`VaultService`, `InviteService`, `UploadService`).
-   **[X] `VaultService`:** Refine state transition validation logic in `set_vault_status` based on the canonical states in `prd.md`.
-   **[X] `VaultService`:** Implement storage usage tracking and updates. (**Note:** Helper `update_storage_usage` implemented, needs to be called by content add/remove logic in other services).
-   **[~] `VaultService`:** Flesh out detailed authorization logic beyond basic owner checks. (Basic role checks added for some actions, more granularity may be needed).
-   **[X] `VaultService`:** Implement function to be called by `PaymentService` to update vault status post-payment.
-   **[X] `InviteService`:** Implement Shamir index assignment logic (fetching used indices).
-   **[X] `InviteService`:** Implement calculation of token expiry dates accurately.
-   **[X] `InviteService`:** Implement Shamir secret sharing using `vsss-rs`. (**Note:** Uses placeholder secret generation, needs update -> **Plan outlined in comments, placeholders noted**).
-   **[!] `UploadService`:** Consider moving upload staging from in-memory `HashMap` to stable memory. (**CRITICAL:** Service still uses in-memory map; `storage::uploads` has stable chunk storage but is unused by service).
-   **[ ] `SchedulerService`:** Implement the actual logic within `purge_expired_invites`, `check_vault_lifecycles`, `cleanup_stale_uploads`, `compact_audit_logs`.
-   **[ ] `SchedulerService`:** Implement efficient iteration methods for scheduler tasks over stable storage.
-   **[ ] `SchedulerService`:** Integrate with `ic-cdk-timers` or establish the Cloudflare worker interaction.
-   **[~] Error Handling:** Refine error handling across services to provide more specific `VaultError` variants as needed. (Partially addressed).

## Phase 3: Candid API & Entry Points
-   **[ ] Task 3.4:** Perform manual happy-path verification of API endpoints using `dfx`.
-   **[~] API Definition:** Ensure `api.rs` fully implements the Candid interface defined in `backend.architecture.md` (Section 6). (`request_download` missing, others present).
-   **[ ] Documentation:** Update or remove the Candid stub in `tech.docs.md` (Section 3) to align with `backend.architecture.md`. (Recommend removal).
-   **[~] Authorization:** Implement detailed authorization logic within API endpoints beyond basic owner checks. (Basic guards exist, more specific needed).

## Phase 4: Payment Adapter - ICP Direct Completed
-   **[ ] Task 4.3:** Perform manual end-to-end testing of the ICP Direct payment flow stub.
-   **[X] Ledger Interaction:** Implement actual ICP ledger query logic in `verify_icp_ledger_payment` using appropriate crates/methods (replace placeholder).
-   **[X] Refactor Payment Verification:** Change `verify_icp_ledger_payment` to use `query_blocks` instead of `account_balance` for robust verification, based on ICP ledger documentation review (2024-07-28).
-   **[X] Use ic-ledger-types:** Refactored `verify_icp_ledger_payment` to use standard types from `ic-ledger-types` crate (2024-07-28).
-   **[X] Pay-to Principal:** Implement secure derivation of a unique subaccount/principal per payment session in `initialize_payment_session` (replace placeholder using `caller`).

## Phase 5: Security & Guards - COMPLETED (2024-07-25)

## Phase 6: Metrics & Admin APIs - COMPLETED (2024-07-25)

## Phase 7: ChainFusion Adapter & HTTP Outcalls - REMOVED (2024-07-28)
-   **(N/A)** Tasks removed as ChainFusion is out of scope for MVP.

## Phase 8: Deployment & Ops Automation - Partially Completed
-   **[X] Task 8.1:** Create `dfx.json` configuration.
-   **[ ] Task:** Implement CI/CD pipeline steps for deployment and cycle top-up (as per `tech.docs.md`).

## General / Integration
-   **[X] Billing Integration:** Implement logic in `PaymentService` to call `storage::billing::add_billing_entry` upon successful payment verification.
-   **[X] Vault Status Integration:** Implement logic in `PaymentService` to call `VaultService` to update vault status upon successful payment verification.
-   **[ ] Dependencies:** Update `backend.plan.md` dependency list to match `Cargo.toml`.
-   **[X] Vault Lifecycle:** Ensure `VaultStatus` enum reflects PRD states and transition logic is correct.
-   **[ ] Review Open Questions:** Address the open questions listed in `backend.plan.md`.
-   **[X] Update Progress:** Regularly update `docs/progress.md` and `backend.tracking.md` as tasks are completed. (Log seems current).

## Code TODO
-   **[ ] API Configuration:** Configure `ADMIN_PRINCIPAL`, `CRON_CALLER` in `api.rs` (load from init/storage).
-   **[ ] API Endpoints:** Implement `request_download` endpoint in `api.rs`.
-   **[ ] API Authorization:** Review and apply specific member/role guards in `api.rs` where needed (beyond owner/admin).
-   **[ ] `InviteService` Implementation:**
    *   Check vault state allows invites.
    *   **Refine Shamir secret handling (replace placeholder generation).** -> **Placeholder noted in plan.**
    *   **Refine Shamir parameter `n` (total shares) based on vault plan/config.** -> **Planned.**
    *   Add function to revoke an invite token. -> **Planned.**
    *   Add function to list members for a vault. -> **Planned.**
    *   Add function to get member details. -> **Planned.**
    *   Add logic based on `finalize_setup` requirements from arch doc (if applicable).
-   **[!] `UploadService` Implementation:**
    *   **Refactor to use `storage::uploads` for chunk storage instead of in-memory `HashMap`.**
    *   Store initiator `Principal` in `UploadSession` for auth checks.
    *   Check vault status allowing uploads (e.g., Active).
    *   Use `caller` for auth/quota in `begin_chunked_upload`.
    *   Check upload size against `vault_config.storage_quota_bytes`.
    *   Validate `mime_type` based on `content_type`.
    *   **Call `vault_service::update_storage_usage` on upload completion.**
    *   Implement get/delete/list content item functions (or move to appropriate service). **Note:** Should call `vault_service::update_storage_usage` on deletion.
-   **[ ] `SchedulerService` Implementation:**
    *   Implement iteration logic for cleanup tasks (`purge_expired_invites`, `check_vault_lifecycles`, `cleanup_stale_uploads`).
    *   Implement `compact_audit_logs` call.
    *   Add other periodic tasks (e.g., recalculate metrics).
-   **[ ] `VaultService` Implementation:**
    *   (Shamir index logic moved to InviteService).
# Backend TODO List

*Generated: 2024-07-26*

This list compiles outstanding tasks, identified inconsistencies needing resolution, and areas requiring further implementation based on a review of backend documentation, plans, progress logs, and codebase structure.

## Phase 1: Core Models & Storage
-   **[ ] Task 1.3:** Perform manual validation of model serialization/deserialization and storage round-trip. (`backend.tracking.md` status needs clarification).
-   **[ ] Storage Implementation:** Ensure `src/backend/storage/structures.rs` implements all required stable memory segments as defined in `backend.architecture.md` (Section 4), including maps for Approvals, Audit Logs, Metrics, and Upload Staging. (Metrics is done).
-   **[ ] Documentation:** Update `tech.docs.md` (Section 5) storage description to align with `backend.architecture.md`.

## Phase 2: Services Layer
-   **[ ] Task 2.5:** Perform manual edge-case testing of implemented services (`VaultService`, `InviteService`, `UploadService`).
-   **[ ] `VaultService`:**
    *   Refine state transition validation logic in `set_vault_status` based on the canonical states in `prd.md`.
    *   Implement logic to fetch related data where needed (e.g., members for Shamir index calculation).
    *   Implement storage usage tracking and updates.
    *   Flesh out detailed authorization logic beyond basic owner checks.
    *   Implement function to be called by `PaymentService` to update vault status post-payment.
-   **[ ] `InviteService`:**
    *   Implement actual Shamir index assignment logic (fetching used indices).
    *   Implement calculation of token expiry dates accurately.
-   **[ ] `UploadService`:**
    *   Consider moving upload staging from in-memory `HashMap` to stable memory as suggested in `backend.architecture.md` (Segment: Upload Staging) for persistence across upgrades.
-   **[ ] `SchedulerService`:**
    *   Implement the actual logic within `purge_expired_invites`, `check_vault_lifecycles`, `cleanup_stale_uploads`.
    *   Implement efficient iteration methods for scheduler tasks over stable storage.
    *   Integrate with `ic-cdk-timers` or establish the Cloudflare worker interaction.
-   **[ ] Error Handling:** Refine error handling across services to provide more specific `VaultError` variants as needed. (Partially addressed in Phase 5)

## Phase 3: Candid API & Entry Points
-   **[ ] Task 3.4:** Perform manual happy-path verification of API endpoints using `dfx`.
-   **[ ] API Definition:** Ensure `api.rs` fully implements the Candid interface defined in `backend.architecture.md` (Section 6), including missing endpoints (`request_download`) and detailed request/response types.
-   **[ ] Documentation:** Update or remove the Candid stub in `tech.docs.md` (Section 3) to align with `backend.architecture.md`.
-   **[ ] Authorization:** Implement detailed authorization logic within API endpoints beyond basic owner checks.

## Phase 4: Payment Adapter - Partially Completed (Phase 7 extended)
-   **[ ] Task 4.3:** Perform manual end-to-end testing of the ICP Direct payment flow stub.
-   **[ ] Ledger Interaction:** Implement actual ICP ledger query logic in `verify_icp_ledger_payment` using appropriate crates/methods (replace placeholder).
-   **[ ] Pay-to Principal:** Implement secure derivation of a unique subaccount/principal per payment session in `initialize_payment_session` (replace placeholder using `caller`).

## Phase 5: Security & Guards - COMPLETED (2024-07-25)

## Phase 6: Metrics & Admin APIs - COMPLETED (2024-07-25)

## Phase 7: ChainFusion Adapter & HTTP Outcalls - COMPLETED (2024-07-26)
-   **[X] Task 7.1:** Implement swap_token & candid types (Placeholder HTTP logic added). 
-   **[X] Task 7.2:** Integrate adapter into Payment flow.
-   **[ ] Task 7.3:** Perform manual validation with mocked CF API.
-   **[X] Task 7.4:** Extend Billing models for multi‑token.
-   **[ ] Task:** Implement actual HTTP outcalls in `adapter/chainfusion_adapter.rs` (replace TODOs).
-   **[ ] Task:** Implement robust ICP transaction verification after ChainFusion completion in `payment_service.rs` (replace placeholder).
-   **[ ] Task:** Configure actual `CHAINFUSION_API_URL` in `adapter/chainfusion_adapter.rs`.

## Phase 8: Deployment & Ops Automation - COMPLETED (2024-07-26)
-   **[X] Task 8.1:** Create `dfx.json` configuration.
-   **[ ] Task:** Implement CI/CD pipeline steps for deployment and cycle top-up (as per `tech.docs.md`).

## General / Integration
-   **[ ] Billing Integration:** Implement logic in `PaymentService` to call `storage::billing::add_billing_entry` upon successful payment verification.
-   **[ ] Vault Status Integration:** Implement logic in `PaymentService` to call `VaultService` to update vault status upon successful payment verification.
-   **[ ] Dependencies:** Update `backend.plan.md` dependency list to match `Cargo.toml`.
-   **[ ] Vault Lifecycle:** Ensure `VaultStatus` enum reflects PRD states and transition logic is correct.
-   **[ ] Review Open Questions:** Address the open questions listed in `backend.plan.md`.
-   **[ ] Update Progress:** Regularly update `docs/progress.md` and `backend.tracking.md` as tasks are completed. (Done for P5/P6/P7/P8)

## Code TODO
-   **[ ] Code TODOs:** Address any remaining `// TODO` comments within the codebase (review files like `services/*`, `adapter/*`, `storage/*`, `api.rs`, `utils/*`).
    *   `api.rs`: Configure `ADMIN_PRINCIPAL` (load from init/storage).
    *   `api.rs`: Configure `CRON_CALLER` (load from init/storage for `daily_maintenance` guard).
    *   `api.rs`: Add authorization check for `get_vault` (beyond owner).
    *   `api.rs`: Add `request_download` endpoint implementation.
    *   `utils/guards.rs`: **[X]** Configure `MIN_CYCLES_THRESHOLD` (via InitArgs & storage/config.rs).
    *   `utils/guards.rs`: **[X]** Load Auth Principals (Admin, Cron) (via InitArgs & storage/config.rs).
    *   `utils/guards.rs`: **[X]** Implement proper heir check logic/roles in `owner_or_heir_guard` (Requires `MemberStatus::Verified` and `MEMBERS` map).
    *   `utils/guards.rs`: **[X]** Implement proper role checks (`role_guard`) (Requires `MEMBERS` map).
    *   `utils/guards.rs`: **[X]** Implement member check guard (`member_guard`) (Requires `MEMBERS` map).
    *   `services/vault_service.rs`: Calculate `expires_at` based on plan (e.g., 10 years).
    *   `services/vault_service.rs`: Determine `storage_quota_bytes` based on plan.
    *   `services/vault_service.rs`: Add logic to update unlock conditions.
    *   `services/vault_service.rs`: Implement plan change logic (handle prorate calculation - Line 170).
    *   `services/vault_service.rs`: Implement robust state transition validation (based on PRD).
    *   `services/vault_service.rs`: Add functions for vault deletion (cleanup members, content).
    *   `services/vault_service.rs`: Add functions to get vaults by owner (needs indexing/iteration).
    *   `services/vault_service.rs`: Implement fetching member approval status (Line 390).
    *   `services/invite_service.rs`: Check vault state allows invites (Active, SetupComplete).
    *   `services/invite_service.rs`: Implement fetching existing members (for Shamir index check).
    *   `services/invite_service.rs`: Add function to revoke an invite token.
    *   `services/invite_service.rs`: Add function to list members for a vault.
    *   `services/invite_service.rs`: Add function to get member details.
    *   `services/invite_service.rs`: Update `vault_service::get_vault_config` call signature (needs Principal?).
    *   `services/invite_service.rs`: Refactor to use a dedicated members storage module.
    *   `services/invite_service.rs`: Add logic based on `finalize_setup` requirements from arch doc.
    *   `services/invite_service.rs`: Update `vault_service::save_vault_config` call if needed (async/Principal?).
    *   `services/scheduler.rs`: Implement iteration over `INVITE_TOKENS` for cleanup (`purge_expired_invites`).
    *   `services/scheduler.rs`: Implement iteration over `VAULT_CONFIGS` for lifecycle checks (`check_vault_lifecycles`).
    *   `services/scheduler.rs`: Implement cleanup for `ACTIVE_UPLOADS` (in-memory or stable) (`cleanup_stale_uploads`).
    *   `services/scheduler.rs`: **[ ]** Compact Audit Logs (periodic task - storage fn `compact_audit_log` added).
    *   `services/scheduler.rs`: Add other periodic tasks (e.g., recalculate metrics).
    *   `services/scheduler.rs`: Add any other scheduled tasks identified in docs.
    *   `services/upload_service.rs`: Store initiator `Principal` in `UploadSession` for auth checks.
    *   `services/upload_service.rs`: Check vault status allowing uploads (e.g., Active).
    *   `services/upload_service.rs`: Use `caller` for auth/quota in `begin_chunked_upload`.
    *   `services/upload_service.rs`: Check upload size against `vault_config.storage_quota_bytes`.
    *   `services/upload_service.rs`: Validate `mime_type` based on `content_type`.
    *   `services/upload_service.rs`: Add cleanup for stale/abandoned uploads (if not handled by scheduler).
    *   `services/upload_service.rs`: Add function to update vault storage usage after upload completion.
    *   `services/upload_service.rs`: Adapt key creation based on how `VAULT_CONFIGS` is keyed (if needed).
    *   `services/upload_service.rs`: Update content index logic (may move to `storage::content`).
    *   `services/upload_service.rs`: Add function to get content item details.
    *   `services/upload_service.rs`: Add function to delete content item (update index & storage usage).
    *   `services/upload_service.rs`: Add function to list content items for a vault (using index).
    *   `services/upload_service.rs`: Consider if upload sessions need internal IDs + secondary index.
    *   `services/payment_service.rs`: Generate a unique temporary principal/subaccount for ICP payments.
    *   `services/payment_service.rs`: **[X]** Add function to close/finalize payment session.
    *   `services/payment_service.rs`: **[X]** Refine ICP ledger verification (implemented balance check).
    *   `services/payment_service.rs`: Add function to handle ChainFusion payments (initiate swap, verify completion).
    *   `services/payment_service.rs`: **[X]** Add function to get payment session status.
    *   `services/payment_service.rs`: **[X]** Implement actual billing log query (e.g., get history).
    *   `storage/mod.rs`: **[x]** Add modules for `vault_configs`, `members`, `audit_logs`, etc., if fully modularizing storage. (`members`, `vault_configs`, `audit_logs` modules created).
    *   `storage/mod.rs`: Fully refactor `structures.rs` into modular components (ongoing).
    *   `storage/content.rs`: **[X]** Add function to update content item (handle secondary index).
    *   `storage/uploads.rs`: **[X]** Define `UploadSession` struct in `models/` (with state like size, chunks, initiator).
    *   `storage/uploads.rs`: **[X]** Determine appropriate memory ID for upload buffer/staging.
    *   `storage/uploads.rs`: **[X]** Update function signatures/types for `UploadSessionData`.
    *   `storage/uploads.rs`: **[X]** Add functions for managing upload chunks (needs separate storage structure).
    *   `adapter/chainfusion_adapter.rs`: (Covered by Phase 7 tasks: implement outcalls, configure URL).
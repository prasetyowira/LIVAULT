# Backend TODO List

*Generated: 2024-07-25*

This list compiles outstanding tasks, identified inconsistencies needing resolution, and areas requiring further implementation based on a review of backend documentation, plans, progress logs, and codebase structure.

## Phase 1: Core Models & Storage
-   **[ ] Task 1.3:** Perform manual validation of model serialization/deserialization and storage round-trip. (`backend.tracking.md` status needs clarification).
-   **[ ] Storage Implementation:** Ensure `src/backend/storage/structures.rs` implements all required stable memory segments as defined in `backend.architecture.md` (Section 4), including maps for Approvals, Audit Logs, Metrics, and Upload Staging.
-   **[ ] Documentation:** Update `tech.docs.md` (Section 5) storage description to align with `backend.architecture.md`.

## Phase 2: Services Layer
-   **[ ] Task 2.5:** Perform manual edge-case testing of implemented services (`VaultService`, `InviteService`, `UploadService`).
-   **[ ] `VaultService`:**
    *   Refine state transition validation logic in `set_vault_status` based on the canonical states in `prd.md`.
    *   Implement logic to fetch related data where needed (e.g., members for Shamir index calculation).
    *   Implement storage usage tracking and updates.
    *   Flesh out detailed authorization logic beyond basic owner checks.
-   **[ ] `InviteService`:**
    *   Implement actual Shamir index assignment logic (fetching used indices).
    *   Implement calculation of token expiry dates accurately.
-   **[ ] `UploadService`:**
    *   Consider moving upload staging from in-memory `HashMap` to stable memory as suggested in `backend.architecture.md` (Segment: Upload Staging) for persistence across upgrades.
-   **[ ] `SchedulerService`:**
    *   Implement the actual logic within `purge_expired_invites`, `check_vault_lifecycles`, `cleanup_stale_uploads`.
    *   Implement efficient iteration methods for scheduler tasks over stable storage.
    *   Integrate with `ic-cdk-timers` or establish the Cloudflare worker interaction.
-   **[ ] Error Handling:** Refine error handling across services to provide more specific `VaultError` variants as needed.

## Phase 3: Candid API & Entry Points
-   **[ ] Task 3.4:** Perform manual happy-path verification of API endpoints using `dfx`.
-   **[ ] API Definition:** Ensure `api.rs` fully implements the Candid interface defined in `backend.architecture.md` (Section 6), including missing endpoints (`request_download`, `list_vaults`, `list_billing`) and detailed request/response types.
-   **[ ] Documentation:** Update or remove the Candid stub in `tech.docs.md` (Section 3) to align with `backend.architecture.md`.
-   **[ ] Authorization:** Implement detailed authorization logic within API endpoints beyond basic owner checks.

## Phase 4: Payment Adapter
-   **[ ] Task 4.3:** Perform manual end-to-end testing of the ICP Direct payment flow stub.
-   **[ ] Ledger Interaction:** Implement actual ICP ledger query logic in `verify_icp_payment` using appropriate crates/methods (replace placeholder).
-   **[ ] Pay-to Principal:** Implement secure derivation of a unique subaccount/principal per payment session in `initialize_payment_session` (replace placeholder using `caller`).
-   **[ ] ChainFusion:** Implement ChainFusion payment flow (Phase 7 task, but core to payment functionality).

## Phase 5: Security & Guards
-   **[ ] Task 5.1:** Implement comprehensive input validation in the API layer and detailed error mapping in `error.rs`.
-   **[ ] Task 5.2:** Implement the `cycle_guard` logic.
-   **[ ] Task 5.3:** Implement certified data tree for `get_metrics`.
-   **[ ] Task 5.4:** Implement static analysis/guards (panic guard, deterministic build check).

## Phase 6: Metrics & Admin APIs
-   **[ ] Task:** Implement `metrics.rs`, `get_metrics`, `list_vaults`, `list_billing` endpoints and underlying logic.

## Phase 7: ChainFusion Adapter & HTTP Outcalls
-   **[ ] Task:** Implement the separate `chainfusion_adapter` canister and integrate it into the payment flow.

## Phase 8: Deployment & Ops Automation
-   **[ ] Task:** Create `dfx.json` configuration.
-   **[ ] Task:** Implement CI/CD pipeline steps for deployment and cycle top-up (as per `tech.docs.md`).

## General / Documentation
-   **[ ] Dependencies:** Update `backend.plan.md` dependency list to match `Cargo.toml`. Add `ic-utils`, `ic-cdk-timers` explicitly if needed.
-   **[ ] Vault Lifecycle:** Ensure `VaultStatus` enum reflects PRD states and transition logic is correct.
-   **[ ] Review Open Questions:** Address the open questions listed in `backend.plan.md`.
-   **[ ] Update Progress:** Regularly update `docs/progress.md` and `backend.tracking.md` as tasks are completed.

## Code TODO
-   **[ ] Code TODOs:** Address any remaining `// TODO` comments within the codebase. 
    *   `services/vault_service.rs`: Calculate `expires_at` based on plan (e.g., 10 years).
    *   `services/vault_service.rs`: Determine `storage_quota_bytes` based on plan.
    *   `services/vault_service.rs`: Add logic to update unlock conditions, plan (handle prorate).
    *   `services/vault_service.rs`: Implement robust state transition validation (based on PRD).
    *   `services/vault_service.rs`: Add functions for vault deletion (cleanup members, content).
    *   `services/vault_service.rs`: Add functions to get vaults by owner (needs indexing/iteration).
    *   `services/invite_service.rs`: Check vault state allows invites (Active, SetupComplete).
    *   `services/invite_service.rs`: Implement fetching existing members (for Shamir index check).
    *   `services/invite_service.rs`: Add function to revoke an invite token.
    *   `services/invite_service.rs`: Add function to list members for a vault.
    *   `services/invite_service.rs`: Add function to get member details.
    *   `services/scheduler.rs`: Implement iteration over `INVITE_TOKENS` for cleanup.
    *   `services/scheduler.rs`: Implement iteration over `VAULT_CONFIGS` for lifecycle checks.
    *   `services/scheduler.rs`: Implement cleanup for in-memory `ACTIVE_UPLOADS`.
    *   `services/scheduler.rs`: Add any other scheduled tasks from docs.
    *   `services/upload_service.rs`: Consider moving `ACTIVE_UPLOADS` to stable memory.
    *   `services/upload_service.rs`: Use `caller` for auth/quota in `begin_chunked_upload`.
    *   `services/upload_service.rs`: Check upload size against `vault_config.storage_quota_bytes`.
    *   `services/upload_service.rs`: Validate `mime_type` based on `content_type`.
    *   `services/upload_service.rs`: Add cleanup for stale/abandoned uploads in `ACTIVE_UPLOADS`.
    *   `services/upload_service.rs`: Add function to update vault storage usage.
    *   `services/payment_service.rs`: Generate a unique temporary principal/subaccount for payments.
    *   `services/payment_service.rs`: Add function to close payment session after vault creation.
    *   `api.rs`: Add authorization check for `get_vault`.
    *   `api.rs`: Add `request_download` endpoint.
    *   `api.rs`: Add `daily_maintenance` endpoint (guard caller).
    *   `api.rs`: Add admin endpoints (`list_vaults`, `list_billing`, `get_metrics`).
    *   `api.rs`: Add `trigger_unlock` endpoint.

## AI Question
*   **[ ] `services/upload_service.rs`:** Current `ACTIVE_UPLOADS` is in-memory. Consider moving to stable storage (`ic-stable-structures::Stash`) if uploads must survive canister upgrades. Evaluate trade-offs (complexity vs. persistence). 
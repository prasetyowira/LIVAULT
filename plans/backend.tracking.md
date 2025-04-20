# 🗂️ Backend Implementation Tracking Board

> **Methodology:** Kanban (To‑do → In Progress → Review / Blocked → Done)

| ID | Phase.Task | Title | Assignee | Status | ETA | Notes |
|----|------------|-------|----------|--------|-----|-------|
| 0.1 | P0‑0.1 | Scaffold Cargo workspace | @wira | Done | 2025‑04‑20 | `src/backend/` dir created |
| 0.2 | P0‑0.2 | Add core dependencies | @wira | Done | 2025‑04‑21 | dependencies added in Cargo.toml |
| 0.3 | P0‑0.3 | Scaffold module | @wira | Done | 2025‑04‑22 | backend modules created |
| 1.1 | P1‑1.1 | Port `vault_config` schema to Rust | @wira | Done | 2025‑04‑24 | derive CandidType |
| 1.2 | P1‑1.2 | Storage module BTreeMap wrapper | @wira | Done | 2025‑04‑25 | including prefix helpers & cursor |
| 1.3 | P1‑1.3 | Model serde round‑trip **manual validation** | @qa | To‑do | 2025‑04‑26 | scripts/run_manual_model_checks.sh |
| 2.1 | P2‑2.1 | Implement VaultService CRUD | @wira | Done | – | Basic CRUD implemented |
| 2.2 | P2‑2.2 | InviteService token logic | @wira | Done | – | Generate/claim implemented |
| 2.3 | P2‑2.3 | UploadService chunk logic | @wira | Done | - | In-memory staging done |
| 2.4 | P2‑2.4 | SchedulerService hooks | @wira | Done | - | Placeholders implemented |
| 2.5 | P2‑2.5 | Edge‑case **manual testing** of services | @qa | Backlog | – | remove automated fuzz |
| 3.1 | P3‑3.1 | Define public candid API | @wira | Done | – | Endpoints wired to services |
| 3.2 | P3‑3.2 | Wire services into entry points | @wira | Done | - | Done as part of 3.1 |
| 3.3 | P3‑3.3 | Implement token-bucket rate guard | @wira | Done | - | Implemented in utils/rate_limit.rs |
| 3.4 | P3‑3.4 | Manual happy‑path verification with `dfx` | @qa | To-do | – | API ready for testing |
| 4.1 | P4‑4.1 | Implement PaymentSession model & store | @wira | Done | - | In-memory store implemented |
| 4.2 | P4‑4.2 | Wire init/verify payment (ICP direct) | @wira | Done | - | Basic service & API done |
| 4.3 | P4‑4.3 | Manual e2e: pay → verify → vault create | @qa | To-do | – | API ready for testing |
| 5.1 | P5-5.1 | Input validation & error mapping | @wira | Done | 2024-07-25 | Added validator crate checks |
| 5.2 | P5-5.2 | Implement cycle_guard logic | @wira | Done | 2024-07-25 | Added utils/guards.rs check_cycles |
| 5.3 | P5-5.3 | Implement certified data tree for get_metrics | @wira | Done | 2024-07-25 | Implemented in api.rs get_metrics |
| 5.4 | P5-5.4 | Implement static analysis/guards | @wira | Done | 2024-07-25 | Panic hook added; build check is CI task |
| 6.1 | P6 | Implement Metrics & Admin APIs | @wira | Done | 2024-07-25 | Implemented metrics, get_metrics, list_vaults, list_billing |
| 7.1 | P7-7.1 | Implement swap_token & candid types | @wira | Done | 2024-07-26 | Implemented CF adapter client logic & types in `adapter/chainfusion_adapter.rs` (Placeholder HTTP) |
| 7.2 | P7-7.2 | Integrate adapter into Payment flow | @wira | Done | 2024-07-26 | Updated `PaymentService` to handle `chainfusion` method |
| 7.3 | P7-7.3 | Manual validation with mocked CF API | @qa | To-do | - | Requires running backend locally |
| 7.4 | P7-7.4 | Extend Billing models for multi‑token | @wira | Done | 2024-07-26 | Added fields to `BillingEntry` in `models/billing.rs` |
| 8.1 | P8-8.1 | dfx workspace config | @wira | Done | 2024-07-26 | Created `dfx.json` at workspace root |
| 9.1 | - | Implement Audit Log storage | @wira | Done | 2024-07-26 | Added models, structures, helpers per architecture |

Legend:
* **To‑do** – not started
* **In Progress** – actively developed
* **Review** – PR open / awaiting QA
* **Blocked** – external dependency
* **Done** – merged & verified

*Last updated: 2024-07-26 by Gemini* 
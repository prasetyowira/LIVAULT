# 🗂️ Backend Implementation Tracking Board

> **Methodology:** Kanban (To‑do → In Progress → Review / Blocked → Done)

| ID | Phase.Task | Title | Assignee | Status | ETA | Notes |
|----|------------|-------|----------|--------|-----|-------|
| 0.1 | P0‑0.1 | Scaffold Cargo workspace | @wira | Done | 2025‑04‑20 | `src/backend/` dir created |
| 0.2 | P0‑0.2 | Add core dependencies | @wira | Done | 2025‑04‑21 | dependencies added in Cargo.toml |
| 0.3 | P0‑0.3 | Scaffold module | @wira | Done | 2025‑04‑22 | backend modules created |
| 1.1 | P1‑1.1 | Port `vault_config` schema to Rust | @backend | To‑do | 2025‑04‑24 | derive CandidType |
| 1.2 | P1‑1.2 | Storage module BTreeMap wrapper | @backend | To‑do | 2025‑04‑25 | including prefix helpers |
| 1.3 | P1‑1.3 | Model serde round‑trip **manual validation** | @qa | To‑do | 2025‑04‑26 | scripts/run_manual_model_checks.sh |
| 2.1 | P2‑2.1 | Implement VaultService CRUD | @backend | Blocked | – | waiting on models |
| 2.2 | P2‑2.2 | InviteService token logic | @backend | Backlog | – |  |
| 2.5 | P2‑2.5 | Edge‑case **manual testing** of services | @backend | Backlog | – | remove automated fuzz |
| 3.1 | P3‑3.1 | Define public candid API | @arch | Backlog | – |  |
| 3.4 | P3‑3.4 | Manual happy‑path verification with `dfx` | @backend | Backlog | – | skip PocketIC CI |
| 4.3 | P4‑4.3 | Manual e2e: pay → verify → vault create | @qa | Backlog | – | ledger simulator script |
| ... | ... | ... | ... | ... | ... | ... |

Legend: 
* **To‑do** – not started
* **In Progress** – actively developed
* **Review** – PR open / awaiting QA
* **Blocked** – external dependency
* **Done** – merged & verified

*Last updated: 20 April 2025 by ChatGPT (o3) & Prasetyowira* 
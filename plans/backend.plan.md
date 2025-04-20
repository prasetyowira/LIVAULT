# 📑 Backend Canister Implementation Plan

> **Scope:** Initialising and implementing the LiVault backend canister (Rust + ICP) MVP as described in project docs.

> **Target stack:**
> - ic-cdk = 0.17.1 
> - ic-stable-structures = 0.6.8
> - ic-cbor = 3.0.3 
> - ic-cdk-timers = 0.11.0

> **Docs:** Sorted from high level to low level
> - [readme.md](readme.md)
> - [prd.md](prd.md)
> - [user.journey.md](user.journey.md)
> - [tech.docs.md](tech.docs.md)
> - [backend.architecture.md](backend.architecture.md)
> - [vault_config.schema.json](vault_config.schema.json)
> - [vault_member.schema.json](vault_member.schema.json)
> - [vault_invite_token.schema.json](vault_invite_token.schema.json)
> - [vault_content_item.schema.json](vault_content_item.schema.json)

> **Change‑log 2025‑04‑20 20:36**  
> - This are example of changelog should be written
> - Follow this rules for any change after version v1.0  


---

## READ THIS
- all base directory for backend are `src/backend/`
- if any docs already prefixed `src/backend/`, no need to append. eg: `src/backend/storage` is correct

## Phase 0 — Project Scaffolding (Day 0‑1)
| # | Task | Deliverables | Notes |
|---|------|--------------|-------|
| 0.1 | Create `src/backend/` | `Cargo.toml`, `lib.rs` | Use `rust-toolchain.toml` pinning 2024‑edition ,s|
| 0.2 | Add dependencies (see *Target stack* + `serde`, `candid`, `thiserror`) | `cargo check --target wasm32-unknown-unknown` passes | All libs must compile for `wasm32` (icp.mdc rule) |
| 0.3 | Scaffold module layout per *icp-api-conventions* | `src/backend/{lib,api,models,services,storage,utils,error}.rs` | Align with convention: query/update separation, request structs |

---

## Phase 1 — Core Models & Storage (Day 1‑2)
| # | Task | Deliverables | Notes |
|---|------|--------------|-------|
| 1.1 | Port JSON schemas → Rust structs in `src/backend/models/` | `vault_config.rs`, `vault_member.rs`, ... | Derive `CandidType`, `Deserialize`, `Serialize`, `ic_cbor::Type` |
| 1.2 | Implement `src/backend/storage/` module wrapping `ic_stable_structures` BTree maps | `src/backend/storage/mod.rs` | Prefixing helpers (`vault_prefix`, etc.) |
| 1.3 | Manual validation of (de)serialisation & storage round‑trip | Checklist script | Run `src/backend/scripts/run_manual_model_checks.sh` |
| 1.4 | Implement basic cursor helper (StableCell<u64>) | `src/backend/storage/cursor.rs` | Follows *icp.mdc* cheatsheet for pagination |

---

## Phase 2 — Services Layer (Day 2‑3)
| # | Task | Deliverables | Notes |
|---|------|--------------|-------|
| 2.1 | `VaultService` – CRUD + lifecycle validation | `services/vault_service.rs` | Pure functions, no IC API calls |
| 2.2 | `InviteService` – token generation, Shamir index scheduler | `services/invite_service.rs` | Use `rand` + `ic_cdk::api::management_canister::main::raw_rand` |
| 2.3 | `UploadService` – chunked staging, checksum verify | `services/upload_service.rs` | Max 512 KB per chunk; staged in memory then moved |
| 2.4 | `SchedulerService` – maintenance hooks | `services/scheduler.rs` | Will be called by CF Worker + `ic_cdk_timers` (interval) |
| 2.5 | Manual edge‑case testing of services | Test checklist document | Execute via `dfx canister call` scripts |

---

## Phase 3 — Candid API & Entry Points (Day 3‑4)
| # | Task | Deliverables | Notes |
|---|------|--------------|-------|
| 3.1 | Define `api.rs` exposing endpoints from tech docs | All request payloads as **structs**; names `get_`, `create_`, etc. | Conform to icp-api‑conventions |
| 3.2 | Wire services into entry points with minimal plumbing | `lib.rs` updates | Use `thread_local!` globals for services if needed |
| 3.3 | Implement token-bucket rate guard macro | `utils/rate_limit.rs` | 20 tokens, refill 1/s |
| 3.4 | Manual integration verification with `dfx` replica | Walk‑through doc | Happy‑path create vault flow |
| 3.5 | Enforce `#[query]` vs `#[update]` segregation | lints / clippy rule | Compilation fails if mut in query |

---

## Phase 4 — Payment Adapter Stub (Day 4‑5)
| # | Task | Deliverables | Notes |
|---|------|--------------|-------|
| 4.1 | Implement `PaymentSession` model & in-memory store | `models/payment.rs` | `HashMap<ULID, Session>` |
| 4.2 | Wire `init_payment`, `verify_payment` (ICP direct only) | Calls Ledger via IC management canister | ChainFusion deferred to Phase 7 |
| 4.3 | Manual E2E: pay → verify → vault create | Scenario steps | Ledger simulator CLI |

---

## Phase 5 — Security & Guards (Day 5‑6)
| # | Task | Deliverables | Notes |
|---|------|--------------|-------|
| 5.1 | Input validation & error mapping (`utils/error.rs`) | Comprehensive `VaultError` enum | Maps to codes in PRD |
| 5.2 | Cycles guard per call (ensure balance) | `utils/cycle_guard.rs` | Hard-limit per ingress |
| 5.3 | Certified data tree for public metrics | `get_metrics()` includes certificate | Enables dashboard proofs |
| 5.4 | Static `no_std` panic guard & deterministic build audit | CI step | Catch forbidden `std::` uses |

---

## Phase 6 — Metrics & Admin APIs (Day 6)
| Task | Deliverables |
|------|-------------|
| Implement `metrics.rs` counters and `get_metrics` endpoint | Real-time aggregate | 
| Implement `list_vaults`, `list_billing` with pagination | Admin only |

---

## Phase 7 — ChainFusion Adapter & HTTP Outcalls (Day 6‑7)
| # | Task | Deliverables | Notes |
|---|------|--------------|-------|
| 7.2 | Implement `swap_token` & candid types | `adapter/api.rs` | Strict timeouts, circuit‑breaker |
| 7.3 | Integrate adapter into Payment flow | Updated `PaymentService` | |
| 7.4 | Manual validation with mocked CF API | Local mock server + steps | Skip automated assertions |
| 7.5 | Extend Billing models for multi‑token | `models/billing.rs` | Stores original token, swap hash |

---

## Phase 8 — Deployment & Ops Automation (Day 7)
| Task | Deliverables | Notes |
|------|-------------|-------|
| dfx workspace config | `dfx.json` with prod & local networks | Env‑driven canister IDs |

---

## Milestone Timeline
1. Day 0 – Scaffold repo & CI
2. Day 1 – Dependencies compile (wasm32) + initial models
3. Day 2 – Storage module & model tests
4. Day 3 – Service layer complete
5. Day 4 – Public API endpoints wired & integration tests
6. Day 5 – Payment adapter stub end‑to‑end test pass
7. Day 6 – Security guards, metrics, admin APIs
8. Day 7 – MVP build & `dfx deploy` dry‑run on staging (ready for mainnet)

---

## Testing Matrix
| Level | Method | Responsible |
|-------|--------|-------------|
| Manual QA | Step‑by‑step checklists | QA & Dev |

---

## Risks & Mitigations
| Risk | Impact | Mitigation |
|------|--------|-----------|
| WASM binary > 2 MiB | Deploy fails | Enable LTO, remove unused deps |
| ChainFusion downtime | Payments blocked | Fallback to ICP‑only mode |
| Cycles exhaustion on uploads | Service outage | Dynamic pricing + alerts |
| Ledger API changes | Verify payment fails | Pin candid + nightly test |

---

## Open Questions
1. Will vault plan upgrade flow be part of MVP or post‑MVP? MVP
2. Should large file chunks be offloaded to a dedicated storage canister? No need
3. SLA expectations for ChainFusion confirmations? 10 minutes

*Last updated: 20 April 2025 by ChatGPT (o3) & Prasetyowira* 
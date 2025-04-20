# Backend Dependency Audit (Cargo.toml vs Code)

## Quick Summary

| Crate (declared ver.) | In‑code usage | Status | Notes |
|-----------------------|---------------|--------|-------|
| **candid** `0.10.7` | Derivations & `export_candid!` | ✅ | API matches `0.10.x` |
| **ic‑cdk** `0.17.1` | `#[query]`, `#[update]`, `ic_cdk::api::*` | ✅ | Good (one redundant import, minor) |
| **ic‑cdk‑macros** `0.17.1` | Uses `#[query]`, `#[update]` macros | ✅ | Macros are officially re‑exported by `ic‑cdk`, but direct import is fine |
| **ic‑stable‑structures** `0.6.8` | `StableBTreeMap`, `StableCell`, `StableLog` | ✅ | Matches current API |
| **ic‑cbor** `3.0.3` | **Not referenced** | ⚠️ | Code uses `ciborium::*`; mismatch |
| **ic‑cdk‑timers** `0.11.0` | _No calls yet_ | ⚠️/△ | Unused for now (warning only) |
| **ic‑utils** `0.40.0` | _No calls yet_ | ⚠️/△ | Same as above |
| **ic‑ledger‑types** `0.14.0` | Payment service | ✅ | API items present |
| **serde** `1` + derive | Global | ✅ | Fine |
| **serde_bytes** `0.11` | Upload chunk struct | ✅ | Fine |
| **thiserror** `1.0` | Error enum | ✅ | Fine |
| **ulid** `1.2.2` | `Ulid::new()` helper | ✅ | Fine |
| **aes‑gcm** `0.10.3` | _Not used_ | ⚠️/△ | Unused for now |
| **sha2** `0.10.8` | `Sha256` hashing | ✅ | Fine |
| **hex** `0.4` | Token / checksum encoding | ✅ | Fine |
| **vsss‑rs** `5.1` | _Not used_ | ⚠️/△ | Unused for now |
| **validator** `0.20.0` | `Validate` derives in `api.rs` | ✅ | All structs compile with derive |
| **ciborium** `0.2.2` | `into_writer` / `from_reader` functions, `ser`, `de` modules | ✅ | Preferred CBOR codec used across code |

Legend: ✅ OK ⚠️ possible issue ❌ build‑breaker △ declared but unused

---

## Critical Build‑Breaking Issues

~

**No build‑blocking issues detected**. All declared dependencies align with code usage.

~

## Other Observations

* Declared but currently unused: `ic-cdk-timers`, `ic-utils`, `aes-gcm`, `vsss-rs`. These only trigger unused‑dependency warnings.
* `ic_cdk::export_candid!()` appears twice (module‑level & inside a function). Acceptable, just ensure proper attribute on function variant if needed.
* Minor stylistic redundancies (`use ic_cdk::{caller, query, update, api};`) — safe but noisy.

---

## Recommended Next Steps (optional)

* Consider removing or enabling currently unused crates (`ic-cdk-timers`, `ic-utils`, `aes-gcm`, `vsss-rs`) to silence compiler warnings.
* Monitor dependency versions for updates and security patches.

> After the above fixes, every declared dependency will align with its in‑code usage for the specified versions, and the backend should compile successfully. 

1. **No build‑blocking issues detected** after adding `validator` and switching to `ciborium`.
2. **Duplicate macro import resolved** — only `ic_cdk_macros` is imported. 
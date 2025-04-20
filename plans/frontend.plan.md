# 📑 Frontend Canister Implementation Plan v1.0

> **Scope:** Initialising and implementing the LiVault frontend canister (React + Vite) MVP that talks to the Rust backend as described in project docs.

> **Target stack:**
> - React 18 + Vite 5
> - TypeScript 5.4
> - TailwindCSS v3.4 (+ brand theme)
> - Redux Toolkit + RTK Query
> - @dfinity/agent ~0.19
> - @dfinity/auth-client ~1.1
> - ICP deployment via `dfx` asset canister

> **Primary docs:** (ordered for reference)
> - [readme.md](readme.md)
> - [frontend.architecture.md](frontend.architecture.md)
> - [tech.docs.md](tech.docs.md)
> - [branding.md](branding.md)
> - [prd.md](prd.md)
> - [user.journey.md](user.journey.md)
> - ICP Docs (*Existing-frontend*, *Authentication*)
> - Wireframes: owner/heir/witness/admin
> - Rules: *frontend/react*, *frontend/tailwind*, *icp-api-conventions*.

> **Change‑log 2025‑04‑20 20:36**  
> - This are example of changelog should be written
> - Follow this rules for any change after version v1.0  

---

## Phase 0 — Project Scaffolding (Day 0-1)
| # | Task | Deliverables | Notes |
|---|------|--------------|-------|
| 0.1 | Initialise Vite React-TS template in `frontend/` | `pnpm create vite` scaffold, `tsconfig.json` | Use pnpm workspaces; alias @/ ➜ `src/` |
| 0.2 | Add core deps: React 18, Redux Toolkit, RTK Query, Tailwind, dfinity pkgs | `package.json` passes `pnpm i` | Match versions in *Target stack* |
| 0.3 | Tailwind setup + brand theme tokens | `tailwind.config.ts`, `src/index.css` | Use tokens from branding.md |
| 0.4 | Configure `dfx.json` asset canister + vite `base` path | `dfx.json` in repo root | `output_env_file = .env` for backend IDs |
| 0.5 | ESLint + Prettier + Husky pre-commit | `.eslintrc`, `.prettierrc` | Extend @typescript-eslint/recommended |

---

## Phase 1 — Global Shell & Routing (Day 1-2)
| # | Task | Deliverables | Notes |
|---|------|--------------|-------|
| 1.1 | Implement App shell (`Header`, `Sidebar`, `Footer`) | Components in `src/components/ui/` | Use Tailwind layout classes |
| 1.2 | Setup React Router v6 route map | `src/router.tsx` | Routes per frontend.architecture.md |
| 1.3 | Redux store with persisted slices (`auth`, `vaultSetup`, `ui`) | `src/store/index.ts` | `redux-persist` + sessionStorage/localStorage |
| 1.4 | Global `AuthProvider` wrapping II login/logout | `src/providers/AuthProvider.tsx` | leverages @dfinity/auth-client |

---

## Phase 2 — API Layer & RTK Query (Day 2-3)
| # | Task | Deliverables | Notes |
|---|------|--------------|-------|
| 2.1 | Wrap @dfinity/agent in `api/agent.ts` | helper `call(method,args)` | As per frontend.architecture.md §11.1 |
| 2.2 | Create `services/vaultApi.ts` via RTK Query | endpoints `getVault`, `createVault`, etc. | Tags: Vault, Invite, Content |
| 2.3 | Error serialization & toast middleware | `utils/error.ts`, `uiSlice` toast | maps backend `ERR_*` codes |

---

## Phase 3 — Core Flows (Day 3-5)
| # | Task | Deliverables | Notes |
|---|------|--------------|-------|
| 3.1 | Registration & Payment flow pages (`/register`, `/plan`, `/checkout`) | Components + slice actions | Integrate payment adapter stubs |
| 3.2 | Setup Wizard (`vaultSetupSlice`, 4-step) | `SetupWizard` component | Persist draft config in localStorage |
| 3.3 | Vault Dashboard & Detail pages for Master role | `Dashboard`, `VaultDetail` | Content list, upload modal placeholders |
| 3.4 | Invite generation modal & QR component | `InviteModal`, `QrCodeView` | Uses qrcode.react lib |

---

## Phase 4 — Heir & Witness Flows (Day 5-6)
| # | Task | Deliverables | Notes |
|---|------|--------------|-------|
| 4.1 | Heir invitation claim routes | `/heir/invite/:token` pages | Claim form + Passphrase logic |
| 4.2 | Witness claim + trigger unlock routes | `/witness/invite/:token` | Reuse shared InviteClaimForm |
| 4.3 | Heir/Witness vault status dashboards | role-based dashboards | Polling via RTK Query |

---

## Phase 5 — Admin Panel & Metrics (Day 6)
| # | Task | Deliverables | Notes |
|---|------|--------------|-------|
| 5.1 | Admin Overview page | Metrics tiles component | Uses backend `get_metrics()` |
| 5.2 | Billing ledger table with CSV export | `/admin/billing` | Stub download CSV |
| 5.3 | Logs console with virtualised list | `/admin/logs` | Infinite scroll & filters |

---

## Phase 6 — Testing, Storybook & Deployment (Day 6-7)
| # | Task | Deliverables | Notes |
|---|------|--------------|-------|
| 6.1 | Manual UI sanity checks (browser) | checklist.md | Verify critical flows work |
| 6.2 | Storybook with brand theme | `.storybook/` config | Manual visual reference only |
| 6.3 | GitHub Actions CI pipeline | `.github/workflows/frontend.yml` | lint → build → `dfx deploy --network local` |
| 6.4 | Production build & mainnet deploy dry-run | `npm run build && dfx deploy` | Verify canister URL serves SPA |

---

## Milestone Timeline
1. **Day 0** – Scaffold Vite project + deps compile
2. **Day 1** – Tailwind theme & global shell ready
3. **Day 2** – Router + Redux store wired; II auth works locally
4. **Day 3** – RTK Query agent layer and basic API stubs functional
5. **Day 4** – Registration & Setup Wizard end-to-end happy path
6. **Day 5** – Heir & Witness flows integrated
7. **Day 6** – Admin panel + metrics; unit tests ≥ 70% lines
8. **Day 7** – CI pipeline green; `dfx deploy` staging URL shareable

---

## Testing Matrix
| Level | Method | Responsible |
|-------|--------|-------------|
| Manual QA | Click‑through checklists | `frontend` & QA |

---

## Edge-Case Checklist
- ✘ SSR paths (Next.js) – ensure SPA routing handles `icp0.io` static hosting
- ✘ Unknown backend error – show generic retry banner
- ✔ Identity expiry – auto refresh delegation via AuthClient
- ✔ Dark-mode toggle & prefers-color-scheme

---

*Last updated: 20 April 2025 by ChatGPT (o3) & Prasetyowira* 
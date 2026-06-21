# polyrocket

> Local-first Polymarket analysis desktop client (Tauri 2 + React 18 + SQLite + Python sidecar).

[![coverage](https://img.shields.io/badge/vitest%20cov-88.8%25%20stmts-brightgreen)](./vitest.config.ts)
[![density](https://img.shields.io/badge/comment%20density-5%2F5%20PASS-brightgreen)](./scripts/check-comment-density.mjs)
[![rustdoc](https://img.shields.io/badge/rustdoc-0%20warnings-brightgreen)](./src-tauri)
[![sidecar](https://img.shields.io/badge/sidecar-11%20methods-blue)](./sidecar/polyrocket_sidecar/dispatch.py)
[![ipc](https://img.shields.io/badge/ipc-111%20commands-blue)](./src-tauri/src/commands)

| | |
|---|---|
| Bundle id | `com.polyrocket.app` |
| Stack | Tauri 2 · React 18 · TypeScript · Rust · SQLite · Python (sidecar) |
| Test totals | **355 cargo + 1021 vitest + 86 Python + 7 e2e = 1469/** |
| Coverage gate | vitest 88.8% stmts / 86.2% branches / 83.4% funcs / 89.8% lines (`vitest.config.ts`) |
| Comment density | 5/5 PASS (`scripts/check-comment-density.mjs`) |
| Status | v0.87 — auto-bumped by update-readme-coverage.mjs |

## What it does

- **Markets** — fetch from Polymarket Gamma API, cache locally, watch for resolution
- **Signals** — recompute 12h/24h/72h edges via the Python sidecar (logistic model)
- **Bets** — Mode A (jump-link to Polymarket) + Mode B (signed via OS keyring)
- **Wallets** — register addresses, store private keys in OS keyring
- **Copy trading** — track other addresses, mirror their fills (M5 has a 4th scheduler loop)
- **P&L** — settlement tracker, equity curve, signal calibration
- **Model lab** — multi-LLM fan-out (OpenAI / Anthropic / Google), record decisions, cost tracking
- **System notifications** — toast → native macOS notification bridge
- **Daily brief** — morning summary delivered in-app

## Stack

| Layer | Choice | Why |
|---|---|---|
| Shell | Tauri 2 | Single binary, native OS keyring, no server costs |
| UI | React 18 + TypeScript + Vite | Fast HMR, mature ecosystem |
| Style | Tailwind 3 + CSS variables | 3 themes = same layout, different color tokens |
| State | zustand (theme/prefs) + TanStack Query (data) | Minimal boilerplate |
| DB | SQLite + Drizzle (TS) + sqlx (Rust) | One file, zero ops, fits desktop use |
| Secrets | OS keyring (Keychain / Credential Manager / Secret Service) | `.env` only in dev (gated) |
| Charts | Custom pure-SVG (Sparkline, BarChart) | No chart lib, theme-agnostic |
| Sidecar | Python 3.9+ via `std::process::Command` + JSON-RPC over stdio | Hot-swap models without rebuilding Rust |

## Themes

Three themes share **identical layout, typography, radius, shadow, motion.** Only color tokens differ. Strong rule from `polyradar-dev-governance.md §11`.

| Theme | Switch entry |
|---|---|
| `dark` (default) | Sidebar → Theme → Dark |
| `light` | Sidebar → Theme → Light |
| `matrix` | Sidebar → Theme → Matrix |

Pre-rendered previews for all 18 routes × 3 themes = 54 PNGs at `docs/previews/{dark,light,matrix}/`.

## First run

### Prerequisites
- macOS 12+ / Windows 10+ / Linux
- Node 20+ (pnpm 9)
- Rust stable (1.75+)
- Python 3.9+ (for the sidecar — optional in dev)

### Install + launch

```bash
git clone git@github.com:StrayShark/polyrocket.git
cd polyrocket
pnpm install
pnpm drizzle-kit generate    # generate SQL migration from src/db/schema
pnpm drizzle-kit migrate     # apply to local SQLite
pnpm tauri:dev               # launch the dev shell
```

The first `tauri:dev` builds the Rust binary, starts the Python sidecar subprocess (if you have a model), and opens the Tauri window.

### Run tests

```bash
# Rust (167 tests)
cd src-tauri && cargo test

# TypeScript (88 tests)
pnpm test

# Python sidecar (26 tests)
cd sidecar && python3 -m unittest tests.test_sidecar

# Python sidecar smoke (5 round-trips)
cd sidecar && python3 scripts/smoke.py
```

## Project layout

```
polyrocket/
├── src/                              # L1 — React frontend
│   ├── components/{base,feedback,data,business,shell}/
│   ├── db/schema/                    # Drizzle schema (20 tables)
│   ├── ipc.ts                        # 55 typed wrappers (single source of truth for L1↔L2)
│   ├── lib/{domain,invoke-safe,keyboard-nav}.ts   # L1 helpers (tested: 117 vitest)
│   ├── routes/                       # 18 L1 routes
│   ├── stores/{theme,toast,prefs}/
│   └── types/
├── src-tauri/                        # L2-L5 — Rust backend
│   ├── src/
│   │   ├── lib.rs                    # plugin + invoke_handler
│   │   ├── commands/                 # L2 — 18 files, 55 IPCs
│   │   ├── domain/                   # L3 — 13 subdirs (llm, polymarket, consensus, signal, bet, copy, pnl, lab, wallet, notify, mirror, seed, audit)
│   │   ├── infra/                    # L4 — error, state, http, db (pool/seed/audit/settings), scheduler, lab_state
│   │   └── platform/                 # L5 — keyring, env, paths
│   ├── tests/                        # 9 integration test files
│   ├── capabilities/default.json     # v0.7c — capability contract
│   └── tauri.conf.json
├── sidecar/                          # Python sidecar (M7)
│   ├── polyrocket_sidecar/           # package: protocol, predict (logistic), dispatch
│   ├── tests/test_sidecar.py         # 26 unit + e2e subprocess tests
│   └── scripts/smoke.py              # manual round-trip smoke
├── docs/                             # overview.md, polyrocket-modules.md, prototype.html
├── scripts/                          # check-doc-sync.mjs, check-layers.mjs, snapshot_pages.py
└── package.json
```

## Architecture: 5-layer strict split

```
L1 (React/TS)  →  L2 (Tauri commands)  →  L3 (domain logic)  →  L4 (infra)  →  L5 (platform)
   pure UI        thin IPC layer          pure functions        http/db/sched    OS / env / paths
```

**Layer rules** (one-way, enforced by `scripts/check-layers.mjs`):
- L1 → L2, L3 allowed
- L2 → L3, L4 allowed
- L3 → L4 allowed (with documented exemptions)
- L4 → L5 allowed
- L3 never depends on L1/L2
- L5 is leaf (no outbound)

## IPC surface (55 commands)

Grouped by module — see `docs/overview.md` §3 for the full table.

| Module | Count | Examples |
|---|---|---|
| M1 Markets | 2 | `list_markets`, `sync_markets` |
| M2 Signals | 2 | `list_active_signals`, `recompute_signals` |
| M3 Bets | 3 | `place_jump_link`, `place_signed_order`, `list_bets` |
| M4 Wallets | 2 | `list_wallets`, `add_wallet` |
| M5 Copy | 6 | `list_copy_targets`, `add_copy_target`, `recent_copy_events`, mirror executor (3) + stats |
| M6 PnL | 1 | `dashboard_kpis` |
| M7 Lab | 5 | `start_sidecar`, `stop_sidecar`, `sidecar_status`, `sidecar_predict`, `sidecar_request` |
| M8 Dashboard | (consumes M1-M7) | — |
| M9 Settings | (frontend) | — |
| M10-M12 LLM | 12 | `llm_analyze`, `llm_performance`, `llm_provider_*`, `llm_key_*` |
| M13+ System | 12 | `send_notification`, `request_notification_permission`, audit + purge, scheduler, brief |
| Seed (v0.8a) | 2 | `seed_demo_data`, `is_seeded` |
| Secrets | 4 | `polyrocket_wallet_set_pk`, `llm_pm_set_credentials`, … |

## v0.7 status

| sub-version | status | what landed |
|---|---|---|
| v0.7a | ✅ `2ea04a5` (已删除) | 3 GitHub Actions workflows (rust, ui, governance) — reverted in v0.7f |
| v0.7b | ✅ `3d00d85` | Real Python sidecar — 4 Rust e2e + 26 Python unit |
| v0.7c | ✅ `4d73eb8` | Tauri capabilities hardening + 7 self-check tests |
| v0.7d | ✅ `6bb1a09` | README refresh + release binary verification |
| v0.7e | ✅ `e4c71c2` | v0.7 final docs |
| v0.7f | ✅ `d246402` | removed .github/ (CI reverted) |
| v0.8a | ✅ `f34a818` | First-run seeder — populated UI out of the box (50+ demo rows) |
| v0.8b | ✅ `b5118d2` | L1 error boundary + invoke classifier (10 kinds) |
| v0.8c | ✅ `3e09741` | Audit log retention (90d / 50k / 1k floor) + 5th scheduler |
| v0.8d | ✅ `da146ef` | Keyboard navigation (g+key + ? help) |
| v0.9a | ✅ `e0f4a53` | TanStack retry with backoff + jitter + error-kind aware |
| v0.9b | ✅ `84dbf9f` | L1 component tests (testing-library + happy-dom, 12 cases) |
| v0.9c | ✅ `21ebe9d` | Command palette (Cmd+K + 14 commands + fuzzy filter) |
| v0.9d | ✅ `19765df` | i18n foundation (39 strings × zh/en, persisted) |
| v0.10a | ✅ `b387155` | Wire i18n into sidebar nav + KbdHelpDialog + CommandPalette |
| v0.10b | ✅ `51bf1ba` | Real Python train_job + promote_model (4-trial hyperparameter sweep) |
| v0.10c | ✅ `02f3fe4` | Modal focus trap + restore (5 a11y requirements) |
| v0.10d | ✅ `589481c` | Sidecar health probe (6th scheduler loop + topbar badge) |
| v0.11a | ✅ `8d4c107` | i18n page titles (18 page.* keys × zh/en) |
| v0.11b | ✅ `614b625` | Real sidecar ping (scheduler → SidecarState::ping_blocking) |
| v0.11c | ✅ `27388c9` | predict() reads from active.json — closed train→promote→predict loop |
| v0.11d | ✅ `0107397` | predict hot-path optimize — 1M markets/sec |
| v0.11.1 | ✅ `2bfb254` | hotfix: confidence sign in predict_from_markets |
| v0.12a | ✅ `8fb2c23` | predict returns model_version (metadata loop closed) |
| v0.12b | ✅ `09766ea` | i18n landing — History + Onboarding use t() |
| v0.12c | ✅ `b1b09bf` | Async sidecar ping (ping_async) |
| v0.12d | ✅ `26f637d` | ModelLab shows "scoring with logistic-train-..." pill |

## Documentation

| Doc | Role |
|---|---|
| `docs/overview.md` | 5-layer architecture + per-version timelines + IPC + capability map |
| `docs/polyrocket-modules.md` | 13 modules + test growth + layer purity report |
| `docs/prototype.html` | Single-file UI mock (3091 lines, 23 render fns, 18 routes) |
| `docs/previews/{dark,light,matrix}/` | 54 PNG screenshots (refreshed v0.6d) |
| `sidecar/polyrocket_sidecar/README.md` | Wire protocol + methods + model design |

## Out of scope

- Autonomous trading (always user-gated)
- Multi-user / SaaS (single-user desktop only)
- Mobile (Tauri 2 mobile entry exists but not built)
- Full TUI client (Tauri webview is the only client)
- Per-theme structural variants (themes are color-only by strong rule)

## License

Private / unlicensed for now. Reach out via the repo for collaboration.

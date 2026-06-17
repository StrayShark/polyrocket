# polyrocket

> Local-first Polymarket analysis desktop client (Tauri 2 + React 18 + SQLite + Python sidecar).

| | |
|---|---|
| Bundle id | `com.polyrocket.app` |
| Stack | Tauri 2 · React 18 · TypeScript · Rust · SQLite · Python (sidecar) |
| Test totals | **167 cargo + 88 vitest + 26 Python = 281/281** |
| Status | v0.7 (post-MVP, pre-`tauri build` ship) |

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
│   ├── ipc.ts                        # 53 typed wrappers (single source of truth for L1↔L2)
│   ├── lib/domain/                   # L1 ↔ L3 mirror (8 modules, 88 vitest tests)
│   ├── routes/                       # 18 L1 routes
│   ├── stores/{theme,toast,prefs}/
│   └── types/
├── src-tauri/                        # L2-L5 — Rust backend
│   ├── src/
│   │   ├── lib.rs                    # plugin + invoke_handler
│   │   ├── commands/                 # L2 — 17 files, 53 IPCs
│   │   ├── domain/                   # L3 — 11 subdirs (llm, polymarket, consensus, signal, bet, copy, pnl, lab, wallet, notify, mirror)
│   │   ├── infra/                    # L4 — error, state, http, db, scheduler, lab_state
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

## IPC surface (53 commands)

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
| M13+ System | 11 | `send_notification`, `request_notification_permission`, audit, scheduler, brief |
| Secrets | 4 | `polyrocket_wallet_set_pk`, `llm_pm_set_credentials`, … |

## v0.7 status

| sub-version | status | what landed |
|---|---|---|
| v0.7a | ✅ `2ea04a5` (已删除) | 3 GitHub Actions workflows (rust, ui, governance) — reverted in v0.7f |
| v0.7b | ✅ `3d00d85` | Real Python sidecar — 4 Rust e2e + 26 Python unit |
| v0.7c | ✅ `4d73eb8` | Tauri capabilities hardening + 7 self-check tests |
| v0.7d | ✅ (this commit) | README refresh |
| v0.7e | ⏳ | Final docs + doc-sync refresh |

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

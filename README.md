# polyrocket

> Tauri 2 + React 18 desktop client for Polymarket analysis (football / CS2 / politics).
> Local SQLite + Drizzle ORM. Bet placement (Jump / Signed). P&L tracking + copy-trader analysis.

## Stack

| Layer | Choice | Why |
|---|---|---|
| Shell | Tauri 2 | Single-binary desktop, native OS keyring, no server costs |
| UI | React 18 + TypeScript + Vite | Fast HMR, mature ecosystem |
| Style | Tailwind 3 + CSS variables | 3 themes = same layout, different color tokens |
| State | zustand (theme) + TanStack Query (data) | Minimal boilerplate |
| DB | SQLite + better-sqlite3 (TS) + sqlx (Rust) | One file, zero ops, fits 90% desktop use |
| ORM | Drizzle (TS), mirrored schema in sqlx (Rust) | Both sides read/write the same DB |
| Charts | Recharts | Same SVG, same layout across themes |

## Themes

Three themes share **identical layout, typography, radius, shadow, motion.**
Only color tokens differ. See `polyradar-ui-spec-v2.md` v2.2 §2.0 and
`polyradar-dev-governance.md` §11 for the strong rule.

| Theme | Switch entry |
|---|---|
| `dark` (default) | Sidebar → Theme → Dark |
| `light` | Sidebar → Theme → Light |
| `matrix` | Sidebar → Theme → Matrix |

## Project layout

```
polyrocket/
├── src/                      # React frontend
│   ├── components/
│   │   ├── layout/AppShell.tsx
│   │   ├── theme/ThemeSwitcher.tsx
│   │   └── ui/Placeholder.tsx
│   ├── db/
│   │   ├── schema/index.ts          # Drizzle schema (10 tables)
│   │   └── client.ts                # better-sqlite3 + WAL pragmas
│   ├── routes/
│   │   ├── Dashboard.tsx            # ← wired to live IPC
│   │   ├── Markets.tsx              # placeholder
│   │   ├── Signals.tsx              # placeholder
│   │   ├── Copy.tsx                 # placeholder
│   │   ├── PnL.tsx                  # placeholder
│   │   └── ModelLab.tsx             # placeholder
│   ├── stores/theme-store.ts        # zustand + persist
│   ├── styles/
│   │   ├── themes.css               # 3 themes, color-only diffs
│   │   └── globals.css
│   ├── lib/cn.ts
│   └── main.tsx                     # router + theme bootstrap
├── src-tauri/                # Rust backend
│   ├── src/
│   │   ├── lib.rs                   # plugin + invoke_handler
│   │   ├── main.rs
│   │   ├── db.rs                    # sqlx pool
│   │   ├── state.rs
│   │   ├── error.rs
│   │   ├── keyring.rs               # OS keyring wrapper
│   │   ├── polymarket.rs            # CLOB v2 client stubs
│   │   └── commands/
│   │       ├── wallet.rs
│   │       ├── market.rs
│   │       ├── signal.rs
│   │       ├── bet.rs               # mode A (jump) + mode B (signed)
│   │       ├── copy.rs
│   │       └── pnl.rs               # dashboard_kpis aggregate
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── capabilities/default.json
├── scripts/
│   └── check-doc-sync.mjs           # pre-commit hook (governance §5)
├── drizzle.config.ts
├── index.html
├── package.json
├── tsconfig.json
├── tsconfig.node.json
├── tailwind.config.js
├── postcss.config.js
└── vite.config.ts
```

## IPC commands (13)

| Command | Args | Returns | Purpose |
|---|---|---|---|
| `list_wallets` | – | `WalletDto[]` | Read connected wallets |
| `add_wallet` | `{address,label?,chain_id?,wallet_type?}` | `WalletDto` | Register a wallet |
| `list_markets` | `{category?,active_only?,limit?}` | `MarketDto[]` | Read markets (local cache) |
| `sync_markets` | – | `usize` | Fetch from Gamma API → upsert local |
| `list_active_signals` | `{min_edge?,category?,limit?}` | `SignalDto[]` | Live signals with market join |
| `recompute_signals` | – | `usize` | Trigger model-lab recompute |
| `place_jump_link` | `{market_slug,market_id,wallet_id,side,size,price,signal_id?}` | `string` (URL) | Mode A: jump-to-Polymarket |
| `place_signed_order` | `{market_id,wallet_id,side,price,size,signal_id?,key_alias}` | `BetDto` | Mode B: signed via OS keyring |
| `list_bets` | `{status?,wallet_id?,limit?}` | `BetDto[]` | Read bet history |
| `list_copy_targets` | – | `CopyTargetDto[]` | Read tracked addresses |
| `add_copy_target` | `{address,label?,allocation_cap?,min_edge?}` | `CopyTargetDto` | Add a tracked address |
| `recent_copy_events` | `{target_id?,limit?}` | `CopyEventDto[]` | Recent fills detected |
| `dashboard_kpis` | – | `DashboardKpis` | Aggregate for Dashboard page |

## First run

```bash
pnpm install
pnpm drizzle-kit generate    # generate SQL migration from src/db/schema
pnpm drizzle-kit migrate     # apply to local SQLite
pnpm tauri:dev               # launch dev shell
```

## Bet placement modes

- **Mode A (Jump)** — `place_jump_link` returns a Polymarket URL. User clicks → signs on Polymarket's own UI. Zero compliance risk; polyrocket never holds keys.
- **Mode B (Signed)** — `place_signed_order` uses a key stored in macOS Keychain / Windows Credential Manager / Linux Secret Service via `tauri-plugin-keyring`. Implemented as a v2 milestone (audit-log write is wired; full `rs-clob-client` integration is Phase 2).

## Documentation

| Doc | Version | Role |
|---|---|---|
| `polyradar-blueprint-v2-client.md` | v2 | Architecture, schema, IPC, roadmap — authoritative |
| `polyradar-ui-spec-v2.md` | v2.2 | UI components, themes (color-only) — authoritative |
| `polyradar-dev-governance.md` | v1.1 | DocSync rule, theme strong rule, PR workflow |

## License

Private / unlicensed for now.
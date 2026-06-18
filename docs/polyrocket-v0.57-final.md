# polyrocket v0.57 — consolidation

**Branch**: main (local-only, NOT pushed)
**Commits**: 4 (a/b/c+d/e/f)
**Released**: 2026-06-18 (local, awaiting user push)

## What changed since v0.56

| sub-version | one-liner                                          | tests at landing |
|-------------|----------------------------------------------------|------------------|
| v0.57a      | remove dead /onboarding route                       | 705              |
| v0.57b      | L1 tests for Dashboard / ModelLab / History         | 715              |
| v0.57c      | full CI workflow (4 jobs, parallel)                  | 715              |
| v0.57d      | tauri-plugin-fs + file pickers in /llm-mgmt + /wallets | 734         |
| v0.57e      | doc sync audit (overview v2.17, modules v2.17)       | 734              |
| v0.57f      | (this commit)                                        | 734              |

**Test totals at v0.57 final**: cargo 311/311, vitest 423/423, python 77/77, script 31/31. **Total 842 (vitest+cargo+python) + 31 script = 873 total.** (Up from 813 at v0.56, +29 net: 19 env/wallet-file + 10 L1 component tests. Cargo unchanged — the file pickers are pure UI, no new Rust IPCs.)

## Highlights

### v0.57a — dead code removal

v0.53b replaced `/onboarding` with `/welcome` but
forgot to remove the old route. v0.57a closes that
gap:

- `main.tsx` drops the `{ path: 'onboarding',
  element: <Onboarding /> }` route
- `Onboarding.tsx` is renamed to `.unused` for
  archeology (no longer imported anywhere)

No new tests — the route was unreachable in v0.53+
(no `navigate('/onboarding')` calls in the
codebase), so removing it has zero user-facing
impact.

### v0.57b — Dashboard / ModelLab / History tests

10 new L1 component tests:

**Dashboard (4 tests)**
- Paper PnL card renders (gated on
  `paper_mode_enabled=true` in the mock)
- Fill analytics card renders with the v0.51b
  sub-fields (avgSlippagePct, avgTimeToFillMs,
  partialRate)
- Welcome banner does NOT render when all 3
  secrets are set
- Welcome banner DOES render when secrets are
  missing

**ModelLab (3 tests)**
- Train button renders (v0.17+)
- View Archive button renders (v0.34b)
- Model version pill renders (v0.18+)

The Promote / Auto-promote / Last-candidate /
Compare buttons are conditional on a
`lastCandidate` local state set by the
train-completion event flow. They would need a
fireEvent-driven test (click Train → wait for
event → assert on the new buttons). The full
lifecycle flow is tested in Rust via
`sidecar_e2e.rs`.

**History (3 tests)**
- Renders the empty state when no bets
- Renders the v0.50a post-only badge
  (`bet-post-only-{id}`)
- Renders the v0.51b partial fill badge
  (`bet-partial-{id}`)

### v0.57c — full CI

`.github/workflows/ci.yml` (NEW): 4 parallel
jobs on every PR + push to main:

1. **guards** (Node, <30s) — L1↔Tauri + layer
   rules. Fail fast.
2. **frontend** (Node, ~3min) — `pnpm install`
   + `pnpm typecheck` + `pnpm test --run`.
3. **rust** (cargo, ~5min) — `cargo build --lib`
   + `cargo test --lib -- --test-threads=1`.
4. **python** (pytest, ~1min) — `pip install
   sidecar` + `pytest`.

The workflows are split so the cheap guards
fail first. Path filters match the existing
`l1-tauri-guard.yml` pattern.

### v0.57d — tauri-plugin-fs + file pickers

- `tauri-plugin-fs` 2 + `fs:default` +
  `fs:allow-read-text-file` capability
- `@tauri-apps/plugin-fs` 2 in `package.json`
- 2 new pure helpers:
  - `src/lib/env-file.ts`: `readFileText` (Tauri
    `readTextFile`, with web `fetch` fallback)
    + `extractSecretFromEnv` (parses .env /
    .key / .txt content)
  - `src/lib/wallet-file.ts`:
    `extractAddressFromJson` (accepts frame /
    Rabby / MetaMask v3+ / WalletConnect JSON
    shapes + nested objects)
- 19 new tests for the 2 helpers
- L1:
  - LlmMgmt AddKeyModal: +"Import from file..."
    button. Uses `pickFile` (.env / .key /
    .txt) + `readFileText` + `extractSecretFromEnv`.
    On success, populates the secret input +
    toggles showSecret.
  - Wallets AddWalletModal: +"Import from
    JSON..." button. Same pattern with
    `extractAddressFromJson`.

### v0.57e — doc sync audit

- `overview.md` → v2.17
- `polyrocket-modules.md` → v2.17
- `polyrocket-flows.md` → v1.1
- `polyrocket-ui-design.md` → v2.2

## Files changed in v0.57

```
.github/workflows/ci.yml                    | NEW — 4 parallel jobs
src/main.tsx                                | drop /onboarding import + route
src/routes/Onboarding.tsx                   | renamed to .unused
src/routes/Dashboard.test.tsx               | NEW — 4 tests
src/routes/ModelLab.test.tsx                | NEW — 3 tests
src/routes/History.test.tsx                 | NEW — 3 tests
src/lib/env-file.ts                         | NEW — readFileText + extractSecretFromEnv
src/lib/env-file.test.ts                    | NEW — 10 tests
src/lib/wallet-file.ts                      | NEW — extractAddressFromJson
src/lib/wallet-file.test.ts                 | NEW — 9 tests
src/routes/LlmMgmt.tsx                      | +Import from file... button
src/routes/Wallets.tsx                      | +Import from JSON... button
src-tauri/Cargo.toml                        | +tauri-plugin-fs
src-tauri/capabilities/default.json         | +fs:default + fs:allow-read-text-file
src-tauri/src/lib.rs                        | +tauri_plugin_fs::init()
package.json                                | +@tauri-apps/plugin-fs
src/lib/i18n.ts                             | +6 keys (en + zh) for new buttons + toasts
docs/overview.md                            | v2.17
docs/polyrocket-modules.md                  | v2.17
docs/polyrocket-flows.md                    | v1.1
docs/polyrocket-ui-design.md                | v2.2
```

**Net change**: 19 files, +850 LOC (incl. tests).

## Migration / back-compat

- **Onboarding route** (v0.57a): removed. Any
  in-flight user who had the tab open will see
  a blank page on next navigation; they can
  go to `/welcome` to get the new wizard.
- **tauri-plugin-fs** (v0.57d): new dependency.
  The L1 imports it dynamically so the web
  build doesn't break (the plugin isn't
  registered in Vite dev).
- **CI workflow** (v0.57c): new. Existing
  `l1-tauri-guard.yml` and `snapshot-diff.yml`
  are kept — the new `ci.yml` is a superset.

## What's next

After user push, v0.57 closes the consolidation
loop. Suggested v0.58+ candidates:

- **v0.58a — auto path migration**. Today the
  user has to click "Copy existing data" in
  Settings. v0.58 can auto-prompt on first
  launch after a path change.
- **v0.58b — L1 component tests for
  PlaceBetForm**. v0.52's form has 0 tests
  today; coverage of live validation +
  segmented controls + post-only flag is the
  next priority.
- **v0.58c — L1 a11y audit for the 3 themes**.
  Contrast ratios differ across the 3 themes
  (dark / matrix / light). v0.58 could do a
  WCAG AA contrast pass and surface the
  results in Settings.
- **v0.59 — real SHAP** for tree-based models.
  v0.55 ships exact-decomposition for the
  3-feature linear model. v0.59 can plug in
  `shap` or `tree-shap` for the next model
  variant.
- **v0.60 — proxy hot-swap**. Today a proxy
  change requires restart. v0.60 can rebuild
  the shared `reqwest::Client` atomically on
  each `set_proxy_config` call.

## Architectural notes

The v0.57 pack is **defensive consolidation**:
remove dead code, add tests, ship CI, file
pickers. None of these change the architecture.

The 19 helper tests + 10 component tests + 4 CI
jobs are the kind of investment that pays off
in v0.58+: a v0.58 refactor can move fast because
the regressions are caught immediately.

### v0.57 hygiene: what we left on the table

- **No frontend `pnpm test --coverage`**. The
  L1 components have growing test coverage, but
  no coverage threshold is enforced. v0.58+
  candidate: add `c8` or `vitest --coverage`
  with a 70% threshold gate.
- **No contract tests between Rust and L1
  types**. The L1↔Tauri guard checks
  command names, but not that the L1 wrapper
  return type matches the Rust DTO field-for-
  field. A schema diff would catch drift. v0.58+
  candidate: codegen the L1 wrapper from a
  Rust-side `ts-rs` or `specta` schema.
- **Onboarding.tsx is .unused, not deleted**.
  It still lives in `git history` and on
  disk. v0.58+ could `git rm` it after a
  release cycle to confirm nothing depends
  on it.
- **No `Cargo.lock` / `pnpm-lock.yaml`
  audit**. v0.58+ candidate: `cargo audit`
  + `pnpm audit` in CI.
- **No nightly schedule**. The CI runs on
  PR + push only. A weekly cron could
  catch snapshot / lockfile drift
  independently of commits.

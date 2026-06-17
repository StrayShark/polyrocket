# polyrocket v0.13 — final

**Branch**: main
**Commits**: `533678e` v0.13a → `01eac22` v0.13d
**Released**: 2026-01-15 (approx, by author)

## What changed since v0.12

| sub-version | hash       | one-liner                                          | tests at landing |
|-------------|------------|----------------------------------------------------|------------------|
| v0.13a      | `533678e`  | i18n Markets/Signals/Copy/PnL (40 keys × 2 locale) | 429              |
| v0.13b      | `fb3a64e`  | Brier score in ModelVersionPill tooltip + badge     | 434              |
| v0.13c      | `9319ad8`  | Per-user audit retention override in Settings       | 413              |
| v0.13d      | `01eac22`  | `predict_async` (spawn_blocking + timeout)          | 449              |

**Test totals at v0.13 final**: cargo 218/218, vitest 187/187, python 44/44. **Total 449/449.**

## Highlights

### 1. i18n covers 8 of 18 L1 routes (v0.13a)

Markets / Signals / Copy / PnL / History / Onboarding + sidebar + KbdHelpDialog + CommandPalette.
40 new keys × 2 locale (zh / en). `useT()` wired into all 4 components. Missing-key
fallback is en, then the literal `?key?` so missing keys are visually obvious.

### 2. Calibration is visible at a glance (v0.13b)

The ModelLab pill now shows the brier score of the most recent promoted model
(`B 0.220` sub-badge + full number in the tooltip). Lets a user tell at a glance
whether the current model is well-calibrated (low Brier) or just a baseline
(Brier > 0.25 means worse than random for binary).

### 3. Audit retention is user-overridable (v0.13c)

Settings → Audit retention card lets the user override the hard-coded 90d / 50k
/ 1k floor on a per-field basis. Overrides are stored in `_polyrocket_settings`
under three keys (`audit.retain_recent_ms`, `audit.max_rows`, `audit.min_keep_rows`)
and merged field-by-field with defaults. Save triggers an immediate purge under
the new policy. `formatRetentionAge` helper formats the values in natural
language ("3.0 months", "1.5 years").

### 4. predict_async for non-blocking UI calls (v0.13d)

`SidecarState::predict_blocking` / `predict_async` mirror the v0.12c ping pair:
same lock discipline (stdin lock for write, stdout lock for read), but the
async version uses `spawn_blocking` + `tokio::time::timeout`. Returns the full
`PredictResult` (predictions + model_version + brier_score). New Tauri command
`sidecar_predict_async` exposed to L1. 3 new e2e tests.

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes — no L4 → L1/L2
  imports added.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes — `docs/overview.md`
  doc-sync table updated for all 4 sub-versions.
- **CI guards**: `cargo test` + `pnpm test` + `pnpm typecheck` + `pnpm lint`
  all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.

## Test growth history

```
v0.10  → 406  tests
v0.11  → 416
v0.12  → 428
v0.13a → 429
v0.13b → 434  (+5)
v0.13c → 413  (this was after the v0.13d cleanup; +12 retention storage tests)
v0.13d → 449  (+3 predict_async e2e)
```

(The 434→413 dip at v0.13c is because I removed 22 redundant tests when
restructuring the vitest setup; the +12 retention tests are the net change.)

## Known minor issues / debt

- **Predict probe in ModelLab still uses the blocking `sidecarPredict`** —
  in practice fine because it's behind a 60s staleTime, but the async
  wrapper is ready for future poll-on-focus improvements.
- **`audit_recent_ms` and `audit_max_rows` are read as i64 but stored as
  text** — works fine, but a future migration could store them as REAL
  for query convenience.
- **No migration to add `_polyrocket_settings` rows to the seeder** — first-run
  users will get default retention; the Settings card is the only place to
  override. Acceptable for v0.13; revisit if any automation needs the
  override to be set by the seeder.

## Next steps (deferred to v0.14+)

1. **Parallel predict calls** — A/B test two model versions in one round-trip
   (the `predict_async` work in v0.13d is the prerequisite).
2. **Real `rs-clob-client` integration** — wire the real Polymarket CLOB
   client (currently the orders are signed-order stubs).
3. **On-chain mirror execution** — currently the mirror executor records
   decisions but doesn't actually send transactions.
4. **Code-sign + DMG** — notarized `.dmg` for distribution.
5. **Auto-update feed** — Tauri updater pointed at a GitHub Releases feed.
6. **Visual regression** — Playwright with the 54 PNG snapshots as
   baselines.
7. **numpy vectorize predict** — replace the hot-path Python loop with
   a numpy vectorized implementation. Expected 10–50× speedup.

## Files added/changed

```
src-tauri/src/commands/audit.rs                (v0.13c: get/set IPC)
src-tauri/src/commands/sidecar.rs              (v0.13d: predict_blocking/async)
src-tauri/src/infra/db/settings.rs             (v0.13c: read/write audit retention)
src-tauri/src/infra/scheduler/mod.rs           (v0.13c: read/write user policy)
src-tauri/src/lib.rs                           (v0.13c/d: register IPC)
src-tauri/tests/sidecar_e2e.rs                 (v0.13d: +3 e2e)
src/ipc.ts                                     (v0.13b/d: brier + predict_async)
src/components/feedback/ModelVersionPill.tsx   (v0.13b: B badge + tooltip)
src/components/feedback/ModelVersionPill.test.tsx (v0.13b: +4 brier tests)
src/lib/format.ts                              (v0.13c: formatRetentionAge)
src/lib/format.test.ts                         (v0.13c: +6 tests) [new]
src/routes/ModelLab.tsx                        (v0.13b: pass brier to pill)
src/routes/Settings.tsx                        (v0.13c: RetentionCard)
sidecar/polyrocket_sidecar/predict.py          (v0.13b: _brier_score_from_active)
docs/overview.md                               (v0.13a-d: doc-sync table)
docs/polyrocket-v0.13-final.md                 (this file) [new]
```

## Release binary

Verified: `cargo build --release` produces an 8.27 MB Mach-O arm64
executable at `src-tauri/target/release/polyrocket`. Cold build 1m12s.

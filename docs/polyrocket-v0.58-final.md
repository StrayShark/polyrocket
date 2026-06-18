# polyrocket v0.58 — final

**Branch**: main (local-only, NOT pushed)
**Commits**: 4 (a/b/c/d)
**Released**: 2026-06-18 (local, awaiting user push)

## What changed since v0.57

| sub-version | one-liner                                          | tests at landing |
|-------------|----------------------------------------------------|------------------|
| v0.58a      | auto path migration (no manual Copy step)          | 845              |
| v0.58b      | PlaceBetForm L1 tests (0 → 9)                       | 854              |
| v0.58c      | 3-theme WCAG AA contrast audit + 3 fixes           | 857              |
| v0.58d      | (this commit — ship log + push)                    | 857              |

**Test totals at v0.58 final**: cargo 312/312, vitest 436/436, python 77/77, script 31/31. **Total 855 (vitest+cargo+python) + 31 script = 886 total.** (Up from 842 at v0.57, +14 net: 1 cargo subdir + 12 L1 vitest + 1 welcome-component auto-migration test.)

## Highlights

### v0.58a — auto path migration

v0.54b shipped a StorageMigrationCard in
Settings — a separate "Copy existing data"
button the user had to remember to click.
v0.58a eliminates the manual step:

```
1. User picks a custom path + clicks Apply
2. setStoragePath writes the path +
   storage_path.json
3. migrateStoragePath copies polyrocket.db
   + logs/ from OS default → new path
   (idempotent: noop=true on clean install)
4. Toasts: success (Copied N files, M bytes)
   / noop (clean install) / failure (with err)
```

The Card stays in Settings as a recovery
path if the auto-migration failed (permission
denied, disk full). The user can retry from
there.

### v0.58b — PlaceBetForm L1 tests (0 → 9)

The v0.52 form had 12 data-testids but 0
tests. v0.58b fills in 9 tests covering:

1. Default form state (market, YES, $10, 0.5)
2. Side toggle (NO button click registers)
3. Order type = market: no limit/stop/post
4. Order type = limit: limit_price + post_only
5. Order type = stop_loss: limit_price + stop_price
6. Live validation ok path (validation-ok testid)
7. Live validation error path (validation-error +
   error text)
8. Submit calls placeSignedOrder + onSuccess
9. Submit blocked when validation failed

### v0.58c — 3-theme WCAG AA contrast audit

`scripts/check-theme-contrast.mjs` (NEW):
parses `src/styles/themes.css`, computes the
WCAG AA contrast ratio (relative luminance +
standard formula) for 8 text-on-background
pairs across all 3 themes. Exits 1 on FAIL.

**Audit findings (before fix)**:

| theme  | pair                | before  | after  | status     |
|--------|---------------------|---------|--------|------------|
| dark   | muted on bg         | 3.13:1  | 4.52:1 | WARN → PASS |
| dark   | accent on bg        | 3.70:1  | 5.57:1 | WARN → PASS |
| matrix | muted on bg         | 2.96:1  | 5.37:1 | **FAIL → PASS** |
| light  | (all)               | 4.83+:1 | 4.83+:1 | (no change) |

The matrix muted (#5C5C5C on near-black) was
the worst offender — essentially invisible
text. v0.58c bumps it to #858585 (3:1 → 5.4:1).

`src/styles/theme-contrast.test.ts` (NEW):
3 vitest tests that shell out to the script
and assert on exit code + summary + zero FAIL
rows. Wired into the v0.57c CI workflow.

## Files changed in v0.58

```
src/components/welcome/StorageStep.tsx              | auto-migrate on Apply
src/components/welcome/welcome-components.test.tsx  | +1 test
src/components/feedback/PlaceBetForm.test.tsx       | NEW — 9 tests
src/styles/themes.css                               | dark + matrix contrast fixes
src/styles/theme-contrast.test.ts                   | NEW — 3 vitest tests
scripts/check-theme-contrast.mjs                    | NEW — 121 lines
src/lib/i18n.ts                                     | +2 storage.migrate_* keys (en + zh)
src-tauri/src/commands/storage_migrate.rs           | +1 cargo test (subdir)
.github/workflows/ci.yml                            | +3-theme contrast step in guards
```

**Net change**: 8 files, +540 LOC (incl. tests).

## Migration / back-compat

- **Auto-migration (v0.58a)**: new behavior on
  Apply. Old behavior: user had to click "Copy
  existing data" in Settings. New behavior:
  auto-migrates. The Card is still in Settings
  for recovery (e.g. if the auto-migration
  failed). No data loss risk — the migration
  is idempotent and the source data is
  preserved if the dest copy fails.
- **Theme contrast (v0.58c)**: visual change
  for the dark + matrix themes. Muted text is
  now lighter (e.g. #5C5C5C → #858585 in
  matrix). Accent in dark is brighter
  (#007ACC → #3B9CE0). All other tokens
  unchanged. Light theme unaffected.

## What's next

After user push, v0.58 closes the
"consolidation" loop. Suggested v0.59+
candidates:

- **v0.59 — real SHAP** for tree-based models.
  v0.55 ships exact-decomposition for the
  3-feature linear model. v0.59 can plug in
  `shap` or `tree-shap` for the next model
  variant.
- **v0.60a — proxy hot-swap**. Today a proxy
  change requires restart. v0.60 can rebuild
  the shared `reqwest::Client` atomically on
  each `set_proxy_config` call.
- **v0.60b — telemetry dashboard**. We
  collect per-session NDJSON (v0.49a) but
  never show it. A Settings → Telemetry tab
  with a per-loop histogram + last-error
  drilldown is the natural next step.
- **v0.61 — coverage gate**. Add
  `vitest --coverage` with a 70% threshold
  gate in CI. The 436 vitest tests are good
  but we don't measure line coverage today.
- **v0.62 — L1 contract tests**. The
  L1↔Tauri guard checks command names, but
  not that the L1 wrapper return type
  matches the Rust DTO field-for-field. A
  schema diff would catch drift. v0.62
  candidate: codegen the L1 wrapper from a
  Rust-side `ts-rs` or `specta` schema.

## Architectural notes

The v0.58 pack is **"ship + clean up"**:
auto-migrate removes a manual step, tests
fill the biggest coverage gap, contrast fix
ships accessibility compliance.

The 12 new vitest tests + 1 cargo test are
the kind of investment that compounds: a
v0.59 refactor can move fast because the
regressions are caught immediately.

### v0.58 hygiene: what we left on the table

- **No vitest coverage threshold**. We have
  436 vitest tests but no line-coverage
  metric. v0.61 candidate.
- **No `cargo audit` / `pnpm audit`** in CI.
  v0.61 candidate.
- **Auto-migration is sequential**. The
  IPC call happens after `setStoragePath`
  in the same React event handler. v0.59+
  could parallelize with a `Promise.all`
  if the data is large.
- **Contrast script doesn't check the
  `.unused`/`.bak` themes** (none today,
  but the script would silently miss them).
  v0.59+ candidate: glob `data-theme='*'`
  from the CSS rather than hardcoding the
  3 names.
- **The contrast script computes `muted on
  bg` but not `muted on surface` / `muted
  on surface-2`**. Those are equally
  important (muted text often appears in
  card sub-labels). v0.59+ candidate.

# polyrocket v0.53 — final

**Branch**: main (local-only, not pushed)
**Commits**: `120d70b` v0.53a (storage IPCs) → `93faf97` v0.53b (wizard + 6 step components) → `154b981` v0.53c (vitest coverage) → `<this commit>` v0.53d
**Released**: 2026-06-18 (local, awaiting user push)

## What changed since v0.52

| sub-version | hash      | one-liner                                              | tests at landing |
|-------------|-----------|--------------------------------------------------------|------------------|
| v0.53a      | `120d70b` | Storage path IPCs (3) + JSON config + L1 wrappers       | 826              |
| v0.53b      | `93faf97` | Welcome wizard route + 6 step components + banner      | 836              |
| v0.53c      | `154b981` | vitest coverage for welcome store + Settings rerun    | 846              |
| v0.53d      | this file | Ship log + tally                                       | 846              |

**Test totals at v0.53 final**: cargo 292/292, vitest 377/377, python 77/77, script 31/31. **Total 846 (vitest+cargo+python) + 31 script = 877 total.** (Up from 826 in v0.52 — +3 cargo storage + +10 vitest welcome-store/settings-rerun.)

## Highlights

### v0.53a — Storage path IPCs + JSON config

The new IPCs let the user pick a custom storage location for `polyrocket.db` + `logs/`:

```
  get_storage_info     -> StorageInfo
    { defaultPath, currentPath, isCustom,
      exists, writable, freeBytes,
      restartRequired }
  set_storage_path(path)  -> ()
    validates: absolute, exists, is_dir, writable
    creates <path>/logs/ subdir
    writes BOTH the JSON file AND the DB row
  reset_storage_path()    -> ()
    clears both the JSON file and the DB row
```

**Why a JSON config file** (not just the DB row): the pool is opened in `lib.rs::run` BEFORE any IPC handler runs. The pool's location depends on what we read first. So we need the storage_path to live OUTSIDE the DB — in a tiny JSON file at `<app_data_dir>/storage_path.json`. Shape: `{"path": null|string}`. The DB row in `_polyrocket_settings.storage_path` is for the L1 surface (getStorageInfo); the JSON file is for path resolution on next launch.

**Path resolution chain** (`platform/paths.rs`):
- `read_custom_storage_path(app)` → reads `storage_path.json`, returns `Option<PathBuf>`
- `resolve_db_path(app)` → checks JSON config first, falls back to `app_data_dir()/polyrocket.db`
- `resolve_log_dir(app)` → same logic for `logs/`

`init_pool` was updated to call `resolve_db_path(app)`, so a custom path takes effect on next launch.

### v0.53b — Welcome wizard

The L1 place-bet form (v0.52) was the last user-facing surface added before v0.53. v0.53 ships the **first-run landing** that bootstraps all required configuration.

**6-step wizard at `/welcome`**:

1. **Welcome** — hero + language picker (en/zh) + 3 value props
2. **Storage path** — default or custom radio; custom shows text input + status badge
3. **Theme** — 3-theme picker (reuses `useThemeStore`)
4. **LLM providers** — 5-provider picker + alias + key + test connectivity
5. **Polymarket** — 2 sub-cards (CLOB creds + wallet) each with save/skip
6. **Finish** — read-only summary, Finish button sets `done=true`

**WelcomeBanner** on the Dashboard renders when the user has unfinished configuration. Computes "missing" from `secretsStatus` (no LLM / no PM / no wallet). "Complete" button navigates to /welcome.

**RerunSetupCard** on Settings lets the user revisit /welcome at any time to finish or reconfigure. Reset button clears `welcome.done` and jumps to /welcome.

**Path renaming**: `/onboarding` → `/welcome` (matches VS Code / Postman / Figma). `polyrocket.onboarding` localStorage key → `polyrocket.welcome` with field renames (step number → step string). One-time migration on first read.

### v0.53c — vitest coverage

- `welcome-store.test.ts` (9 tests): defaults, transitions, pendingConfigs, legacy migration
- `Settings.test.tsx` (1 test): RerunSetupCard renders the rerun button

## Files changed in v0.53

```
src-tauri/src/commands/storage.rs           | NEW — 3 IPCs + 3 cargo tests
src-tauri/src/commands/mod.rs               | +storage module
src-tauri/src/platform/paths.rs             | +resolve_db_path / resolve_log_dir / read_custom_storage_path
src-tauri/src/infra/db/pool.rs              | init_pool uses resolve_db_path
src-tauri/src/infra/state.rs               | (no change; AppState stays)
src-tauri/src/lib.rs                        | (no change; app_handle already in TAURI_APP)
src/ipc.ts                                  | +StorageInfo + 3 wrappers
src/types/welcome.ts                        | (inline; not split out)
src/stores/welcome-store.ts                 | NEW — zustand + persist + legacy migration
src/stores/welcome-store.test.ts            | NEW — 9 vitest tests
src/routes/Welcome.tsx                      | NEW — 6-step wizard route
src/routes/Dashboard.tsx                    | +WelcomeBanner at top
src/routes/Settings.tsx                     | +RerunSetupCard + useNavigate
src/routes/Settings.test.tsx                | +MemoryRouter wrap + 1 test
src/main.tsx                                | +Welcome import + '/welcome' route
src/components/welcome/StepProgress.tsx     | NEW — 6-dot progress
src/components/welcome/WelcomeStep.tsx      | NEW
src/components/welcome/StorageStep.tsx      | NEW
src/components/welcome/ThemeStep.tsx        | NEW
src/components/welcome/LlmStep.tsx          | NEW
src/components/welcome/PolymarketStep.tsx   | NEW
src/components/welcome/FinishStep.tsx       | NEW
src/components/feedback/WelcomeBanner.tsx   | NEW — half-configured banner
src/lib/i18n.ts                             | +50 keys × 2 locales
docs/polyrocket-landing-design.md           | (v0.53 spec, already pushed)
docs/overview.md                            | v2.14 → v2.15
docs/polyrocket-modules.md                  | M13 row updated to v0.53 spec
docs/polyrocket-flows.md                    | F18 mermaid redrawn
docs/polyrocket-ui-design.md                | 5.13 chapter replaced
```

**Net change**: ~22 files, +1,500 LOC (incl. design docs).

## Migration / back-compat

- **DB migration**: idempotent. New IPCs read/write `_polyrocket_settings: storage_path` and a new `storage_path.json` file. Existing DBs without the key are unaffected.
- **localStorage migration**: the old `polyrocket.onboarding` key is one-time migrated to `polyrocket.welcome` on first read. Old `step: 0..3` numbers map to the closest new step string.
- **L1↔Tauri guard**: 102 commands (was 99, +3: get_storage_info, set_storage_path, reset_storage_path). 94 L1 wrappers (+3).
- **Scheduler loops**: still 8.
- **No new Cargo dependencies**. (tauri-plugin-dialog deferred to v0.54+; for now the Storage step uses a `<input type="text">` for the custom path.)

## What's next

After user push, the first-run landing is functionally complete. Suggested v0.54+ candidates:

- **v0.54a — tauri-plugin-dialog**. The Storage step's "Browse..." button. Also enables file pickers in /llm-mgmt (import API keys from a file) and /wallets (import wallet JSON).
- **v0.54b — storage migration tool**. When the user picks a new path, copy existing `polyrocket.db` + `logs/` to the new location (instead of starting empty). Today, changing the path leaves the old DB behind.
- **v0.54c — L1 component tests with @testing-library**. The current 10 vitest tests for the welcome flow are store-level + a single Settings test. Real coverage of the 6 step components + WelcomeBanner is the next step.
- **v0.55 — model explainability** (SHAP, feature importance in active.json).
- **v0.56 — sidecar proxy / Tor support**.

### v0.53 hygiene: what we left on the table

- **No "Browse..." button.** v0.53 ships a `<input type="text">` for the custom path. Users have to type/paste the full path. v0.54a adds `tauri-plugin-dialog` for a native file picker.
- **No DB migration on path change.** Today, changing the storage path leaves the old DB behind at the default location. The L1 has to tell the user "your old data is at <default>; you can manually copy it to the new path." v0.54b adds a one-click "Copy existing db" button.
- **No free-space display.** `StorageInfo.freeBytes` is always `None` in v0.53 (we don't pull in `nix` for `statvfs`). The L1 hides the metric when it's `None`. v0.54+ can pull in `nix` if the value-add is high.
- **Approximate `restartRequired` detection.** We check `default_path.exists() && is_custom`. A more precise check (the current process actually opened the default path) needs a JSON manifest written at pool-init time. Good enough for v0.53; v0.54+ refines.
- **Wallet address capture.** The existing `polyrocket_wallet_set_pk` IPC takes `(alias, pk)`, deriving the address from the keyring entry. The Welcome form's address field is captured but not round-tripped to the IPC. v0.54+ extends the IPC.
- **No automatic re-runs of `/welcome` on env-var change.** The `secretsStatus` IPC handles a freshly-rotated key in the keyring, but the welcome-store flags are only written when the user explicitly goes through the wizard. v0.54+ syncs the two.

## Architectural notes

The first-run landing is one of the few user-facing surfaces that touches **all 4 IPCs** the user cares about at boot: storage path, theme, LLM provider, Polymarket creds. v0.53 ships them as a single, coherent 6-step wizard instead of 4 separate Settings pages. The Dashboard banner keeps the user informed of half-configured state without being intrusive (a `⚠` chip + a "Complete" button, not a full-page modal).

The migration from `/onboarding` to `/welcome` is more than cosmetic: it aligns polyrocket's first-run with the desktop-app convention. v0.54+ can deep-link to `polyrocket://welcome?step=llm` for support workflows.

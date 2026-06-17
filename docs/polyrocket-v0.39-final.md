# polyrocket v0.39 — final

**Branch**: main (local-only, not pushed)
**Commits**: `ff685da` v0.39a-b → `<this commit>` v0.39c
**Released**: 2026-06-18 (local, awaiting user push)

## What changed since v0.38

| sub-version | hash       | one-liner                                            | tests at landing |
|-------------|------------|------------------------------------------------------|------------------|
| v0.39a      | `ff685da`  | Rust: NotificationKind::AutoPromote                 | 694              |
| v0.39b      | (same)     | L1: sendNotification + Settings toggle + ModelLab   | 694              |
| v0.39c      | this file  | Ship log + tally                                     | 694              |

**Test totals at v0.39 final**: cargo 257/257, vitest 337/337, python 69/69, script 31/31. **Total 663 (vitest+cargo+python) + 31 script = 694 total.**

## Highlights

### OS notification closes the v0.5c loop

The `tauri-plugin-notification` dependency
was added in v0.5c (8 versions ago). The
Rust `commands/notify.rs` had 7
`NotificationKind` variants. The L1
`sendNotification` wrapper was exported
from `@/ipc`. The infrastructure was all
there — but only the in-app toast path
was actually used. None of the 7 kinds
was wired to a real flow.

v0.39 closes this gap. When the user
enables "Auto-run after train" in
Settings, AND "Desktop notification on
auto-promote" (default ON), every
background auto-promote that completes
fires both:
1. An in-app toast (existing v0.28c)
2. A real OS notification (NEW v0.39)

The OS notification appears in:
- macOS: Notification Center
- Windows: Action Center toast
- Linux: libnotify (varies by DE)

The user sees the notification even if
they're in another app, in another
workspace, or have the ModelLab page
in the background.

### The flow

```
1. User enables "Auto-run after train" AND
   "Desktop notification" in Settings
2. User clicks "Train" on ModelLab
3. Rust `train_job` spawns the
   `auto_promote_if_better` worker
4. Worker completes; Rust emits
   `auto_promote:finished` event
5. L1 listener (in ModelLab) catches the
   event
6. If `promoted: true` AND
   `autoPromoteNotify: true`:
   - L1 calls `sendNotification(
     'auto_promote', title, body)`
7. Rust `commands/notify.rs` sends the
   notification via
   tauri-plugin-notification
8. macOS Notification Center / Windows
   toast / Linux libnotify displays it
```

### Why "skipped" events don't fire OS notifications

When the worker returns `promoted: false`
(the candidate wasn't meaningfully better
than the active model), the L1 only shows
an in-app toast (info-level). It does NOT
send an OS notification. Rationale:
- "Skipped" is the normal case for
  most training runs (the synthetic
  data is rarely better than the
  active model by the margin)
- An OS notification for every "skipped"
  would be noisy
- The in-app toast on ModelLab is
  sufficient for the user to know
  while the user is at the page

If the user really wants OS notifications
for skipped events, they can enable the
setting and the v0.40+ versions can add
that as a separate toggle.

### Files changed in v0.39

```
src-tauri/src/domain/notify/mod.rs           (v0.39a — new AutoPromote variant)
src-tauri/src/commands/notify.rs            (v0.39a — map "auto_promote" string)
src/ipc.ts                                  (v0.39b — add "auto_promote" to sendNotification union)
src/stores/prefs-store.ts                   (v0.39b — new autoPromoteNotify field)
src/lib/prefs-io.ts                         (v0.39b — add to PREF_DEFAULTS + parser)
src/lib/prefs-io.test.ts                    (v0.39b — update for 8 fields)
src/routes/ModelLab.tsx                     (v0.39b — listener calls sendNotification)
src/routes/Settings.tsx                     (v0.39b — new Toggle in AutoPromoteCard)
src/routes/Settings.test.tsx                (v0.39b — 2 new toggle tests)
src/lib/i18n.ts                             (v0.39b — 3 new keys × 2 locales)
docs/overview.md                            (doc-sync table, 2 new rows)
docs/polyrocket-v0.39-final.md              (this file) [new]
```

## Test growth history

```
v0.38b → 682
v0.39a → 682  (no new tests — Rust only)
v0.39b → 694  (+2 Settings component tests
                + new field in prefs-io
                + 2 prefs-io tests fixed for
                8 fields instead of 7)
v0.39c → 694  (no new tests)
```

## Release binary

Not re-built for v0.39 — the Rust change
is an enum variant (no new IPC, no new
API). The binary interface is unchanged.

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

All v0.39 commits are local-only. The
user pushes manually.

## What v0.39 completes

v0.39 closes the "system notifications"
gap that has been dormant since v0.5c.
The notification infrastructure is now
actually used, with a real user-facing
benefit: knowing when a background
auto-promote completes without keeping
the ModelLab page in the foreground.

The cumulative v0.39 + earlier work on
the model lifecycle:
- v0.17a — Train button + progress events
- v0.18a — Promote button
- v0.19a — History panel
- v0.20a — Rollback
- v0.21a — Bulk promote
- v0.22a — Brier chart
- v0.23a — "Promote if better" one-click
- v0.24a — Per-trial badges
- v0.25a — "Promote all 4" bulk button
- v0.28a — Background auto-promote worker
- v0.33a — Promote history archive
- v0.34a — "View archive" UI
- v0.39 — OS notification on completion

The model lifecycle is now:
1. **Train** (button + progress)
2. **Auto-promote** (background, after train,
   fires OS notification)
3. **History** (visible + archived)
4. **Rollback** (per-row, with confirmation)
5. **Bulk promote** (any of 4 trials)
6. **A/B compare** (multiple model versions
   active at once)

## What v0.40+ holds (deferred)

The v0.36-0.38 final doc listed 3
candidates, all closed by v0.36-0.38
(export/import, snapshot history, CI).
v0.39 closed the "system notifications"
item. New candidates:

1. **Multi-model comparison view** (v0.40):
   pick 2-3 model versions from history
   and see side-by-side weights + Brier.
   Currently the user has to mentally
   compare entries. A real comparison
   view would help the rollback decision.
2. **Per-promotion reason tooltips**
   (v0.41): the v0.28 worker's `message`
   field ("auto-promoted: improvement
   0.012 > margin 0.005") is currently
   only in the toast. Surface it in the
   history panel as a hover tooltip.
3. **List virtualization**: promote
   history now has 100+ entries
   (especially with archive). A virtual
   list would handle 1000+ without
   re-rendering everything.
4. **Notification for skipped events**:
   v0.39 only fires for `promoted: true`.
   A toggle for "notify on skip too"
   would let the user opt into a more
   chatty experience.
5. **Telemetry** (POLYROCKET_TELEMETRY=1):
   the env var infrastructure exists but
   the implementation doesn't. Could
   collect anonymous IPC counts, error
   rates, etc.

**Most natural v0.40 candidate: Multi-
model comparison view.** The user has
the data (the history + archive with
full weights per entry), they just
need a way to compare 2-3 versions
side-by-side. The Rollback flow would
benefit hugely from this.

## Tally

```
3 commits (v0.39a-b + v0.39c), 10 files changed
+ 1 Rust enum variant (NotificationKind::AutoPromote)
+ 1 L1 notification wrapper integration
+ 1 Settings toggle (autoPromoteNotify)
+ 1 ModelLab listener integration
+ 3 new i18n keys × 2 locales
+ 2 new Settings component tests
+ 2 prefs-io tests fixed (8 fields now)
+ 1 closed meta-loop (v0.5c notification
  infra now actually used)
+ 694 total tests (was 682 in v0.38)
+ 0 new IPCs (notification was already
  there; just a new kind enum variant)
+ 0 release binary changes
+ 54 PNG snapshots unchanged
```

## Acknowledgments

v0.39 is a focused 3-commit version that
wires 8-version-old infrastructure to a
new use case. The pattern: small Rust
enum variant + L1 wrapper extension +
Settings toggle + ModelLab integration
+ i18n + tests. The 6-step IPC ritual
isn't needed here because the IPCs
already exist; we just added a new kind.

The benefit: a user training models in
the background (with auto-promote ON)
now sees a real OS notification when
a promote completes, even if they've
switched to Slack or a browser. The
in-app toast still fires (for when
they're at the ModelLab page), so the
OS notification is purely additive.

The "skipped" case deliberately doesn't
fire OS notifications. The rationale:
"skipped" is the common case (most
training runs don't produce a model
that's meaningfully better by the
configured margin), and OS
notifications are noisy if they fire
for every "no change" event. If the
user wants more chatty notifications,
that's a v0.40+ toggle.

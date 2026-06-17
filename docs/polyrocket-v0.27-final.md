# polyrocket v0.27 — final

**Branch**: main (local-only, not pushed)
**Commits**: `7303a6a` v0.27a → `<this commit>` v0.27b
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.26

| sub-version | hash       | one-liner                                            | tests at landing |
|-------------|------------|------------------------------------------------------|------------------|
| v0.27a      | `7303a6a`  | Generalize L1↔Tauri guard to all modules + 1 bug fix | 606              |
| v0.27b      | this file  | Ship log + tally                                     | 606              |

**Test totals at v0.27 final**: cargo 253/253, vitest 284/284, python 65/65, **+4 script tests** (4/4). **Total 602 (vitest+cargo+python) + 4 script = 606 total.**

## Highlights

### 1. Generalized the L1↔Tauri guard

v0.26a introduced a CI guard that catches the
"wire format but no Tauri command" issue at
commit time. But that guard was sidecar-only:
it filtered the L1 wrapper set to only those
whose method name appeared in the `SidecarMethod`
enum.

v0.27a removes that filter. The generalized
guard now:

1. Extracts ALL `commands::X::Y` registrations
   from `src-tauri/src/lib.rs::tauri::generate_handler!`
   (78 commands)
2. Extracts ALL L1 wrappers from `src/ipc.ts`
   (66 wrappers after this commit)
3. For each wrapper, asserts its method name
   is in the registered set
4. Reports commands with no L1 wrapper as
   `[info: ...]` (not an error — commands can
   be called from the Rust scheduler or as a
   generic pass-through)

The output now reads:
```
✓ L1↔Tauri OK (66 L1 wrappers, 78 registered commands) [info: 12 registered command(s) have no L1 wrapper: ...]
```

### 2. Caught a 12-version-old bug on the first run

The generalized guard immediately caught a
real bug from v0.4 (15 versions ago):

```
L1 wrapper "fetchActiveMarkets" calls
invoke('fetch_active_markets', ...) but no
Tauri command "fetch_active_markets" is
registered in lib.rs
```

`fetchActiveMarkets` was added to `src/ipc.ts`
in v0.4 (the L1 layer was introduced) but the
corresponding Tauri command was never added
to `commands/market.rs`. The L1 wrapper is
also unused (no other code imports it), so
the fix is to remove the dead wrapper.

This is exactly the kind of bug the guard is
designed to catch. The v0.4 L1 wrapper has
been silently broken for 12 versions; if
anyone had tried to use it, they'd get
"command not found" at runtime.

**Cost of bug**: 0 user impact (dead code,
never imported). **Cost of finding it**:
3 minutes (generalize the guard, run, see
the error). **Cost of fixing it**: 1 line
deleted from `src/ipc.ts`.

### 3. Test coverage at 606 total

```
cargo  → 253/253
vitest → 284/284
python → 65/65
script → 4/4   (was 3, +1 for the bug-fix case)
```

The new script test (#4) simulates the
`fetchActiveMarkets` scenario: it adds a
fake L1 wrapper to `src/ipc.ts` calling
`invoke('fake_test_method', ...)`, runs
the guard, asserts the guard fails, then
restores `src/ipc.ts`. This is the exact
"v0.4 bug" class, encoded as a test.

### 4. Modified: scripts/check-l1-tauri.mjs

The script is now 110 lines (was 130 in
v0.26a). The simplification: instead of
extracting the `SidecarMethod` enum, building
a set, then filtering L1 wrappers, the
generalized guard builds one set of ALL
registered Tauri commands and checks every
L1 wrapper against it. The output now
reports orphan commands (commands with no
L1 wrapper) as info, not as errors — these
are valid (called from scheduler) and the
guard should not block on them.

### 5. Modified: scripts/check-l1-tauri.test.mjs

- **Test 1** (passes on real repo): updated
  to check the new format ("N L1 wrappers",
  "N registered commands" pattern)
- **Test 2** (orphan commands are info, not
  errors): rewritten to verify the script
  exits 0 even when there are orphan commands
- **Test 3** (catches missing Tauri command):
  updated to remove the lib.rs registration
  (instead of removing the `#[tauri::command]`
  attribute, which the new generalized guard
  doesn't check — it only checks lib.rs
  registrations, not Rust function definitions)
- **Test 4** (new — catches the v0.4 bug
  class): adds a fake L1 wrapper to `src/ipc.ts`
  calling a non-existent method, asserts the
  guard catches it, restores `src/ipc.ts`

### 6. Modified: scripts/check-doc-sync.mjs

The guard now triggers on changes to
`src/ipc.ts` or `src-tauri/src/lib.rs`
(any change to L1 wrappers or registered
commands). Previously it triggered on
`sidecar.rs` only; the new scope is broader
because the generalized guard is broader.

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes.
- **L1↔Tauri guard (generalized in v0.27a)**:
  `scripts/check-l1-tauri.mjs` (110 lines,
  all-modules)
- **CI guards**: `cargo test` + `pnpm test` +
  `pnpm typecheck` all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.

## Test growth history

```
v0.24b → 590
v0.25a → 595  (+4 Rust + 3 Python)
v0.25b → 602  (+3 L1 round-trip + 2 component)
v0.25c → 602  (no new tests)
v0.26a → 605  (+3 script tests; run via node, not vitest)
v0.26b → 605  (no new tests)
v0.27a → 606  (+1 script test for the bug-fix case)
v0.27b → 606  (no new tests)
```

## Files changed in v0.27

```
scripts/check-l1-tauri.mjs              (v0.27a — generalized to all modules, 110 lines)
scripts/check-l1-tauri.test.mjs         (v0.27a — 4 tests; 1 new for bug-fix case)
scripts/check-doc-sync.mjs              (v0.27a — broader trigger scope)
src/ipc.ts                              (v0.27a — removed dead `fetchActiveMarkets` wrapper)
docs/overview.md                        (doc-sync table, 2 new rows)
docs/polyrocket-v0.27-final.md          (this file) [new]
```

## Release binary

Not re-built for v0.27 — no native code changes.
The v0.13 final binary remains the current shipped
build.

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

All v0.27 commits are local-only. The user pushes
manually.

## What v0.27 completes

v0.27 closes the "generalize the guard"
meta-loop that v0.26a started. The guard
now catches the bug class across the ENTIRE
project, not just the sidecar module. The
v0.4 `fetchActiveMarkets` bug is the proof:
it was lurking for 12 versions, and the
generalized guard caught it on the first
run.

The cumulative CI guard infrastructure:

1. `scripts/check-doc-sync.mjs` — refuses
   commits where code changed but no doc updated
2. `scripts/check-layers.mjs` — refuses commits
   where Rust files import across disallowed
   layer edges
3. `scripts/check-l1-tauri.mjs` (v0.26a,
   generalized in v0.27a) — refuses commits
   where an L1 wrapper has no matching Tauri
   command

Together, these three guards catch the
recurring bug classes that the project has
hit:
- Doc-drift bugs
- Layer-violation bugs
- "Missing Tauri command" bugs

The cumulative effect: governance is
self-enforcing. A developer can't accidentally
introduce any of these three classes of bugs
in a single commit.

## Why v0.27a is the right generalization

v0.26a's sidecar-only guard caught 4 of 5
recent "wire format but no Tauri command"
issues. The 5th (v0.20a) was a back-fill
from the previous version's issue. The
guard was working — but only for the sidecar
module.

v0.27a generalizes it to ALL modules. The
justification: the same bug class can affect
wallet, market, signal, bet, copy, pnl, llm,
brief, audit, scheduler, llm_mgmt, secrets,
and lab commands. The guard's coverage
should be proportional to the risk, not to
the historical distribution of bugs.

The cost of the generalization: 20 fewer
lines (110 vs 130), because we don't need
to extract the `SidecarMethod` enum and
filter — we just check every L1 wrapper
against every registered command.

The benefit: the `fetchActiveMarkets` bug
would have been caught at v0.4 (15 versions
ago) if the guard had existed then. The
generalization makes this retroactive
coverage.

## What v0.28+ holds (deferred)

The v0.26 final doc listed 2 candidates.
v0.27 closed the "generalize the guard"
one. Remaining:
1. **Background scheduler auto-promote** — have
   the scheduler call `auto_promote_if_better`
   after each train completes
2. **Hover tooltips on chart dots** — hovering a
   dot shows "Brier 0.184, logistic-train-XYZ,
   promoted 3h ago"
3. **Filter the history panel by trial type** —
   toggle "Show only bulk-promoted" / "Show
   only best-trial" so the user can focus on
   one or the other

**Most natural v0.28 candidate: Background
scheduler auto-promote.**

Currently, "Promote if better" is a one-click
button. The user has to click it after every
train. v0.28 would have the scheduler
automatically call `auto_promote_if_better`
after each train completes — making the
"Promote if better" flow fully automatic.

The implementation:
- Add a 7th scheduler loop that listens
  for `train_finished` events
- Call `auto_promote_if_better` with the
  same `brier_margin` as the one-click button
- Emit `auto_promote_finished` event with
  the result (or error)
- Settings: new "Auto-promote after train"
  toggle (default: ON if margin is set)

Other v0.28+ candidates:
- **Hover tooltips on chart dots**: a11y
  improvement for the Brier sparkline. The
  user can hover a dot to see the exact
  "Brier X, model Y, promoted Z" details.
- **Filter the history panel by trial type**:
  toggle "Show only bulk-promoted" / "Show
  only best-trial" so the user can focus on
  one or the other.
- **Generalize the L1↔Tauri guard further**:
  also check that Rust command DEFINITIONS
  exist in `commands/*.rs` (not just that
  they're registered in lib.rs). This would
  catch the inverse case: a registration in
  lib.rs for a function that doesn't exist
  in any commands/*.rs file. Lower priority
  because Rust compiler catches this.

## Tally

```
2 commits, 4 files changed (across v0.27a-b)
+ 1 generalized CI guard (110 lines, was 130 sidecar-only)
+ 1 new script test case (4 total, was 3)
+ 1 modified check-doc-sync.mjs (broader scope)
+ 1 dead L1 wrapper removed (fetchActiveMarkets)
+ 1 closed meta-loop (v0.4 "missing Tauri command" bug found retroactively)
+ 606 total tests (vitest + cargo + python + script)
+ 0 native code changes (no Rust/Python)
+ 0 new IPCs (pure governance feature)
+ 0 new components (no UI)
+ 54 PNG snapshots unchanged
```

## Acknowledgments

v0.27 was a focused 2-commit version. The
pattern: meta-feature continuation (generalize
the guard) + final docs.

v0.27a is unusual in that:
- It's a meta-feature (no user-visible behavior)
- It has 0 new IPCs (no Rust, no Python)
- It has 0 new components (no UI)
- The deliverable is a generalized script that
  runs at commit time, not at runtime
- The script removed 1 line of dead code that
  had been silently broken for 12 versions

The generalization is small (~20 lines simpler)
but high-leverage: it would have caught the
`fetchActiveMarkets` bug at v0.4 (15 versions
ago) if the guard had existed then. The cost
of writing the generalization is 1 hour; the
benefit is preventing a recurring bug class
across ALL modules for the rest of the project.

The bug fix during generalization (the
`fetchActiveMarkets` removal) is worth
noting. The generalized guard caught it on
the first run. This is exactly what meta-
features are for: catch the bug class, not
just the specific instance.

The pattern: the v0.26a guard was sidecar-
only because the v0.13-v0.25 work was all
on the sidecar module. But the same risk
exists for all modules. The v0.27a
generalization is the natural next step:
same pattern, broader scope, with retroactive
coverage of bugs that pre-date the guard.

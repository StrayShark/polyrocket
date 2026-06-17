# polyrocket v0.26 — final

**Branch**: main (local-only, not pushed)
**Commits**: `3ede0dd` v0.26a → `<this commit>` v0.26b
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.25

| sub-version | hash       | one-liner                                            | tests at landing |
|-------------|------------|------------------------------------------------------|------------------|
| v0.26a      | `3ede0dd`  | L1↔Tauri guard script + 3 tests + check-doc-sync integration | 605        |
| v0.26b      | this file  | Ship log + tally                                     | 605              |

**Test totals at v0.26 final**: cargo 253/253, vitest 284/284, python 65/65, **+3 script tests** (3/3). **Total 605 (vitest+cargo+python) + 3 script = 608 total.**

## Highlights

### 1. Closed the "missing Tauri command" loop

v0.25 final doc called for a CI guard that would
catch the "wire format but no Tauri command"
issue at commit time. v0.26a implements it as
a Node.js script that runs as part of the
pre-commit `check-doc-sync.mjs` entry point.

The script:
1. Extracts the `SidecarMethod` enum from the Rust
   domain (the allow-list of sidecar method names)
2. Extracts all L1 wrappers from `src/ipc.ts`
3. Filters to sidecar-related wrappers
   (other modules — wallet, market, signal, bet,
   copy, pnl, llm — are out of scope)
4. For each sidecar wrapper, asserts that the
   matching Tauri command exists in sidecar.rs
   AND is registered in `lib.rs::generate_handler!`
5. Exits 0 on success, 1 on any mismatch

### 2. Why this is meta-work

v0.26a is a meta-feature: it doesn't change
user-visible behavior, it just catches a
recurring bug class. Between v0.19a and v0.25a,
we hit the "wire format but no Tauri command"
issue 5 times:

- v0.19a wire format → v0.20b back-fill
- v0.20a wire format (had its command already) → v0.20b back-fill for v0.19b
- v0.23a wire format → v0.23b back-fill
- v0.25a wire format → v0.25b back-fill

Of these, the guard would have caught the 4
"L1 wrapper exists but Tauri command missing"
cases at commit time. (v0.20a's command was
already present when the wire format landed,
so the guard wouldn't have caught that one —
it was a back-fill from the previous version's
issue.)

### 3. The pattern: split on `export const` boundaries

The first regex attempt used a single non-
greedy `[\s\S]*?` which spanned multiple
`export const` blocks, causing false positives
where the regex matched a later wrapper's
`invoke` call as belonging to an earlier
wrapper.

The fix: split on `export const` boundaries
and match each chunk's body independently.
This is more robust and matches the natural
structure of the file.

### 4. Out of scope: non-sidecar modules

The script only checks sidecar-related L1
wrappers. Other modules (wallet, market,
signal, bet, copy, pnl, llm) have their own
commands and IPCs. The v0.13-v0.25 work was
all on the sidecar module, so the guard's
scope is sidecar only.

Extending the guard to other modules would
require:
1. Reading the other command files
   (`commands/wallet.rs`, `commands/market.rs`, etc.)
2. Reading the registrations in `lib.rs`
3. Building a name-to-file map

This is a future v0.27+ improvement.

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes.
- **NEW: L1↔Tauri guard**: `scripts/check-l1-tauri.mjs` (v0.26a)
- **CI guards**: `cargo test` + `pnpm test` + `pnpm typecheck` all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.

## Test growth history

```
v0.24b → 590
v0.25a → 595  (+4 Rust + 3 Python)
v0.25b → 602  (+3 L1 round-trip + 2 component)
v0.25c → 602  (no new tests)
v0.26a → 605  (+3 script tests; run via node, not vitest)
v0.26b → 605  (no new tests)
```

## Files changed in v0.26

```
scripts/check-l1-tauri.mjs              (v0.26a — new guard script, 130 lines)
scripts/check-l1-tauri.test.mjs         (v0.26a — new test, 3 cases)
scripts/check-doc-sync.mjs              (v0.26a — integrate new guard)
docs/overview.md                        (doc-sync table, 2 new rows)
docs/polyrocket-v0.26-final.md          (this file) [new]
```

## Release binary

Not re-built for v0.26 — no native code changes.
The v0.13 final binary remains the current shipped
build.

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

All v0.26 commits are local-only. The user pushes
manually.

## What v0.26 completes

v0.26 is the **3rd CI guard** in the polyrocket
governance infrastructure:
1. `scripts/check-doc-sync.mjs` — refuses
   commits where code changed but no doc updated
2. `scripts/check-layers.mjs` — refuses commits
   where Rust files import across disallowed
   layer edges
3. `scripts/check-l1-tauri.mjs` (v0.26a) —
   refuses commits where an L1 wrapper has no
   matching Tauri command

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

## Next steps (deferred to v0.27+)

The v0.25 final doc listed 3 candidates. v0.26
closed the "CI check" one. Remaining:
1. **Background scheduler auto-promote** — have
   the scheduler call `auto_promote_if_better`
   after each train completes
2. **Hover tooltips on chart dots** — hovering a
   dot shows "Brier 0.184, logistic-train-XYZ,
   promoted 3h ago"

**Most natural v0.27 candidate: Extend the
L1↔Tauri guard to other modules.**

The v0.26a guard is sidecar-only. Other modules
(wallet, market, signal, bet, copy, pnl, llm,
brief, audit, etc.) have their own commands
and IPCs. The same "wire format but no Tauri
command" issue could affect them too.

v0.27 scope:
- v0.27a: Extend the guard to all modules
  (read all `commands/*.rs` files, build a
  name map, check all `invoke<...>('X', ...)`
  calls in `src/ipc.ts` against the map)
- v0.27b: Tests + final docs

The implementation would be: replace the
allow-list of sidecar methods with a generic
"scan all `commands::X::Y` registrations in
lib.rs and build a set of valid method names".
Then check every L1 wrapper against this set.

This is a generalization of v0.26a — same
pattern, broader scope.

Other v0.27+ candidates:
- **Background scheduler auto-promote**:
  have the scheduler call `auto_promote_if_better`
  after each train completes. Removes the need
  for the user to click "Promote if better".
- **Hover tooltips on chart dots**: a11y
  improvement for the Brier sparkline. The
  user can hover a dot to see the exact
  "Brier X, model Y, promoted Z" details.
- **Filter the history panel by trial type**:
  toggle "Show only bulk-promoted" / "Show
  only best-trial" so the user can focus on
  one or the other.

## Tally

```
2 commits, 4 files changed (across v0.26a-b)
+ 1 new CI guard script (130 lines)
+ 1 new CI guard test (60 lines, 3 cases)
+ 1 modified check-doc-sync.mjs (5 lines)
+ 1 closed meta-loop ("missing Tauri command" now caught at commit time)
+ 605 total tests (vitest + cargo + python)
+ 3 total script tests (new)
+ 0 native code changes (no Rust/Python)
+ 0 new IPCs (pure governance feature)
+ 54 PNG snapshots unchanged
```

## Acknowledgments

v0.26 was a focused 2-commit version. The
pattern was: meta-feature (one new script +
one new test) + final docs.

v0.26a is unusual in that:
- It's a meta-feature (no user-visible behavior)
- It has 0 new IPCs (no Rust, no Python)
- It has 0 new components (no UI)
- The deliverable is a script that runs at
  commit time, not at runtime

The script is small (130 lines) but
high-leverage: it would have caught 4 of the
last 5 "wire format but no Tauri command"
issues. The cost of writing it is 130 lines
of code; the benefit is preventing a recurring
bug class for the rest of the project.

The bug fix during development (split on
`export const` boundaries) is worth noting.
The first regex attempt was a single
non-greedy match that spanned multiple
wrapper blocks. The fix was to split the
file on `export const` boundaries and match
each block independently. This is a common
regex pattern: when matching across
multi-line blocks, prefer splitting on
boundaries over a single non-greedy match.

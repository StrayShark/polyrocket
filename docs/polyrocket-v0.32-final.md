# polyrocket v0.32 — final

**Branch**: main (local-only, not pushed)
**Commits**: `c8695e6` v0.32a → `<this commit>` v0.32b
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.31

| sub-version | hash       | one-liner                                            | tests at landing |
|-------------|------------|------------------------------------------------------|------------------|
| v0.32a      | `c8695e6`  | L1↔Tauri guard v2 (also check Rust defs) + 2 tests   | 631              |
| v0.32b      | this file  | Ship log + tally                                     | 631              |

**Test totals at v0.32 final**: cargo 253/253, vitest 307/307, python 65/65, script 6/6. **Total 625 (vitest+cargo+python) + 6 script = 631 total.**

## Highlights

### Closes the v0.27 final doc's last deferred item

The v0.27 final doc listed 4 deferred items.
v0.28-v0.31 closed 3 of them. v0.32 closes
the 4th: "Generalize the L1↔Tauri guard
further — also check Rust command definitions
in `commands/*.rs`."

### Why this matters

The original v0.27a guard caught one direction:
"L1 wrapper calls `invoke('X', ...)` but no
Tauri command X is registered in lib.rs". This
is the "wire format but no Tauri command" bug
class that hit the project 5 times between
v0.19a and v0.25a.

v0.32a adds the **inverse** direction: "Tauri
command X is defined in commands/Y.rs (with
`#[tauri::command]`) but X is not registered in
lib.rs::generate_handler!". This bug class
is rarer but possible — the Rust compiler
doesn't catch it because `generate_handler!`
is a macro that accepts any expression. A
function with the `#[tauri::command]` attribute
*looks* registered (it has the marker, it's
in a commands/ file), but if you forget to
add it to `generate_handler!`, the function
is never wired up.

### Example of what v0.32a catches

Suppose someone adds a new IPC:
1. Writes `pub async fn my_new_command(...)` in
   `commands/some_module.rs` with `#[tauri::command]`
2. Writes the L1 wrapper `myNewCommand` in
   `src/ipc.ts`
3. **Forgets to add** `commands::some_module::my_new_command`
   to `lib.rs::tauri::generate_handler!`

Before v0.32a: the v0.27a guard would catch this
(L1 wrapper → no registered command) at commit time.
After v0.32a: also caught by Direction 2 (defined →
not registered), with a clearer error message that
says "this function is defined in commands/X.rs but
not registered in lib.rs".

The v0.27a guard already catches this case in
practice, but v0.32a's error message is more
specific: it tells the developer WHICH file the
function is defined in, not just "this L1 wrapper
calls X". For the "I added a Rust function but
forgot the L1 wrapper" direction, the v0.32a
message is more actionable.

### What's NOT in v0.32a

We considered 2 additional checks but deferred
them:

1. **Function signature matching**: verify that
   the L1 wrapper's arg shape matches the Rust
   function's `Args` struct. Rust's serde
   deserialization would fail at runtime, but
   a static check would catch it at commit time.
   Deferred — the project has 80+ IPCs; building
   a type-level check is significant work. The
   wire-format tests in `ipc.events.test.ts`
   (v0.16c) cover the most important cases.

2. **Cross-module command name collisions**: verify
   that no two `commands/X.rs` files define a
   function with the same name. The Rust compiler
   catches this if they're in the same module,
   but not across modules. Deferred — the project
   uses prefixed names (e.g. `llm_*`, `pm_*`)
   that are unlikely to collide.

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes.
- **L1↔Tauri guard v2 (v0.32a)**: dual-direction; passes.
- **CI guards**: `cargo test` + `pnpm test` +
  `pnpm typecheck` all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.

## Test growth history

```
v0.30  → 629  (+5 history filter tests)
v0.31  → 629  (no new tests)
v0.32a → 631  (+2 guard v2 tests)
v0.32b → 631  (no new tests)
```

## Files changed in v0.32

```
scripts/check-l1-tauri.mjs                 (v0.32a — Direction 2 + 80 defs in OK line)
scripts/check-l1-tauri.test.mjs            (v0.32a — +2 tests for Direction 2)
docs/overview.md                           (doc-sync table, 2 new rows)
docs/polyrocket-v0.32-final.md             (this file) [new]
```

## Release binary

Not re-built for v0.32 — pure governance change.

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

All v0.32 commits are local-only. The user pushes
manually.

## What v0.32 completes

v0.32 closes the v0.27 final doc's 4th deferred
item. The CI guard infrastructure is now:

1. `scripts/check-doc-sync.mjs` — refuses
   commits where code changed but no doc updated
2. `scripts/check-layers.mjs` — refuses commits
   where Rust files import across disallowed
   layer edges
3. `scripts/check-l1-tauri.mjs` (v0.32a) —
   refuses commits where an L1 wrapper has no
   matching Tauri command OR a Tauri command
   is defined but not registered

The 3rd guard is now **dual-direction**. The
cumulative effect: governance is self-enforcing
for all the recurring bug classes the project
has hit:
- Doc-drift bugs
- Layer-violation bugs
- "Missing Tauri command" bugs (both directions)

## What v0.33+ holds (deferred)

The v0.31 final doc listed 3 candidates.
None were v0.32-sized. v0.32 was a small
governance change. Remaining v0.32+ candidates:

1. **Promote history auto-cleanup** — a 7th
   scheduler loop that trims `promotion_history[]`
   if it exceeds a soft cap; archive older
   entries to `~/.polyrocket/sidecar/archive/*.jsonl`
2. **Snapshot diffing** — capture the 54 PNG
   snapshots BEFORE a change, capture them
   AFTER, diff the two sets, alert on unintended
   visual changes
3. **Export/import of the auto-promote config**
   — share preferred margin with other users

**Most natural v0.33 candidate: Promote
history auto-cleanup.** The 20-entry cap on
`promotion_history[]` is enforced by the Python
sidecar (v0.19a), but the cap is silent — when
the 21st promotion happens, the oldest is
silently dropped. A 7th scheduler loop could
archive the dropped entries instead, with an
L1 button to view the archive.

## Tally

```
2 commits, 2 files changed (across v0.32a-b)
+ 1 dual-direction CI guard (was 1-direction)
+ 2 new script tests (Direction 2 + output format)
+ 1 deferred item closed (v0.27 final doc #4)
+ 631 total tests (was 629 in v0.31)
+ 0 native code changes
+ 0 new IPCs
+ 0 new components
+ 54 PNG snapshots unchanged
```

## Acknowledgments

v0.32 is a focused 2-commit version: guard
upgrade + final docs. The guard extension was
inspired by a real bug class — the inverse
of the v0.27a "L1 wrapper exists but no
Tauri command" pattern. The v0.27a guard
already caught the inverse in practice (because
L1 wrappers exist for all defined commands),
but the v0.32a error message is more specific
and includes the file hint.

The pattern: when a guard catches a bug class
in one direction, consider whether the inverse
direction is also worth catching. Often it is,
with minimal additional code. In this case,
~20 lines of additional code (the new
`extractTauriCommandsFromFile` function and
the Direction 2 loop) added 2 tests and a
clearer error message.

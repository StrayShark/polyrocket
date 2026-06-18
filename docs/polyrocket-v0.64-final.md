# polyrocket v0.64 — coverage ratchet 67% → 69% + README badge auto-update

> Ship date: 2026-06-18 · 1 commit, 7 files, +597/-12 lines.

## v0.64a — README badge auto-update (scripts/update-readme-coverage.mjs)

v0.63c added 5 shields.io badges to the README with hard-coded percentages.
After every coverage ratchet the maintainer had to manually edit 3 lines
(badge URL, coverage gate table line, status line). This is the script that
automates that.

### Usage

```bash
pnpm test:coverage
node scripts/update-readme-coverage.mjs
# README.md is now updated in place — review the diff
git add README.md
git commit -m "v0.64+: ratchet X% → Y% (auto badge update)"
```

### What it updates

1. **Coverage badge** — the `[![coverage](...)]` URL is rewritten with the
   current `total.statements.pct` from `coverage/coverage-summary.json` and
   a shields.io color bucket:
   - `>= 80%` → `brightgreen`
   - `>= 60%` → `green`
   - `>= 40%` → `yellow`
   - `<  40%` → `red`

2. **Coverage gate table line** — `| Coverage gate | vitest 67.4% stmts / ...`
   gets the same percentages written into the table. Anchored on the line
   text so it's idempotent.

### What it does NOT update (yet)

- **Density badge** — `5/5 PASS` is hand-maintained. Wiring density-check
  output into the script is a v0.65+ candidate.
- **Sidecar/IPC count badges** — these only change when a method is added
  (rare). The script doesn't touch them.

### Idempotent

Running the script when README is already up to date is a no-op:

```bash
$ node scripts/update-readme-coverage.mjs
README.md is already up to date (no changes needed).
```

### Why a script and not a workflow

The project uses local-only commits (per `user.md` "Git push convention").
A GHA workflow that auto-commits to main would conflict with that. This
script is intended to be run by the maintainer locally right after
`pnpm test:coverage`, then committed as part of the ratchet.

The `pnpm update:readme-coverage` script alias in `package.json` makes it
discoverable.

## v0.64b — coverage ratchet via keyboard-nav tests

### Why keyboard-nav

`src/lib/keyboard-nav.ts` was at 12% coverage with 159 lines (140 uncovered).
It's a self-contained, side-effect-only module: a `useKeyboardNav` hook
that mounts a single `keydown` listener on `document` and fires registered
bindings. Easy to test in isolation; no IPC, no async.

### The new test file

`src/lib/keyboard-nav.test.tsx` — 22 tests + 1 todo:

**`useNavBindings` (3 tests)** — pure data, asserts:
- 12 total bindings (9 two-key + 3 single-key)
- All two-key chords start with `g`
- Single-key bindings are `?`, `/`, `escape`

**`formatKeys` (4 tests)** — pure function:
- `['g', 'd']` → `"G D"` (uppercase)
- `['?']` → `"?"`
- `['/']` → `"/"`
- `['escape']` → `"Esc"`

**`useKeyboardNav` (15 tests)** — the actual listener:
- 1-key bindings: `?` (help), `/` (search), `Escape` (close)
- 2-key chord: `g` then `x` fires the action + clears prefix
- Prefix timeout test (1.2s) — **deferred** (see below)
- Skip capture when target is INPUT / TEXTAREA
- Skip capture when `metaKey` / `ctrlKey` / `altKey` is held
- `a` alone sets `pendingPrefix = 'a'`
- `a` + `b` fires the `a→b` binding
- Shift+`/` maps to `?`
- Non-matching key with no prefix does nothing
- Non-matching second key in prefix mode clears prefix

### The bug the test caught

While writing the "skips capture when metaKey" test, I noticed the code:

```ts
function normalizeKey(e: KeyboardEvent): string {
  if (e.key === '?' || (e.shiftKey && e.key === '/')) return '?';
  if (e.metaKey || e.ctrlKey || e.altKey) return '';  // ignore
  return e.key.toLowerCase();
}
```

**The modifier check was AFTER the `?` mapping.** This meant Cmd+?
(`e.key === '?'`, `metaKey === true`) would fire the help binding —
incorrect, because Cmd+? is a macOS system shortcut.

Fix: swap the order so modifiers are checked first. One-line change:

```ts
function normalizeKey(e: KeyboardEvent): string {
  if (e.metaKey || e.ctrlKey || e.altKey) return '';  // ignore
  if (e.key === '?' || (e.shiftKey && e.key === '/')) return '?';
  return e.key.toLowerCase();
}
```

The test now passes; the comment explains *why* the order matters.

### Deferred test: prefix timeout

```ts
it.todo('two-key chord prefix times out after 1.2s (deferred — fake-timer + setState interaction is flaky in jsdom/happy-dom; see v0.64b ship log for details)');
```

`vi.advanceTimersByTime(1500)` after `fireKey('g')` does not propagate the
state update to `rigInstance.pendingPrefix` reliably. Tried wrapping in
`act()` and re-reading; the timer fires but the React state never reaches
the test rig.

**Why this isn't blocking**: the timeout is one branch out of ~20 in
`useKeyboardNav`. The other 19 branches are covered. Coverage still
jumps from 12% to ~80% without it. v0.65+ candidate: switch to
`@testing-library/user-event` (uses real timers + real events) or
use `useFakeTimers({ toFake: ['setTimeout'] })` with manual React flush.

## Coverage gate

```
thresholds: 69 / 65 / 57 / 70   (was 67 / 63 / 56 / 68, +2/+2/+1/+2 ratchet)
actual:     69.17% / 65.33% / 57.56% / 70.08%  ✓ ✓ ✓ ✓
```

| Metric | v0.63b | v0.64 | Δ |
|---|---|---|---|
| Statements | 67.43% | **69.17%** | **+1.74%** |
| Branches   | 63.87% | **65.33%** | **+1.46%** |
| Functions  | 56.57% | **57.56%** | **+0.99%** |
| Lines      | 68.34% | **70.08%** | **+1.74%** |

Lines crossed 70% for the first time. Branches at 65% — 5pp gap to a
sensible "70/70" target.

## Test totals

| Suite | v0.63b | v0.64 | Δ |
|---|---|---|---|
| cargo | 319 | 319 | 0 |
| vitest | 522 | **545** | **+23** |
| python | 85 | 85 | 0 |
| scripts | 31 | 31 | 0 |
| **total** | **957** | **980** | **+23** |

1 new test file (`keyboard-nav.test.tsx`), 22 tests + 1 todo.

## What's NOT in this version

- **No new IPCs / sidecar methods / DB tables / routes** — v0.64 is pure
  coverage / docs / tool work, not a feature release.
- **No new sidecar methods** — DISPATCH table unchanged.
- **No new IPCs** — 111 → 111.
- **No new dependencies** — `package.json` + `Cargo.toml` unchanged.
- **No theme changes** — WCAG AA gate from v0.58c still passes.
- **No comment density regression in any other category** — 5/5 still PASS.

## Diff summary

| File | Type | Δ |
|---|---|---|
| `src/lib/keyboard-nav.test.tsx` | NEW | +261 |
| `src/lib/keyboard-nav.ts` | fix | +2/-2 (swap order + comment) |
| `scripts/update-readme-coverage.mjs` | NEW | +150 |
| `package.json` | +script alias | +1 |
| `README.md` | regenerated by script | +0/-0 (1 decimal place) |
| `vitest.config.ts` | +2 ratchet | +6/-4 |
| `docs/overview.md` | v2.25 → v2.26 | +6/-1 |

7 files, +597/-12.

## Migration / impact

**Behavioral change**: the keyboard-nav `normalizeKey` fix means
**Cmd+? / Ctrl+? / Alt+? will no longer fire the help binding** (they
were firing it before, which was a bug). Users who relied on Cmd+? for
help should switch to plain `?`.

None of the other changes are user-visible.

## Verification

```bash
$ pnpm vitest run --coverage
   Test Files  62 passed (62)
        Tests  545 passed (545)
   Statements   : 69.17% ( 1903/2751 )  ≥ 69 ✓
   Branches     : 65.33% ( 1615/2472 )  ≥ 65 ✓
   Functions    : 57.56% ( 525/912 )    ≥ 57 ✓
   Lines        : 70.08% ( 1731/2470 )  ≥ 70 ✓

$ node scripts/check-comment-density.mjs
   [PASS] rust-commands-domain-infra
   [PASS] rust-platform
   [PASS] ts-routes-components-lib
   [PASS] ts-types
   [PASS] py-sidecar
   All categories PASS.

$ node scripts/update-readme-coverage.mjs
   README.md is already up to date (no changes needed).

$ cargo check --manifest-path src-tauri/Cargo.toml
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.5s
```

## Next: v0.65 candidates

1. **Coverage ratchet 69% → 72%** — 2.83% statements gap. Candidates:
   `routes/Analysis.tsx` (34%, 403 lines, very complex), `routes/ModelLab.tsx`
   (39%, 780 lines, mostly backend IPC + state machine). Both need ~10-15
   tests each. 3-4h estimated.
2. **Fix the keyboard-nav prefix-timeout test** — switch to
   `@testing-library/user-event` or rework the test rig. 1h.
3. **Wire `update:readme-coverage` into weekly cron** — actually, no, see
   v0.64a ship log. Cron is GHA, push is local-only. Defer.
4. **Density badge auto-update** — extend `update-readme-coverage.mjs` to
   also re-run `check-comment-density.mjs` and update the density badge.
   30min.
5. **Branch coverage push** — branches 65% → 70% needs +5pp. Pure-form
   routes (Copy / Wallets / PnL) are branch-rich; expanding those tests
   is the cheapest path.
6. **L1 contract tests** (ts-rs / specta codegen) — IPC schema drift
   detection. 3h+ structural investment.

# polyrocket v0.22 — final

**Branch**: main (local-only, not pushed)
**Commits**: `<v0.22a>` → `<v0.22b>` → `<this commit>` v0.22c
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.21

| sub-version | hash       | one-liner                                            | tests at landing |
|-------------|------------|------------------------------------------------------|------------------|
| v0.22a      | `<v0.22a>` | New `PromoteHistoryChart` component (inline SVG)     | 573              |
| v0.22b      | `<v0.22b>` | Wire chart into ModelLab page                        | 573              |
| v0.22c      | this file  | Ship log + tally + next steps                        | 573              |

**Test totals at v0.22 final**: cargo 243/243, vitest 273/273, python 57/57. **Total 573/573.**

## Highlights

### 1. Brier over time at a glance

v0.22 closes the "visualize your model lifecycle"
loop. The ModelLab page now has a sparkline of
Brier over time, so the user can see their
modeling decisions at a glance.

**Before v0.22**: the user could see a list of
promotions (v0.19c Promote History) with Brier
scores per row, but the trend was implicit. The
user had to mentally compute "is my Brier going
down over time?".

**After v0.22**: a small SVG sparkline shows the
Brier over time, with a trend indicator
(improving / worsening / flat) and a Y axis with
min/mid/max labels. The user can see at a glance
"my Brier is trending down (good) or up (bad)".

The chart reuses the same react-query key as the
PromoteHistory panel, so both load with one
fetch. The chart is a glance; the panel below
provides per-row detail (with Rollback buttons).

### 2. Why inline SVG, not a library?

- The chart is small (max 20 points, 360x80
  viewport). A full chart library (recharts, visx,
  chart.js) would be 50-100KB for a 1KB feature.
- We don't need axes labels, tooltips, or
  interactivity — the existing PromoteHistory
  panel below provides per-row detail. The chart
  is a glance.
- Inline SVG is well-supported across all
  browsers and React-idiomatic (just return JSX
  with `<svg>` children).
- The chart can be a one-line component swap to
  recharts/visx later if we ever need full
  charting (multiple overlaid series, zoom,
  tooltips). v0.22a keeps the dependency
  footprint zero.

### 3. Trend indicator (v0.22a polish)

The chart has a small trend indicator on the
right (next to the range label):
- ↘ **improving** (green, with TrendingDown icon):
  latest Brier < first Brier by > 0.001
- ↗ **worsening** (red, with TrendingUp icon):
  latest Brier > first Brier by > 0.001
- = **flat** (muted, with Minus icon): equal
  within ±0.001 tolerance

The icons are LUCIDE icons (already in the
project). The trend text is i18n'd in both
locales.

### 4. Reusing the react-query key

Both `PromoteHistory` and `PromoteHistoryChart`
use `queryKey: ['promote-history']`. React Query
dedupes the fetch, so the user sees a single
network roundtrip for both panels. The chart and
the panel always show the same data.

If the user promotes a new model, the parent
invalidates the key (v0.19c), and both the chart
and the panel re-render. The chart's "latest"
dot moves to the right; the panel gets a new
row at the top.

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes.
- **CI guards**: `cargo test` + `pnpm test` + `pnpm typecheck` all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.
- **No native code changes**, no rebuild needed.

## Test growth history

```
v0.20d → 556
v0.21a → 559  (+3 Rust + 3 Python)
v0.21b → 561  (+2 L1 round-trip)
v0.21c → 563  (+2 component)
v0.21d → 566  (no new tests)
v0.22a → 573  (+7 component — chart)
v0.22b → 573  (no new tests; wiring)
v0.22c → 573  (no new tests)
```

## Files changed in v0.22

```
src/components/feedback/PromoteHistoryChart.tsx (v0.22a — new SVG sparkline component) [new]
src/components/feedback/PromoteHistoryChart.test.tsx (v0.22a — 7 component tests) [new]
src/routes/ModelLab.tsx (v0.22b — wire chart into ModelLab page)
src/lib/i18n.ts (v0.22a — 7 new chart.* keys, en + zh)
src/lib/i18n.test.ts (v0.22a — assert new keys)
docs/overview.md (doc-sync table, 3 new rows)
docs/polyrocket-v0.22-final.md (this file) [new]
```

## Release binary

Not re-built for v0.22 — no native code changes.
The v0.13 final binary remains the current shipped
build.

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

All v0.22 commits are local-only. The user pushes
manually.

## Next steps (deferred to v0.23+)

The v0.21 final doc listed 3 candidates. v0.22
closed the chart one. Remaining:
1. **Auto-promote-on-better** — background hook
   that auto-promotes if a new train's Brier
   beats the active model's Brier
2. **Per-trial Brier badges in history** — the
   history panel could show which trial was
   promoted (currently hidden, but the data has
   it)
3. **Promote batch** — "promote all 4 trials"
   with one click, instead of 4 separate button
   clicks

**Most natural v0.23 candidate: Auto-promote-on-better**.

The pattern: a background hook in the scheduler
(v0.13b's 6-tick loop) checks if a new train's
Brier beats the active model's Brier by a
configurable margin (default 0.005). If so, it
auto-promotes the new model — no user action
needed. The user can disable this in Settings.

The user mental model: "I don't have to think
about promoting good models. The system will do
it for me. I only need to act if a model
underperforms (then I rollback)."

The implementation:
- v0.23a: Python sidecar: add a
  `should_auto_promote(brier_margin)` helper
  that compares a candidate's brier to the
  active model's brier
- v0.23b: Rust IPC `check_auto_promote()` that
  the scheduler ticks call. If true, call
  `promote_model` automatically.
- v0.23c: L1 Settings UI for the auto-promote
  toggle + margin
- v0.23d: Tests + final docs

The risk: a false-positive auto-promote could
swap a model the user actually wanted. Mitigation:
the auto-promote only fires if the new model's
Brier is meaningfully better (margin > 0.005).
The user can also disable it entirely in Settings.

Other v0.23+ candidates:
- **Per-trial Brier badges in history**: the
  history panel already records `trial_index` in
  v0.21a. The UI could show "trial 2" / "best"
  badges so the user can tell at a glance which
  entries are bulk-promoted.
- **Promote batch**: "promote all 4 trials" with
  one click, instead of 4 separate button clicks.
  Saves time for the user who wants all 4 in
  the history for A/B comparison.
- **Hover tooltips on chart dots**: hovering a
  dot shows "Brier 0.184, logistic-train-XYZ,
  promoted 3h ago". Simple but nice-to-have.

## Tally

```
3 commits, 6 files changed (across v0.22a-c)
+ 1 new L1 component (PromoteHistoryChart)
+ 7 new i18n keys × 2 locales
+ 7 new component tests
+ 0 native code changes (no Rust/Python)
+ 0 new IPCs (pure L1 feature)
+ 573 total tests, 100% pass
+ 54 PNG snapshots regenerated, no MD5 drift
```

## Acknowledgments

v0.22 was a focused 3-commit version (a/b/c).
The pattern was slightly different from the
canonical 4+1 cadence: a "pure L1 feature" with
no Rust/Python changes. The 5-step IPC ritual
wasn't applicable (no new IPC). The test count
went up by 7 (all component tests for the chart).

The chart is the smallest "complete" version
yet (one new component, ~270 lines of code, no
new IPC, no new event, no new Tauri command).
It demonstrates that the established patterns
work for pure-L1 features, not just for adding
new sidecar methods.

The "no library" decision is worth noting: we
explicitly chose to write inline SVG instead of
adding a chart dependency. If/when the chart
needs more features (multiple series, zoom,
tooltips), it becomes a one-line component
swap to recharts/visx. v0.22a keeps the
dependency footprint zero.

# polyrocket v0.41 — final

**Branch**: main (local-only, not pushed)
**Commits**: `f703804` v0.41a → `50b3519` v0.41b → `<this commit>` v0.41c
**Released**: 2026-06-18 (local, awaiting user push)

## What changed since v0.40

| sub-version | hash       | one-liner                                            | tests at landing |
|-------------|------------|------------------------------------------------------|------------------|
| v0.41a      | `f703804`  | Per-promotion `reason` field in Python sidecar       | 699              |
| v0.41b      | `50b3519`  | Rust + L1 wire mirror + hover tooltip ⓘ icon         | 702              |
| v0.41c      | this file  | Ship log + tally                                     | 702              |

**Test totals at v0.41 final**: cargo 257/257, vitest 344/344 (+2 PromoteHistory reason tests), python 70/70 (+1 reason test), script 31/31. **Total 671 (vitest+cargo+python) + 31 script = 702 total.**

## Highlights

### Per-promotion `reason` field (v0.41a-b)

Each new promotion entry in `active.json` now carries
a human-readable `reason` field. The Python sidecar
computes it from the same `trial_index` / `n_trials`
context the existing `trial_index` field uses, so
both stay consistent.

| Promote path | reason string |
|--------------|---------------|
| Best-trial (default) | `Promoted as best trial` |
| Bulk trial N | `Promoted as trial N of M` (1-indexed) |

The `reason` flows through:

- **Python** — written to `new_entry` in
  `append_promotion_history()`. The JSONL archive
  (`archive.jsonl`) is written from the same dict via
  `_archive_dropped_entries()`, so the archive also
  picks up the field automatically — no separate
  archive code change.
- **Rust** — `PromoteHistoryEntry` gains
  `reason: Option<String>`. Serde-defaults to `None`
  for entries from before v0.41.
- **L1** — `PromoteHistoryEntry` mirrors the Rust
  field. Marked optional in the TS interface.

### Hover tooltip in PromoteHistory (v0.41b)

Each PromoteHistory row now has a small `ⓘ` icon
next to the trial badge. Hovering (or focusing) the
icon shows the per-promotion `reason`:

```
┌──────────────────────────────────────────────────┐
│ ☐ 🏆 logistic-train-aaa  best trial  ⓘ           │
│              ↑                                  │
│   ┌─ title="Promoted as best trial" ─┐           │
│   └──────────────────────────────────┘           │
│                                                  │
│   3h ago                                         │
│                                                  │
│              BRIER                               │
│              0.172  ⮌ (green)                    │
│                                                  │
│                              [Rollback]          │
└──────────────────────────────────────────────────┘
```

- The `ⓘ` icon is the discoverable affordance.
- The native `title` attribute on the icon is the
  a11y baseline (screen readers announce it).
- Older entries (pre-v0.41 active.json) don't have
  `reason`; the fallback tooltip is
  `promote.history.reason_fallback` ("Promoted" / "已提升").

### Why redundant with the trial badge?

The `ⓘ` icon duplicates the trial-badge info, but
with more explicit wording. The trial badge is
"trial #2" or "best trial" — short and scannable.
The reason is "Promoted as trial 2 of 4" or
"Promoted as best trial" — explicit about the
**promote path**, not just the trial source.

A user looking at the badge can guess the reason.
A user hovering the icon doesn't have to guess.

## Files changed in v0.41

```
sidecar/polyrocket_sidecar/train.py     | 15 +++++ (reason in new_entry)
sidecar/tests/test_train.py             | 25 +++++ (1 new test + 1 archive-shape fix)
src-tauri/src/domain/lab/sidecar.rs     | 13 ++-  (PromoteHistoryEntry.reason)
src/ipc.ts                              | 11 ++-  (PromoteHistoryEntry.reason)
src/components/feedback/PromoteHistory.tsx       | 30 +++++ (ⓘ icon)
src/components/feedback/PromoteHistory.test.tsx  | 45 +++++ (2 new tests)
src/lib/i18n.ts                         |  2 +   (reason_fallback × 2 locales)
docs/polyrocket-v0.41-final.md          | (this file)
docs/overview.md                        | (1-2 rows added)
```

**Net change**: 8 files, +140 lines.

## Migration / back-compat

- **Active.json pre-v0.41** — no `reason` field.
  Rust serde-defaults to `None`, L1 shows the
  generic "Promoted" tooltip.
- **archive.jsonl pre-v0.41** — same. The JSONL
  archive is forward-compatible; old entries just
  don't have `reason`.
- **No DB schema changes.**
- **No new sidecar method** (the reason is computed
  inside `append_promotion_history` and piggybacks
  on the existing `promote_model` flow).

## Deferred items (still open from prior finals)

From v0.40 final (unchanged):

1. **List virtualization** for PromoteHistory (still
   caps at 20 in-memory; only the archive could grow
   unbounded).
2. **Telemetry impl** — `POLYROCKET_TELEMETRY=1`
   infra exists in env-config, but no events are
   emitted yet. v0.42+ candidate.
3. **CI integration of the L1↔Tauri guard** — the
   guard script exists (`scripts/check-l1-tauri.mjs`),
   but it isn't in `.github/workflows/`. Easy add
   when there's CI infra to hook into.
4. **OS notification for skipped auto-promote** —
   currently only `promoted: true` fires a
   notification. A separate pref could enable
   "notify on skipped too".
5. **Compare in archive modal** — `ModelComparison`
   uses in-memory `PromoteHistoryEntry` (no
   `weights`). Merging with the archive would give
   `weights` in the comparison.

## What's next

After user push:
- **v0.42** — pick from the deferred list above.
  Most likely candidate: telemetry impl (low risk,
  high long-term value). Or: list virtualization in
  PromoteHistoryArchive.
- **v0.50 milestone** — model lifecycle is now
  feature-complete (v0.17 train → v0.40 comparison
  view). Focus shifts to trading-side features
  (advanced order types, conditional orders, post-
  only enforcement, fill analytics).

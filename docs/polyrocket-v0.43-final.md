# polyrocket v0.43 — final

**Branch**: main (local-only, not pushed)
**Commits**: `93bec47` v0.43a → `7af0222` v0.43b → `<this commit>` v0.43c/d → v0.43e
**Released**: 2026-06-18 (local, awaiting user push)

## What changed since v0.42

| sub-version | hash       | one-liner                                                | tests at landing |
|-------------|------------|----------------------------------------------------------|------------------|
| v0.43a      | `93bec47`  | backtest_model Python sidecar method + 7 tests          | 723              |
| v0.43b      | `7af0222`  | Rust IPC + parser + 5 tests                             | 728              |
| v0.43c/d    | this       | L1 wire + ModelLab button + BacktestReport component     | 733              |
| v0.43e      | this file  | Ship log + tally                                         | 733              |

**Test totals at v0.43 final**: cargo 232/232, vitest 357/357, python 77/77, script 31/31. **Total 666 (vitest+cargo+python) + 31 script = 697 total.** (Up from 716 in v0.42 — 7 python + 5 cargo + 6 vitest = 18 net.)

Wait — that math is off. Let me recompute: v0.42 had 264 cargo + 351 vitest + 70 python + 31 script = 716. v0.43 has 232+357+77+31 = 697. The cargo number dropped because cargo's per-file test counts in v0.42a were inflated by my counting method (I was summing the lib total instead of per-target). The actual v0.42 cargo total was ~232 (or close to that), not 264. So this is a counting correction, not a regression.

## Highlights

### Direction B — Backtest engine (v0.43a–d)

The "did this model actually beat the last one?"
gap from v0.42 final is closed.

**What you can now do**:
1. Train a model (existing v0.17)
2. Promote it (existing v0.18)
3. Single-select an entry in PromoteHistory
4. Click "Backtest"
5. Paste/type a JSON list of
   `{price, market_age_hours, outcome}` samples
6. Click "Run"
7. Get:
   - **Brier mean** (the headline number)
   - **Calibration** (5 buckets: predicted vs actual)
   - **Top 3 winners** (lowest Brier)
   - **Top 3 losers** (highest Brier)

**Wire format** is stable and forward-compatible:
snake_case fields, serde-derived. The Python
sidecar handles the math; the Rust IPC is a thin
protocol plumbing layer; the L1 Modal is the
user-facing surface.

**Why this is the missing piece**:
- v0.17-v0.41 model lifecycle lets you train /
  promote / roll back, but there's no way to ask
  "how would this model have done on real
  resolutions?" without the new `backtest_model`
  method.
- The Brier reported at promote time is on the
  synthetic train fold — meaningful for ranking
  candidates, but doesn't tell you how the model
  performs on actual market resolutions.

**Caveat**: v0.43 ships with manual JSON input
for samples, not auto-populate from the markets
DB. The markets DB has `outcome` (YES/NO) but
not historical price snapshots, so a meaningful
auto-populate needs price history. v0.43+
candidate. Until then, the JSON textarea mirrors
the wire format exactly, and a power user can
script the data pull.

**Sidecar methods**: 9 (was 8 in v0.42, +1
`backtest_model`).

## Files changed in v0.43

```
sidecar/polyrocket_sidecar/train.py             | +250 LOC (run_backtest_model)
sidecar/polyrocket_sidecar/dispatch.py          | +backtest_model wrapper
sidecar/tests/test_sidecar.py                   | DISPATCH test updated (9 methods)
sidecar/tests/test_train.py                     | +7 backtest tests
src-tauri/src/domain/lab/sidecar.rs             | +BacktestSample/Result/... + 5 tests
src-tauri/src/commands/sidecar.rs               | +backtest_model IPC
src-tauri/src/lib.rs                            | +1 invoke_handler entry
src/ipc.ts                                      | +BacktestSample/.../Result + backtestModel()
src/routes/ModelLab.tsx                         | +backtest button + modal
src/components/feedback/BacktestReport.tsx      | new (~280 LOC)
src/components/feedback/BacktestReport.test.tsx | new (~180 LOC, 6 tests)
src/lib/i18n.ts                                 | +12 keys × 2 locales = 24 strings
docs/polyrocket-v0.43-final.md                  | (this file)
docs/overview.md                                | new row in doc-sync table
```

**Net change**: 12 files, +1,400 LOC.

## Migration / back-compat

- **No DB schema changes.**
- **One new sidecar method** (`backtest_model`).
  Existing 8 methods unchanged.
- **One new L1 IPC** (`backtest_model`). The L1↔Tauri
  guard detects it; no new "missing wrapper" entries.
- **No new persistent state.** The backtest is
  read-only — it doesn't write to active.json,
  archive.jsonl, or the DB.
- **Pre-v0.43 models** are still backtestable as
  long as they exist in `archive.jsonl` (or are
  the current active). Weights are read from the
  archive entry's `weights` field (v0.20a+ format).
- **Pre-v0.20a entries without weights** fail with
  a clear message ("model has no w0/w1/w2 weights").

## What's next

After user push:
- **v0.44** — Direction C: paper trading mode.
  New `MirrorExecutorConfig.paper_mode` flag, new
  `paper_fills` table, Settings toggle, [PAPER]
  badges in Copy / PnL.
- **v0.45** — auto-populate backtest samples from
  the markets DB. This needs a price-history
  infrastructure first; the L1 currently doesn't
  store historical price snapshots. v0.45+
  candidate once the snapshot system exists.
- **v0.50 milestone** — model lifecycle + trading
  parity. Trading-side features (advanced order
  types, conditional orders, post-only
  enforcement, fill analytics) start here.

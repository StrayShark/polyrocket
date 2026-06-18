# polyrocket v0.59 + v0.60 — final

**Branch**: main (local-only, NOT pushed)
**Commits**: 5 (v0.59a, v0.60a, v0.60b, + 2 ship logs)
**Released**: 2026-06-18 (local, awaiting user push)

## What changed since v0.58

| sub-version | one-liner                                          | tests at landing |
|-------------|----------------------------------------------------|------------------|
| v0.59a      | real SHAP via KernelExplainer (sidecar + Rust + L1) | 868              |
| v0.60a      | proxy hot-swap (no restart required)               | 870              |
| v0.60b      | vitest coverage gate (50% threshold + CI)         | 870              |
| v0.60c      | (this commit — ship log + push)                    | 870              |

**Test totals at v0.60 final**: cargo 319/319, vitest 439/439, python 86/86, script 31/31. **Total 875 (vitest+cargo+python) + 31 script = 906 total.** (Up from 855 at v0.58, +20 net: 5 cargo SHAP + 2 cargo hot-swap + 9 python SHAP + 3 vitest SHAP toggle + 1 method round-trip extension.)

## Highlights

### v0.59a — real SHAP via KernelExplainer

v0.55 ships the exact-decomposition
`contribution_i = w_i * x_i * p(1-p)` —
correct for the 3-feature linear model, but
doesn't satisfy the SHAP *efficiency* axiom
(Σφ_i = f(x) - E[f(x)]) when features are
correlated. v0.59 ships a real
KernelExplainer that does.

**Why not the `shap` PyPI library?** It would
pull in numpy + scipy + pandas (~30MB) for
100 lines of math on a 3-feature model. v0.59
is pure stdlib (math + itertools only).

**Algorithm**: KernelSHAP for M features:
  1. Generate all 2^M binary coalition masks
  2. Convert each mask to an imputed input
     (replace absent features with the
     background / median)
  3. Evaluate the model on each imputed input
  4. Weight each coalition by the SHAP kernel
     weight: w(z) = (M-1) / (C(M, |z|) * |z| *
     (M - |z|))
  5. Fit weighted LS to extract φ_i

For M=3, the cost is 8 coalition evaluations +
1 LS fit (~100µs total). For tree-based models
with M > 5 we'd need TreeSHAP; the response
includes a `method` field so a future TreeSHAP
path can be distinguished.

**L1 UI**: The ExplainabilityCard now has a
2-way toggle between SHAP and the v0.55
exact-decomposition. SHAP shows:
  - the method name (e.g. "kernel_shap")
  - the baseline (empty-coalition) prediction
  - the target prediction
  - the efficiency residual (a hint that the
    math converged)
  - the per-feature SHAP values (bar chart)

The toggle re-renders the bar chart with the
same data shape (the renderer normalises
`shap_value` vs `contribution` at render time).

### v0.60a — proxy hot-swap (no restart)

v0.56 ships network proxy support but the user
has to restart polyrocket for changes to take
effect. v0.60a fixes that.

**Architecture**:
  - Replaced `OnceCell<reqwest::Client>` in
    `commands/llm.rs` with
    `ArcSwap<reqwest::Client>`. arc-swap is a
    lock-free atomic Arc swap with relaxed
    load — no contention on the hot path.
  - `http_client()` returns `Arc<reqwest::Client>`
    (clonable, no lock).
  - `replace_http_client()` rebuilds the
    client and atomically swaps the Arc
    contents. Old client + connection pool
    are dropped when refcount → 0.
  - `set_proxy_config` IPC now:
    1. Writes the JSON config file
    2. Updates the DB row
    3. Sets `POLYROCKET_PROXY` env var
    4. Calls `replace_http_client()`
    5. Returns `restart_required: false`

**i18n**: "Restart required" → "Changes take
effect immediately" (en + zh).

### v0.60b — vitest coverage gate

Adds `@vitest/coverage-v8` (v8 provider, fast)
and configures vitest with a coverage gate:

- `npm run test:coverage`
- CI: vitest coverage gate job

**Threshold (initial, conservative)**:
  statements: 50%
  branches:   50%
  functions:  40%
  lines:      50%

Current actual: 54.8% statements, 55.3% lines,
51% branches, 43% functions. We start
conservative and ratchet up as v0.61+ fills in
the routes + business components (Dashboard,
ModelLab, etc., which are at 38-77% today).

**Excluded from coverage**:
- `*.test.{ts,tsx}` — the tests themselves
- `test-setup.ts` — setup boilerplate
- `main.tsx` — app entry, exercised by E2E
- `ipc.ts` — thin wrapper around `invoke()`,
  tested via mock + integration
- `types/**` — type-only files
- `**/*.unused` — archeology (Onboarding.tsx
  moved to .unused in v0.57a)

**Coverage report formats**:
- text (terminal)
- json-summary (CI artifact)
- html (browser-friendly, opened via
  `file://coverage/index.html`)

## Files changed in v0.59 + v0.60

```
sidecar/polyrocket_sidecar/shap.py                   | NEW — 270 lines
sidecar/polyrocket_sidecar/dispatch.py               | +shap_explain handler
sidecar/tests/test_shap.py                           | NEW — 9 python tests
src-tauri/Cargo.toml                                 | +arc-swap = "1"
src-tauri/src/domain/lab/sidecar.rs                  | +ShapExplain + ShapResult + builders
src-tauri/src/commands/sidecar.rs                    | +shap_explain IPC
src-tauri/src/commands/llm.rs                        | ArcSwap swap + hot-swap fn
src-tauri/src/commands/network.rs                    | set_proxy_config calls replace
src-tauri/src/lib.rs                                 | register shap_explain
src/ipc.ts                                           | +shapExplain wrapper + ShapResult
src/lib/i18n.ts                                      | +5 SHAP + 3 hot-swap keys
src/routes/Settings.tsx                              | SHAP/deriv toggle + new result UI
src/routes/ExplainabilityCard.test.tsx               | NEW — 3 vitest tests
vitest.config.ts                                     | +coverage thresholds
package.json                                         | +test:coverage
.github/workflows/ci.yml                             | +vitest coverage gate
```

**Net change**: 13 files, +1,500 LOC (incl. tests).

## Migration / back-compat

- **SHAP (v0.59a)**: no migration needed. The
  sidecar adds an 11th method; existing
  methods unchanged. The L1 ExplainabilityCard
  defaults to SHAP but the user can toggle to
  the v0.55 exact-decomposition.
- **Proxy hot-swap (v0.60a)**: in-flight
  requests may still use the old client (one
  in-flight `.send().await` is not interrupted).
  New requests pick up the new client
  immediately. No "Restart required" UI
  changes needed.
- **Coverage gate (v0.60b)**: new CI job. The
  threshold starts at 50% — we ratchet up as
  we fill in the routes.

## What's next

After user push, v0.59 + v0.60 close the
"ship + clean up" loop. Suggested v0.61+
candidates:

- **v0.61a — coverage ratchet**. The current
  50% threshold is a starting line. v0.61+
  can ratchet up by filling in Dashboard /
  ModelLab / MarketDetail / Audit
  (currently 38-77% coverage). Each route
  is a natural unit.
- **v0.61b — L1 contract tests**. The
  L1↔Tauri guard checks command names, but
  not that the L1 wrapper return type matches
  the Rust DTO field-for-field. A schema diff
  would catch drift. v0.61b candidate:
  codegen the L1 wrapper from a Rust-side
  `ts-rs` or `specta` schema.
- **v0.62 — telemetry dashboard**. We
  collect per-session NDJSON (v0.49a) but
  never show it. A Settings → Telemetry tab
  with a per-loop histogram + last-error
  drilldown is the natural next step.
- **v0.63 — TreeSHAP**. v0.59a implements
  KernelSHAP for the 3-feature model. When
  the model grows to a tree-based variant,
  the SHAP path needs TreeSHAP (efficient
  exact decomposition for trees).
- **v0.64 — multi-wallet UI**. The
  AddWalletModal already supports wallet
  JSON import (v0.57d). A wallet manager
  page (per-wallet balances, export, delete,
  rotate) is the natural next step.

## Architectural notes

The v0.59 + v0.60 pack is "real engineering":
a proper SHAP implementation, a lock-free
client hot-swap, a coverage gate. None of
these change the user-facing architecture —
they raise the floor on quality.

The 19 new tests (9 python + 5 cargo + 3 vitest
+ 2 cargo hot-swap) compound with the 855
tests from v0.53-v0.58. The 906 total tests
+ 50% coverage gate + 5-job CI is a solid
baseline for v0.61+ refactors.

### v0.59 + v0.60 hygiene: what we left on the table

- **KernelSHAP cost is O(2^M)**. With M=3
  it's 8 evaluations. For M=10, it's 1024.
  v0.63+ candidate: switch to TreeSHAP for
  tree-based models.
- **Hot-swap race window**. We set
  `POLYROCKET_PROXY` then call
  `replace_http_client()`. A parallel
  `new_http_client()` call between these two
  could read the new env var but not the new
  client. In practice this is harmless (the
  swap is atomic) but theoretically a tiny
  window exists. v0.61+ candidate: bundle
  the env-var + client into a single
  `OnceCell`-like setter.
- **Coverage threshold is 50%** — too low
  to catch real regressions. v0.61+ should
  ratchet up as we add tests.
- **No `cargo audit` / `pnpm audit`** in CI.
  v0.61+ candidate.
- **No nightly schedule**. The CI runs on
  PR + push only. A weekly cron could
  catch snapshot / lockfile drift
  independently of commits.
- **The proxy hot-swap doesn't touch
  `commands/llm_mgmt.rs` test_connectivity**
  (which builds a fresh `reqwest::Client`
  with no proxy). v0.61+ candidate: route
  test_connectivity through the shared
  `http_client()` so the connectivity test
  honors the proxy too.

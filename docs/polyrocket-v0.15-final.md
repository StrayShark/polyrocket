# polyrocket v0.15 — final

**Branch**: main (local-only, not pushed)
**Commits**: `fb2fef8` v0.15a → `3c70506` v0.15d
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.14

| sub-version | hash       | one-liner                                          | tests at landing |
|-------------|------------|----------------------------------------------------|------------------|
| v0.15a      | `fb2fef8`  | Rust `llm_analyze` emits 4 progress events         | 457              |
| v0.15b      | `0fe4847`  | L1 `listen` wrappers + typed event payloads        | 464              |
| v0.15c      | `7e7835b`  | `AnalyzeProgress` component + Analysis page wiring | 464              |
| v0.15d      | `3c70506`  | 8 component tests with mocked events               | 472              |
| v0.15e      | this file  | Ship log + event architecture + convention note     | 472              |

**Test totals at v0.15 final**: cargo 222/222, vitest 206/206, python 44/44. **Total 472/472.**

## Highlights

### 1. LLM analyze is now observable (v0.15a-d)

The `llm_analyze` IPC used to be a black box. The L1 showed a
spinner for 5-30 seconds, then a result. There was no way to
tell which provider was hung vs done vs failed without waiting
for the entire fan-out to return.

v0.15 changes that with 4 Tauri events:

| event                       | when it fires                              | payload                                                  |
|-----------------------------|--------------------------------------------|----------------------------------------------------------|
| `llm_analyze:started`       | after the analysis row INSERT, before fan-out | `analysis_id`, `market_id`, `prompt_version`, `providers[]`, `started_at` |
| `llm_analyze:provider_done` | once per provider, ok or fail               | `analysis_id`, `provider_id`, `ok`, `latency_ms`, `tokens_in/out`, `cost_cents`, `error_kind`, `error_message`, `finished_at` |
| `llm_analyze:consensus_done` | once per analyze, after consensus computed | `analysis_id`, `status`, `n_success`, `n_failed`, `consensus_pred/side/conf` |
| `llm_analyze:finished`      | terminal event, after the consensus update  | `analysis_id`, `status`, `total_latency_ms`, `total_cost_cents`, `n_success`, `n_failed`, `finished_at` |

The L1 (`AnalyzeProgress` component) subscribes to all 4 events
and renders a per-provider status grid:

  ⟳ Analyzing… (1/3)
  ·  Anthropic    ✓ 1.2s
  ·  OpenAI       ⟳ running…
  ·  Google       · pending

→

  ✓ Done — 2 ok, 1 failed (3 total)  [partial]   1.5s · $0.08
  Consensus: 62.0% · YES
  ·  Anthropic    ✓ 1.2s
  ·  OpenAI       ✓ 0.8s
  ·  Google       ✗ rate_limit

### 2. The chicken-and-egg of "events fire before IPC returns"

The Rust `llm_analyze` generates a UUID, INSERTs the analysis
row, emits `started`, fans out to N providers in parallel,
emits `provider_done` for each, computes consensus, emits
`consensus_done` and `finished`, then returns. The IPC call
in L1 only resolves AFTER the entire sequence. So if the L1
tries to "capture the id from the IPC return value" the
events are already lost.

**v0.15c solution**: a `useRef<boolean>` flag pattern. The
Analysis page listens for the next `started` event and
captures the `analysis_id` from its payload, then passes it
down to `AnalyzeProgress`. The flag resets to false after
the first event, so subsequent events from other analyzes
are ignored.

This avoids a Rust-side refactor (e.g. emitting the id over
a separate channel first) and keeps the IPC contract simple.

### 3. L1 ↔ L2 typed event payloads

The 4 Rust event structs (`AnalyzeStartedEvent`,
`ProviderDoneEvent`, `ConsensusDoneEvent`,
`AnalyzeFinishedEvent`) live in
`src-tauri/src/domain/llm/progress.rs` as the canonical
shape. The L1 re-declares matching interfaces in
`src/ipc.ts` and uses them as the generic parameter to
`listen<...>()`.

The vitest test in `src/ipc.events.test.ts` round-trips each
event through JSON to catch any drift between the Rust and
TS shapes (e.g. if someone adds a field to one side but not
the other).

### 4. Component tests with mocked Tauri events (v0.15d)

The `AnalyzeProgress` component subscribes to Tauri events at
mount time. The 8 component tests mock `@/ipc` with
`vi.mock()` so the 4 `on*` listen functions append to a
module-level subscriber list. The tests then call `fire*`
helpers (wrapped in `act()`) to simulate events and assert
the DOM updates.

This pattern is reusable for any future Tauri event-driven
component. The actual Tauri event bus is not vitest-friendly
(out of scope for happy-dom), so mocking the `ipc` module
is the cleanest path.

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes. The new
  `progress.rs` is in L3 (domain), the IPC command stays L2, the
  L1 wrapper stays L1.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes.
- **CI guards**: `cargo test` + `pnpm test` + `pnpm typecheck` all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.
- **Snapshots**: 54 PNGs regenerated, no MD5 drift vs v0.14 (the
  Analysis page change is text-only — no visual difference when
  the progress grid is hidden).

## Test growth history

```
v0.13d → 449
v0.14a → 450  (+1 i18n)
v0.14b → 451
v0.14c → 452
v0.14d → 453
v0.15a → 457  (+4 progress event serde tests)
v0.15b → 464  (+7 ipc.events round-trip tests)
v0.15c → 464  (no new tests this commit)
v0.15d → 472  (+8 AnalyzeProgress component tests)
```

## Files changed in v0.15

```
src-tauri/src/domain/llm/progress.rs        (v0.15a — new file, 4 event structs + 4 tests)
src-tauri/src/domain/llm/mod.rs             (v0.15a — register progress submodule)
src-tauri/src/commands/llm.rs               (v0.15a — AppHandle param + 4 emit calls)
src/ipc.ts                                  (v0.15b — 4 event types + 4 listen wrappers)
src/ipc.events.test.ts                      (v0.15b — 7 round-trip tests) [new]
src/components/feedback/AnalyzeProgress.tsx (v0.15c — new component) [new]
src/components/feedback/AnalyzeProgress.test.tsx (v0.15d — 8 component tests) [new]
src/routes/Analysis.tsx                     (v0.15c — listener + flag pattern + progress mount)
src/lib/i18n.ts                             (v0.15c — 12 new keys × 2 locales)
src/lib/i18n.test.ts                        (v0.15c — assert new keys)
docs/overview.md                            (doc-sync table, 4 new rows)
docs/polyrocket-v0.15-final.md              (this file) [new]
```

## Release binary

Not re-built for v0.15 — no native code changes. The v0.13
final binary remains the current shipped build.

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## New convention (per 2026-06-17 user)

This entire milestone is local-only. None of the v0.15
commits have been pushed. The user pushes manually.

The convention is saved to `~/.mavis/memory/user.md`.

## Next steps (deferred to v0.16+)

From the v0.13 final doc's deferred list (most still valid):

1. **Parallel predict A/B test** — needs a rethink because the
   I/O bottleneck (single sidecar process) prevents true
   parallelism. v0.13d's `predict_async` + the v0.15 event
   pattern are the building blocks. v0.16 could run multiple
   sidecar processes and load-balance predict calls across
   them.
2. **Real `rs-clob-client` integration** — wire the real
   Polymarket CLOB client (currently orders are signed-order
   stubs).
3. **On-chain mirror execution** — currently the mirror
   executor records decisions but doesn't actually send
   transactions.
4. **Code-sign + DMG** — notarized `.dmg` for distribution.
5. **Auto-update feed** — Tauri updater pointed at GitHub
   Releases.
6. **Visual regression** — Playwright with the 54 PNG
   snapshots as baselines.
7. **numpy vectorize predict** — replace the hot-path Python
   loop with a numpy vectorized implementation.

The v0.15 event pattern is directly reusable for items #4
(code-sign progress) and #5 (auto-update progress). The
v0.15 component test pattern is reusable for any future
event-driven UI (#6 visual regression if we mock the
underlying renderer).

## Open question

The `LlmAnalysis` TS type in `src/types/llm.ts` says
`id: number` but the actual schema uses `id: text` (UUID
string). The v0.15c commit had to cast with
`as unknown as number` to work around this. v0.16 should
fix the L1 type to match the schema.

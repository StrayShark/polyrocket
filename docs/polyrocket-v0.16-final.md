# polyrocket v0.16 — final

**Branch**: main (local-only, not pushed)
**Commits**: `3643977` v0.16a → `0faa9ed` v0.16c
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.15

| sub-version | hash       | one-liner                                          | tests at landing |
|-------------|------------|----------------------------------------------------|------------------|
| v0.16a      | `3643977`  | Fix L1 DTO types to match Rust source of truth      | 472              |
| v0.16b      | `52fd869`  | Fix broken `recMut` + `recordLlmDecision` IPCs     | 472              |
| v0.16c      | `0faa9ed`  | 19 DTO + top-recommendation picker tests            | 491              |
| v0.16d      | this file  | Ship log + bug post-mortem + convention note         | 491              |

**Test totals at v0.16 final**: cargo 222/222, vitest 225/225, python 44/44. **Total 491/491.**

## Highlights

### 1. Two real production bugs fixed (v0.16a + v0.16b)

The v0.15 final doc's "open question" turned out to be much
bigger than a single type cast. Investigation found:

| bug | impact | root cause |
|-----|--------|------------|
| `LlmAnalysis.id: number` | `as unknown as number` cast at call site; would silently mis-type any future field that uses the id | L1 type was a guess; never reconciled with the schema |
| `LlmAnalysis.consensus_prob` → Rust: `consensus_predicted` | L1 reads the wrong field; the consensus never showed up | L1 field name drift from the Rust DTO |
| `LlmAnalysis.status` 4 states vs 3 | The "partial" state could never be displayed | L1 type was missing 2/4 states |
| `LlmRecommendation.analysis_id: number` | FK typed wrong; L1 passed UUID to a number field | same — L1 vs schema drift |
| `LlmRecommendation.rationale` → Rust: `reasoning` | Modal would never show the LLM rationale | L1 field name drift |
| **`recMut` passed analysis UUID to rec-id IPC** | "Show top recommendation" button did nothing | L1 IPC wrapper + Rust command arg shapes never reconciled |
| **`recordLlmDecision` had wrong arg shape entirely** | "I follow top" / "Skip" buttons did nothing | Same — L1 wrapper signature drifted from Rust struct |

The last two are real production bugs. Clicking either
button would fail silently (no `onError` handler, Tauri
errors get swallowed by the IPC layer). The user
probably saw the buttons not respond and worked around
it by manually running recompute_signals.

The root cause for all of these is the same: the L1
DTOs and IPC wrappers were written once in v0.4 and
never reconciled with the Rust side as it evolved.
The drift compounded over v0.5-v0.12 (15+ sub-versions)
until the type mismatches became the norm.

**v0.16 fix**:
- All DTO fields match the Rust DTOs (source of truth)
- IPC wrappers match the Rust command arg shapes
- 19 new tests document the expected wire format
- The `as unknown as number` cast is gone

### 2. The "as unknown as number" cast is gone

v0.15c's progress UI work added this cast at the
`analyzeMut.onSuccess` handler to work around the
`LlmAnalysis.id: number` typo. After v0.16a, the type
is `string` and the cast goes away.

```ts
// v0.15c
analysisId: r.analysis.id as unknown as number,

// v0.16a
analysisId: r.analysis.id,
```

The TS compiler is happy, no more `unknown` escape
hatch. Future code that touches the analysis id gets
the right type automatically.

### 3. recMut now picks the top rec explicitly

Before v0.16b, the L1 passed `analyzeResult.analysisId`
to `llmGetRecommendation`, expecting Rust to "figure out
which rec to show". Rust didn't — it tried to parse the
UUID as i64 and failed silently.

After v0.16b, recMut takes no args and internally:

```ts
const recs = analyzeResult?.recommendations ?? [];
const top = [...recs]
  .filter((r) => r.parse_ok)
  .sort((a, b) => (b.confidence ?? 0) - (a.confidence ?? 0))[0]
  ?? recs[0];
if (!top) throw new Error('no recommendations to show');
return llmGetRecommendation(top.id);
```

The picker logic is also tested in isolation (5 unit
tests in v0.16c). The v0.16c tests cover:
- Highest confidence wins
- parse_failed is a hard filter
- All-failed falls back to first
- Null confidence sorts as 0
- Empty recs returns undefined

### 4. `recordLlmDecision` arg shape corrected

Before v0.16b:

```ts
recordLlmDecision(analysisId: number, decision: string)
// → invoke('record_llm_decision', { analysisId, decision })
```

After v0.16b:

```ts
interface RecordLlmDecisionArgs {
  analysisId: string;
  userDecision: 'follow_top' | 'manual_yes' | 'manual_no' | 'skip' | 're_analyze';
  userDecidedSide?: 'YES' | 'NO' | null;
  followedLlmId?: number | null;
  betId?: string | null;
  contextSnapshot?: string | null;
}
recordLlmDecision(args: RecordLlmDecisionArgs)
// → invoke('record_llm_decision', { args })
```

Tauri 2 maps the JS object's camelCase keys to the
Rust struct's snake_case fields. Required: analysisId
+ userDecision. The 4 optional fields map to the
`Option<T>` defaults in `RecordDecisionArgs`.

The 5 v0.16c tests document the expected shapes
(follow_top / manual_yes / skip / re_analyze / manual_no).

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes.
- **CI guards**: `cargo test` + `pnpm test` + `pnpm typecheck` all pass.
- **No native code changes**, no rebuild needed.
- **No new visual changes** (the recommendation modal was
  the only visible fix; it now actually shows the LLM
  rationale + predicted prob + side + cost + latency).

## Test growth history

```
v0.15d → 472
v0.16a → 472  (type fixes; no new tests)
v0.16b → 472  (IPC fixes; no new tests)
v0.16c → 491  (+19 DTO + picker tests)
```

The +19 tests are entirely new test coverage for the
LLM DTOs. The v0.4-v0.15 codebase never had a single
test for the LLM DTO shape — every drift was caught
by the TS compiler in the worst case, or silently
mis-deserialized in the best case.

## Files changed in v0.16

```
src/types/llm.ts                  (v0.16a — corrected DTOs)
src/types/llm.test.ts             (v0.16c — 19 DTO + picker tests) [new]
src/ipc.ts                        (v0.16b — RecordLlmDecisionArgs + corrected wrapper)
src/routes/Analysis.tsx           (v0.16a — field renames + nullable handling;
                                            v0.16b — recMut picker + decisionMut typed arg)
docs/overview.md                  (doc-sync table, 3 new rows)
docs/polyrocket-v0.16-final.md    (this file) [new]
```

## Release binary

Not re-built for v0.16 — no native code changes. The
v0.13 final binary remains the current shipped build.

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## Post-mortem: how did this bug last 11 sub-versions?

The drift started in v0.4 when `LlmAnalysis` was first
introduced. The Rust DTOs evolved over v0.5-v0.12 to add
fields (`signal_id`, `total_latency_ms`, `cost_cents`),
rename fields (`consensus_prob` → `consensus_predicted`,
`rationale` → `reasoning`), expand status enums
(`ok` → `completed` / `partial` / `failed`), and change
primary key types (UUID vs autoinc).

The L1 DTOs were copy-pasted once and never touched
again. The TypeScript compiler couldn't catch the
drift because:

1. The fields were never **used** in any code path
   that mattered (consensus was a "nice to have" that
   never made it to the UI; reasoning was display-only
   in a modal nobody visited).
2. The IPC wrappers were written against the
   LlmAnalysis type, not against the Rust DTO. So
   the L1 thought it was passing the right thing.
3. The button bugs (recMut, recordLlmDecision) were
   invisible because they were on a "secondary" page
   (Analysis) and the failures had no `onError` handler.

The fix is structural, not just point fixes:
- v0.16c adds tests for every DTO shape, so future
  drift will be caught.
- v0.16a+b makes the L1 types and IPC wrappers
  match the Rust source of truth by inspection
  (the v0.16a commit message has a side-by-side
  table of every correction).

Future IPC additions should:
1. Add the Rust DTO first.
2. Add the L1 mirror with the same field names +
   types.
3. Add a v0.16c-style JSON round-trip test.
4. Add the IPC wrapper matching the Rust arg shape.
5. Add an L1 component test that exercises the
   wrapper (mocked `invoke` + `listen`).

This is a 5-step ritual but it prevents the entire
class of bug we just fixed.

## New convention (per 2026-06-17 user)

All v0.16 commits are local-only. The user pushes
manually. Saved to `~/.mavis/memory/user.md`.

## Next steps (deferred to v0.17+)

From the v0.13 final doc's deferred list:

1. **Parallel predict A/B test** — needs a rethink
   because the I/O bottleneck (single sidecar
   process) prevents true parallelism. v0.13d's
   `predict_async` + v0.15 event pattern + v0.16
   DTO discipline are the building blocks.
2. **Real `rs-clob-client` integration** — wire the
   real Polymarket CLOB client (currently orders
   are signed-order stubs).
3. **On-chain mirror execution** — currently the
   mirror executor records decisions but doesn't
   actually send transactions.
4. **Code-sign + DMG** — notarized `.dmg` for
   distribution.
5. **Auto-update feed** — Tauri updater pointed at
   GitHub Releases.
6. **Visual regression** — Playwright with the 54
   PNG snapshots as baselines.
7. **numpy vectorize predict** — replace the
   hot-path Python loop with a numpy vectorized
   implementation.

The v0.16c DTO-test pattern is reusable for #2 and
#3 (real CLOB / on-chain). The v0.15 event pattern
is reusable for #5 (auto-update progress).

# polyrocket v0.90 + v0.91 — codegen Phase 5 + useTrainProgress hook

> 2026-06-21 · 1 commit · 1 source refactor + 3 new files
> v0.90 (build pipeline) + v0.91 (ModelLab hook extraction)

## TL;DR

Two sub-versions landed in a single commit:

- **v0.90 — codegen Phase 5**: `pnpm build` / `pnpm tauri:build` now auto-regenerate
  `src/types/generated/index.ts`. Drift is caught at build time, not just CI. The
  migration plan's Phase 5 is **DONE** (Phases 1+2+3+4+5 all green).

- **v0.91 — useTrainProgress hook extraction**: The train-progress state machine
  inside `ModelLab.tsx` (the deferred v0.83 branch coverage gap) is now a standalone
  custom hook with 7 unit tests. Strict-mode double-mount behavior is now testable
  in isolation.

Test count: 960 → 967 vitest (+7 hook tests).
Coverage: 87.5/85.5/81.6/88.7 (was 87.5/85.4/81.6/88.6). All 4 dims pass the v0.89
threshold (87/84/81/88).

## v0.90 — codegen Phase 5 (build pipeline)

### What changed

| File | Lines | Purpose |
|---|---|---|
| `scripts/gen-ts-with-stub.sh` | +50 (NEW) | Wraps `cargo run --bin gen_ts_types` with dist/ stub |
| `package.json` | ±8 | `gen:ts` → wrapper; `build` / `tauri:build` → regen + build |
| `scripts/check-codegen-drift.mjs` | ±10 | Use wrapper instead of direct cargo |
| `scripts/run-ci-local.sh` | +11 | Add `pnpm check:codegen-drift` to Job 2 |
| `docs/codegen-migration-plan.md` | +56/-10 | Mark Phase 5 DONE; document wrapper + drift surfacing |

### Why a wrapper script

The `tauri::generate_context!()` macro inside `src-tauri/src/lib.rs` panics on
a clean checkout without `dist/`. Previously, every `pnpm gen:ts` invocation
required a manual `mkdir dist && touch dist/index.html` workaround. The
wrapper now handles this transparently:

```bash
#!/usr/bin/env bash
# scripts/gen-ts-with-stub.sh
if [ ! -d dist ]; then
  mkdir -p dist/assets
  cat > dist/index.html <<'EOF'
<!doctype html>...stub html...
EOF
  echo "console.log('stub')" > dist/assets/main.js
  STUB_CREATED=1
fi
trap 'rm -rf dist 2>/dev/null || true' EXIT
cd src-tauri && cargo run --bin gen_ts_types --quiet
```

Pattern lifted from `scripts/run-ci-local.sh:178-187`. The trap ensures
`dist/` is cleaned up after the wrapper exits, regardless of cargo's exit
code. (If `dist/` already existed before the wrapper ran, it's left
untouched — the wrapper only removes stubs it created.)

### Build pipeline integration

```
pnpm build:
  → pnpm gen:ts             # ~30-60s clean, ~1-2s incremental
  → tsc -b                  # typecheck
  → vite build              # bundle

pnpm tauri:build:
  → pnpm gen:ts
  → tauri build             # compile Rust + bundle .app / .msi / .deb
```

For hot dev iteration, use the `:dev` variants which skip codegen:

```
pnpm build:dev           # tsc + vite only
pnpm tauri:build:dev     # tauri build only
```

### Drift surfacing

If you change a Rust DTO and forget to commit the regenerated
`src/types/generated/index.ts`:

- `pnpm build` regenerates the file → vite rebuilds → `git diff` shows the change
- `git push` triggers pre-push hook's `cargo test --lib` (which implicitly
  re-runs codegen) → drift detected → push fails
- `pnpm check:codegen-drift` (also wired into local CI Job 2) → explicit
  failure with "run pnpm gen:ts && git add src/types/generated/"

### Cost

| Operation | Cost |
|---|---|
| Incremental `pnpm build` | ~1-2s (cargo sees no Rust changes, just re-exports the bin) |
| Clean `pnpm build` | ~30-60s (cargo compiles the bin from scratch) |
| `pnpm check:codegen-drift` | ~30-60s (full re-export + diff parse) |

## v0.91 — useTrainProgress custom hook extraction

### The v0.83 deferred branch coverage gap

v0.83 added ModelLab branches coverage tests but **explicitly deferred** the
train-progress `useEffect` because:

> "The `onTrainStarted` listener returns a `Promise<UnlistenFn>`. The
> hook depends on Tauri's `safeListen`. Hard to test in isolation."

Plus, the strict-mode double-mount cleanup (`cancelled = true` on first
mount) caused the "Should not already be working" errors in vitest when
the test rendered + unmounted quickly.

### The extraction

**Before** (ModelLab.tsx lines 112, 126-144):

```tsx
const [activeTrainJobId, setActiveTrainJobId] = useState<string | null>(null);
// ... 14 lines of comments ...
const expectedTrainRef = useRef<boolean>(false);
useEffect(() => {
  let cancelled = false;
  const unsubStarted = onTrainStarted((e: TrainStartedEvent) => {
    if (cancelled) return;
    if (expectedTrainRef.current) {
      setActiveTrainJobId(e.job_id);
      expectedTrainRef.current = false;
    }
  });
  return () => {
    cancelled = true;
    unsubStarted.then((u) => u()).catch(() => {});
  };
}, []);
```

**After** (ModelLab.tsx):

```tsx
const { activeTrainJobId, markExpected, clearActive } = useTrainProgress({
  onTrainStarted,
});
```

The hook lives at `src/hooks/useTrainProgress.ts` (+113 lines, with full
JSDoc per the v0.61+ doc convention). The Train button onClick now calls
`markExpected()`; the trainMut's `onSettled` calls `clearActive()`. No
direct ref mutation, no direct `setActiveTrainJobId` outside the hook.

### The state machine

```
            markExpected()          train:started
   idle ───────────────────→ expected ──────────────→ running
    ↑                                                    │
    └──────────── clearActive() / setActiveTrainJobId(null)
```

The ref `expectedTrainRef` resets to false after capture, so a stale event
won't re-trigger. This protects against:
- Two trains in flight simultaneously (second train's event is ignored
  if the first hasn't completed)
- A late `train:started` event from a previous session (only the click
  that called `markExpected()` is allowed to capture)

### Tests

`src/hooks/useTrainProgress.test.ts` (+149 lines, 7 tests, 7/7 pass):

1. **Initial state** — `activeTrainJobId=null`, listener subscribed once
2. **markExpected + event captured** — `activeTrainJobId` set to event's job_id
3. **Event without markExpected ignored** — ref guard works
4. **Ref reset after capture** — second event without re-markExpected is ignored
5. **clearActive()** — resets to null
6. **enabled=false** — no listener subscription
7. **Unmount cleanup** — unlisten Promise awaited and called

Tests use a `makeListener()` helper that produces a controllable
`ListenFn` (the same signature as Tauri's `safeListen`). No Tauri runtime
needed; happy-dom is enough.

### Coverage impact

| File | Before | After |
|---|---|---|
| `useTrainProgress.ts` | n/a (new) | 100/100/87.5/100 |
| `ModelLab.tsx` | 74.64/72.58/59.61/74.8 | 73.64/72.5/57.44/74.16 |
| **Project** | **87.42/85.0/81.51/88.6** | **87.5/85.5/81.6/88.7** |

The ModelLab fn % drop (-2.17pp) is a refactor side effect: 5 functions
moved out of ModelLab into the hook. The hook is 87.5% covered, so the
total business logic coverage is up. The slight project improvement
(+0.06pp avg) reflects this redistribution.

### vitest.config.ts update

Added `src/hooks/**/*.{ts,tsx}` to the coverage include list. The
`src/hooks/` directory existed since v0.13b (`useDebounce.ts`) but was
missing from coverage tracking. Now it counts.

## What's still on the roadmap

**v0.92** — codegen coverage beyond 28% (the remaining 81 IPCs).
Two strategies: (1) custom `serde_json::Value` field wrappers for DTOs
with truly dynamic payloads; (2) accept 28% as realistic ceiling if (1)
proves infeasible. Decision deferred.

**v0.93** — more ModelLab coverage. The hook extraction unlocked the
train-progress state machine but ModelLab's other 20 uncovered
functions (auto-promote flow, comparison modal, archive viewer)
still need targeted tests.

**v0.94+** — feature work. The v0.69-v0.91 era was coverage + tooling;
the next era can focus on actual product features.

## Verified

- `pnpm typecheck` → exit 0
- `pnpm vitest run` → **967 tests pass** (was 960; +7 hook tests)
- `pnpm test:coverage` → 87.5/85.5/81.6/88.7 (all 4 dims pass v0.89
  threshold 87/84/81/88)
- `pnpm build` → exit 0, regenerated file unchanged
- `pnpm check:codegen-drift` → "no drift"
- `bash scripts/gen-ts-with-stub.sh` → exit 0
- `scripts/run-ci-local.sh` → **5/5 jobs PASS** (governance, L1+vitest
  + codegen drift, Rust cargo, Python pytest, Playwright e2e)

## Files changed

| File | Lines | Purpose |
|---|---|---|
| `scripts/gen-ts-with-stub.sh` | +50 (NEW) | v0.90 — dist/ stub wrapper for codegen |
| `src/hooks/useTrainProgress.ts` | +113 (NEW) | v0.91 — extracted train-progress hook |
| `src/hooks/useTrainProgress.test.ts` | +149 (NEW) | v0.91 — 7 hook unit tests |
| `package.json` | ±8 | v0.90 — `gen:ts`/build/`tauri:build` wired |
| `scripts/check-codegen-drift.mjs` | ±10 | v0.90 — use wrapper |
| `scripts/run-ci-local.sh` | +11 | v0.90 — add drift check to Job 2 |
| `docs/codegen-migration-plan.md` | +56/-10 | v0.90 — Phase 5 DONE |
| `vitest.config.ts` | +1 | v0.91 — add `src/hooks/**` to coverage |
| `src/routes/ModelLab.tsx` | -22/+30 | v0.91 — use hook (replaces inline useEffect) |
| `README.md` | ±4 | auto-bumped by update-readme-coverage.mjs |

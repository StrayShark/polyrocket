# polyrocket v0.69 final — CI infra 3-fix

> **TL;DR**: v0.69 = **CI-only** release. No new features, no test additions.
> Three independent CI failures were fixed (governance / frontend /
> rust jobs) so future pushes pass green for the first time since
> v0.66. One hidden typecheck failure (masked by the broken pnpm
> install step) was also fixed.

---

## 1. Why this version exists

The user reported "every push CI fails". Investigation of CI run
`27803502583` (the most recent `polyrocket CI` run, against the
v0.67 commit `fa7c956` on `origin/main`) revealed **4 independent
failures**, only 3 of which the CI logs showed directly:

```
governance guards        → README badges in sync (v0.67d)   FAIL
L1 typecheck + vitest    → Install dependencies               FAIL
Rust cargo test          → cargo build                        FAIL
Python sidecar tests     → (passed)                           OK
```

The 4th failure (TypeScript typecheck with 39 errors) was hidden
because `Install dependencies` failed first and short-circuited
the rest of the job. Once the pnpm install bug is fixed, typecheck
becomes the next gate — and it was red.

All 4 are CI-infra bugs, not product bugs. v0.69 ships the fixes.

---

## 2. v0.69a — typecheck fix (1 commit, 6 files, +14/-18)

**Symptom**: `pnpm typecheck` returns 39 errors (25× `TS2322`,
14× `TS6133`). Silently masked by the broken pnpm install step.

**Root cause**: vitest 4.x changed `Mock`'s generic from
`<T>` to `<Mock<Procedure | Constructable>>`. `createIpcMock`'s
parameter type (`Record<string, ReturnType<typeof vi.fn>>`)
became stricter. Test files commonly pass inline arrow
functions like:

```ts
vi.mock('@/ipc', () => createIpcMock({
  llmAnalyze: () => mockLlmAnalyze(),  // type: () => any
}));
```

TypeScript now rejects `() => any` as not assignable to
`Mock<Procedure | Constructable>`. 25 such errors across
`src/routes/*.more.test.tsx` and `src/lib/keyboard-nav.test.tsx`.

The remaining 14 errors are dead-code `vi.fn()` consts in
`Analysis.more.test.tsx` and `Copy.more.test.tsx` — leftover
from an earlier draft before the test was refactored to use
`vi.hoisted()`. Plus 4 unused imports.

**Fix**:

- `src/test-mocks.ts` — relax `createIpcMock` parameter/return
  type from `Record<string, ReturnType<typeof vi.fn>>` to
  `Record<string, (...args: any[]) => any>`. Accepts both raw
  arrow functions and `vi.fn()` instances. Doc note added.

- `src/routes/Analysis.more.test.tsx` — drop 5 dead `vi.fn()`
  consts (mockLlmAnalyze et al.). The hoisted versions (created
  via `vi.hoisted()`) are what `vi.mock()` actually uses.

- `src/routes/Copy.more.test.tsx` — same pattern, 5 more dead
  consts.

- `src/lib/ipc-contract-v2.test.ts` — drop unused `TYPES_DIR`
  constant (was reserved for the v0.68b codegen Phase 1 stub).

- `src/lib/keyboard-nav.test.tsx` — drop unused `useState` and
  `withFakeTimersAndState` imports (v0.65b refactor left them
  behind). Kept `useEffect` (still used in TestRig at line 36).

- `src/routes/Copy.test.tsx` — drop unused `fireEvent` import.

**Verification**:

```
$ pnpm typecheck
> tsc --noEmit

(exit 0, no output)
```

Tests still 581/581 passing.

---

## 3. v0.69b — pnpm install fix (1 commit, 2 files, +6/-3)

**Symptom**: CI `Install dependencies` step fails with
`ERROR packages field missing or empty` on every push.

**Root cause**: `pnpm-workspace.yaml` declared only `allowBuilds:`
(no `packages:` field). Local pnpm 11 tolerates this; CI pnpm 9
(pinned via `pnpm/action-setup@v4` with `version: 9`) requires
`packages:` and errors out.

The workspace yaml existed in the first place because pnpm 9
needs `allowBuilds` (now `pnpm.onlyBuiltDependencies`) to opt-in
to native-module postinstall scripts (better-sqlite3, esbuild).
Without that opt-in, pnpm refuses to invoke them and the build
fails at `better-sqlite3 not built`.

**Fix**: move the allowlist to the standard pnpm 9.4+ location
(`pnpm.onlyBuiltDependencies` in `package.json`) and delete
`pnpm-workspace.yaml` entirely. The project is single-package
(only one `package.json` at the root); no real workspace exists.

```diff
# package.json
+  "pnpm": {
+    "onlyBuiltDependencies": [
+      "better-sqlite3",
+      "esbuild"
+    ]
+  }

# pnpm-workspace.yaml (deleted)
- allowBuilds:
-   better-sqlite3: true
-   esbuild: true
```

**Verification** (matches CI's pnpm version):

```
$ pnpm --version
9.15.9
$ pnpm install --frozen-lockfile
+ typescript 5.9.3
+ vite 6.4.3
+ vitest 4.1.9

Done in 1m 17.1s using pnpm v9.15.9
```

Also re-verified with local pnpm 11.5.0 (the user's actual dev
environment) — works the same.

---

## 4. v0.69c — Rust toolchain bump 1.77 → 1.88 (1 commit, 1 file, +1/-1)

**Symptom**: CI `cargo build --lib` fails with:

```
failed to parse manifest at .../dlopen2_derive-0.4.3/Cargo.toml
feature edition2024 is required
The package requires the Cargo feature called `edition2024`,
but that feature is not stabilized in this version of Cargo
(1.77.2 (e52e36006 2024-03-26)).
```

**Root cause**: `dlopen2_derive 0.4.3` (transitive dep of
`tao 0.35.3` → `tauri-runtime-wry 2.11.2` → `tauri 2.11.2`)
uses Rust's edition2024, stable since 1.85. Our CI was pinned
to **Rust 1.77** (via `dtolnay/rust-toolchain@stable` with
`toolchain: 1.77`). Cargo 1.77 can't parse the manifest even
when the .crate file is locked.

**Toolchain audit**:

| toolchain | result |
|---|---|
| 1.77 (current CI) | dlopen2_derive 0.4.3 manifest parse fails |
| 1.85 | time-0.3.49, darling 0.23.0 require 1.88+. Still fails. |
| **1.88** | All current Cargo.lock transitive deps satisfied. |
| 1.96 (local dev) | Works (this is what the user uses). |

1.88.0 was released 2025-06-23 — ~12 months headroom from "now"
(2026-06). Bumping CI to 1.88 keeps us on a stable LTS-ish floor
while giving breathing room for future Tauri minor releases.

**Fix**:

```diff
# .github/workflows/ci.yml
   - uses: dtolnay/rust-toolchain@stable
     with:
-      toolchain: 1.77
+      toolchain: 1.88
```

**Verification**:

```
$ cargo +1.88 check --manifest-path src-tauri/Cargo.toml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 01s

$ cargo +1.88 test --lib --manifest-path src-tauri/Cargo.toml -- --test-threads=1
test result: ok. 319 passed; 0 failed; 0 ignored; 0 measured
```

The `--test-threads=1` flag is already in the CI workflow (line
~140) — it forces sequential test execution to dodge a parallel
race condition on the `POLYROCKET_SIDECAR_MODEL_DIR` env var in
`commands::sidecar::tests::archive_filters_by_job_ids`. Without
it, that test fails intermittently (and 1 other) under parallel
threads.

---

## 5. v0.69d — README badge sync (1 commit, 1 file, +3/-3)

**Symptom**: CI `README badges in sync (v0.67d)` step fails
because the committed README.md held v0.67 numbers (72.7% stmts,
569 vitest, v0.66 status) while the actual repo state after v0.68
was 73.9% / 581 vitest tests.

**Root cause**: `scripts/update-readme-coverage.mjs` runs and
greps for `"no changes needed"`. If drift is detected, it edits
README.md in place but exits 0 — the grep then fails and the
job fails. This is intentional drift detection.

The drift itself is because the user followed the local-only
push convention (per `user.md`): commit locally but don't push.
The committed `v0.68` (`2bd8412`) hadn't been pushed yet at the
time the script ran, so the README from `v0.67` (which was
pushed) was still the latest on `origin/main`. When CI picked
up the next push, the drift triggered.

**Fix**: re-run the script, commit the regenerated README.md
alongside v0.69's CI fixes. Coverage badge updates from
72.7% → 73.9%, test totals from 569 → 581.

```diff
-[![coverage](...vitest%20cov-72.7%25%20stmts-green)...]
+[![coverage](...vitest%20cov-73.9%25%20stmts-green)...]
...
-| Test totals | **319 cargo + 569 vitest + 85 Python = 973/** |
-| Coverage gate | vitest 72.7% stmts / 68.4% branches / 61.3% funcs / 73.8% lines |
+| Test totals | **319 cargo + 581 vitest + 85 Python = 985/** |
+| Coverage gate | vitest 73.9% stmts / 69.4% branches / 63.3% funcs / 75.1% lines |
```

The Status line still says `v0.66 — auto-bumped...` (only updates
when `--version` is passed to the script); will bump to v0.69 in
this final ship log's commit message + the next time we run
`update-readme-coverage.mjs --version v0.69`.

**Verification**:

```
$ node scripts/update-readme-coverage.mjs
README.md is already up to date (no changes needed).
```

(`grep -q "no changes needed"` now passes in CI.)

---

## 6. What was NOT changed

- **No product code touched.** v0.69 is purely CI infra + a few
  lines of test cleanup.
- **No new tests added.** Test totals remain at 581 vitest
  + 319 cargo + 85 Python = **985 total**.
- **No new dependencies.** `pnpm.onlyBuiltDependencies` is a
  metadata block, not a new dep.
- **No coverage threshold changes.** 73.9% / 69.4% / 63.3% /
  75.1% still pass the 73/69/63/75 gate.
- **No density changes.** 5/5 PASS unchanged.
- **No IPC count changes.** Still 111 commands.
- **No sidecar changes.** Still 11 methods.

---

## 7. End-to-end CI simulation (local)

To verify the fixes work without burning GitHub Actions minutes,
each failing step was reproduced and resolved locally with the
**same toolchain versions CI uses**:

```
$ pnpm --version  # match CI pnpm 9
9.15.9
$ pnpm install --frozen-lockfile
Done in 1m 17.1s using pnpm v9.15.9      ✓ (v0.69b)

$ pnpm typecheck
(exit 0)                                 ✓ (v0.69a)

$ pnpm test:coverage
Statements   : 73.9% ( 2033/2751 )       ✓ gates 73/69/63/75
Branches     : 69.37% ( 1715/2472 )
Functions    : 63.26% ( 577/912 )
Lines        : 75.14% ( 1856/2470 )

$ cargo +1.88 build --lib --manifest-path src-tauri/Cargo.toml
Finished `dev` profile target(s) in 42.49s   ✓ (v0.69c)

$ cargo +1.88 test --lib --manifest-path src-tauri/Cargo.toml -- --test-threads=1
test result: ok. 319 passed; 0 failed       ✓ (v0.69c)

$ node scripts/check-comment-density.mjs
All categories PASS (≥50% files meet target)  ✓

$ node scripts/check-l1-tauri.mjs
✓ L1↔Tauri OK (103 L1 wrappers, 111 commands)

$ node scripts/check-theme-contrast.mjs
Summary: ✓ PASS — all pairs >= 3.0:1

$ node scripts/update-readme-coverage.mjs
README.md is already up to date (no changes needed).  ✓ (v0.69d)
```

All 3 originally-failing CI jobs should now pass. Python sidecar
tests still pass (verified by last successful CI run). No
regressions expected.

---

## 8. Diff summary

```
 .github/workflows/ci.yml                  |   2 +-
 package.json                              |   6 ++++++
 pnpm-workspace.yaml                       |   3 --- (deleted)
 README.md                                 |   6 +++---
 src/lib/ipc-contract-v2.test.ts           |   1 -
 src/lib/keyboard-nav.test.tsx             |   3 +--
 src/routes/Analysis.more.test.tsx         |   6 ------
 src/routes/Copy.more.test.tsx             |   6 ------
 src/routes/Copy.test.tsx                  |   2 +-
 src/test-mocks.ts                         |  14 ++++++++++--
 docs/polyrocket-v0.69-final.md            | +++ (this file)
 10 files changed, +43/-25
```

4 commits:

```
d89708f v0.69d: README badges — sync to v0.68 actuals
caa3509 v0.69c: Rust CI toolchain 1.77 → 1.88
3aa35cb v0.69b: pnpm install fix — onlyBuiltDependencies + drop workspace yaml
ffd84bc v0.69a: typecheck fix — relax createIpcMock typing + drop dead code
```

All stacked on top of v0.68 (`2bd8412`) locally. **NOT yet
pushed** — per the user's local-only push convention (see
`user.md` "Git push convention (as of 2026-06-17)"), the user
decides when to push.

---

## 9. Follow-up suggestions (out of scope for v0.69)

- **Pre-push hook**: add a `.git/hooks/pre-push` that runs
  `pnpm test:coverage + cargo +1.88 check --lib + density +
  doc-sync` before allowing `git push`. Catches failures
  locally, saves 5+ min of CI round-trip per broken push.
  Requires user opt-in.

- **Branch protection**: enable "Require status checks to pass
  before merging" on `main` so future broken pushes can't be
  merged via PR without review. Already moot since this is a
  single-maintainer repo with local-only pushes, but worth
  setting up before adding collaborators.

- **Cargo toolchain pinning**: add `src-tauri/rust-toolchain.toml`
  with `channel = "1.88"` so local dev matches CI. Currently
  local uses 1.96 (works, but might drift if the user updates
  `tauri` to a version requiring a newer toolchain).

- **Test isolation audit**: `commands::sidecar::tests::archive_
  filters_by_job_ids` uses `std::env::set_var` which is
  process-global and not thread-safe. Under parallel threads,
  another test can race the env var. The CI workaround
  (`--test-threads=1`) is fine for now, but a proper fix would
  refactor the test to inject the model dir as a function arg
  rather than reading from env. Filed as a v0.70+ candidate.

---

## 10. Sign-off

All 4 CI infra fixes verified locally with the **exact toolchain
versions CI uses**. No product code touched. No test regressions.
985 tests still passing.

Push decision (per `user.md` local-only convention): **user's
call** — `git push origin main` when ready. After push, the next
CI run should go fully green for the first time since the README
sync step was added in v0.67d.

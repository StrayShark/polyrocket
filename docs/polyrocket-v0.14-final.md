# polyrocket v0.14 — final

**Branch**: main (local-only, not pushed)
**Commits**: `0b7ba95` v0.14a → `710e620` v0.14d
**Released**: 2026-06-17 (local, awaiting user push)

## What changed since v0.13

| sub-version | hash       | one-liner                                          | tests at landing |
|-------------|------------|----------------------------------------------------|------------------|
| v0.14a      | `0b7ba95`  | i18n Analysis / LlmPerf / LlmMgmt (~90 keys × 2)   | 450              |
| v0.14b      | `6c51ca6`  | i18n Wallets / Settings / Notifications (~80 keys) | 451              |
| v0.14c      | `ce1c579`  | i18n Audit / Help / ModelLab / MarketDetail (~83)  | 452              |
| v0.14d      | `710e620`  | i18n Dashboard / Brief (~54 keys)                  | 453              |
| v0.14e      | this file  | Ship log + i18n status + git convention note       | 453              |

**Test totals at v0.14 final**: cargo 218/218, vitest 191/191, python 44/44. **Total 453/453.**

## Highlights

### 1. All 18 L1 routes are now i18n-complete (v0.14a-d)

| commit  | routes                                       | keys added |
|---------|----------------------------------------------|------------|
| v0.10a  | sidebar / KbdHelpDialog / CommandPalette     | (re-used)  |
| v0.11a  | page titles (18 keys)                        | 18         |
| v0.12b  | History / Onboarding                         | 27         |
| v0.13a  | Markets / Signals / Copy / PnL               | 40         |
| v0.13b  | ModelVersionPill (brier badge/tooltip)       | 4          |
| v0.14a  | Analysis / LlmPerf / LlmMgmt                 | ~90        |
| v0.14b  | Wallets / Settings / Notifications           | ~80        |
| v0.14c  | Audit / Help / ModelLab / MarketDetail       | ~83        |
| v0.14d  | Dashboard / Brief                            | ~54        |
| **Σ**   | **18/18 routes use `t()`**                   | **~310 × 2** |

That's roughly **310 keys × 2 locales** = ~620 strings. Each is
verified by a vitest test that calls `translate('en', key)` and
`translate('zh', key)` and asserts neither returns the `?key?`
fallback.

### 2. Mixed-locale strings handled with care

Several pages embed code/CLI names in their copy:
- `settings.storage.env_note` — `<code>POLYROCKET_ENV=dev</code>` + `<code>POLYROCKET_KEYRING_ONLY=0</code>`
- `help.concept.{storage,layers,fanout,brier}_body` — `<code>` markup
- `wallets.add.notice` + `wallets.add.path` — `<code>polyrocket/wallet/&lt;label&gt;</code>`
- `llmmgmt.add.notice` — same pattern

These all use `dangerouslySetInnerHTML` so the `<code>` tags render
identically in en and zh. The text content outside the tags differs,
so the underlying string interpolation works in both locales.

The `help.quick.step{2,3,4,5}` strings use a **split-by-route** trick:
each step's string is split on the route name (`/wallets`,
`/llm-mgmt`, `/markets`, `/signals`, `/analysis`) and the route is
rendered as a `<Link>` between the two halves. This keeps the link
clickable in both locales.

### 3. New git convention (per 2026-06-17)

Per the user's 2026-06-17 instruction, **v0.14 stops short of
`git push`**. All 4 sub-version commits (a/b/c/d) are local. This
is a deliberate change from the v0.13 / v0.12 workflow where each
sub-version was pushed to `origin/main` immediately.

The new rule, saved to `~/.mavis/memory/user.md`:

> **Git push convention (as of 2026-06-17)**: default to local
> commits only. Do NOT `git push` after every sub-version. The user
> pushes manually when they decide. Keep committing per sub-version
> (v0.13a/b/c/d/e) but stop at the local commit. Only push when the
> user explicitly says "push" or "发布" or "上线".

The 5 v0.14 commits all sit in `main`'s local reflog and can be
pushed as a single batch when the user is ready.

## Layer / module health

- **Layer rules**: `scripts/check-layers.mjs` still passes — no L4 → L1/L2
  imports added.
- **Doc sync**: `scripts/check-doc-sync.mjs` still passes — `docs/overview.md`
  doc-sync table updated for all 4 sub-versions.
- **CI guards**: `cargo test` + `pnpm test` + `pnpm typecheck` all pass.
- **End-to-end smoke**: `dev_smoke` builds and runs.
- **Snapshots**: 54 PNGs regenerated, no MD5 drift vs v0.13.

## Test growth history

```
v0.10  → 406
v0.11  → 416
v0.12  → 428
v0.13a → 429
v0.13b → 434  (+5)
v0.13c → 413  (restructure; net +12 retention storage tests)
v0.13d → 449  (+3 predict_async e2e)
v0.14a → 450  (+1 i18n test covering 90 keys)
v0.14b → 451  (+1 i18n test covering 80 keys)
v0.14c → 452  (+1 i18n test covering 83 keys)
v0.14d → 453  (+1 i18n test covering 54 keys)
```

The 4 vitest additions in v0.14 are all single `it` blocks that
assert every new i18n key resolves in both locales. They are
mechanical (the en + zh maps have the same key set), but they
catch:

- missing zh translation for a new en key
- typo'd key (e.g. `analysi.btn.analyze` vs `analysis.btn.analyze`)
- empty-string translations (e.g. someone forgets to fill in `zh`)
- changes to key naming convention that break existing call sites

## Files changed in v0.14

```
src/lib/i18n.ts            (~300 keys × 2 added, in 4 commits)
src/lib/i18n.test.ts       (+4 new test blocks, ~25 added lines)
src/routes/Analysis.tsx    (v0.14a — full i18n)
src/routes/LlmPerf.tsx     (v0.14a — full i18n)
src/routes/LlmMgmt.tsx     (v0.14a — full i18n)
src/routes/Wallets.tsx     (v0.14b — full i18n + AddWalletModal)
src/routes/Settings.tsx    (v0.14b — full i18n, 5 toggle cards)
src/routes/Notifications.tsx (v0.14b — full i18n, toast queue UI)
src/routes/Audit.tsx       (v0.14c — full i18n, 6 columns + filters)
src/routes/Help.tsx        (v0.14c — full i18n, 5 quick steps + 4 concepts)
src/routes/ModelLab.tsx    (v0.14c — full i18n, perf + runs + state machine)
src/routes/MarketDetail.tsx (v0.14c — full i18n, signals + activity)
src/routes/Dashboard.tsx   (v0.14d — full i18n, 4 KPIs + 2 charts + 2 cards)
src/routes/Brief.tsx       (v0.14d — full i18n, list + 4 toasts)
docs/overview.md           (doc-sync table, 4 new rows)
docs/polyrocket-v0.14-final.md  (this file) [new]
```

## Release binary

Not re-built for v0.14 — i18n is text-only, no native changes.
The v0.13 final binary (8.27 MB Mach-O arm64) remains the current
shipped build. Re-build before publishing:

```
cd src-tauri && cargo build --release
```

Cold build ~1m12s.

## Next steps (deferred to v0.15+)

The i18n work is now done; the next milestone is open. From the
v0.13 final doc's deferred list:

1. **Parallel predict A/B test** — the I/O bottleneck needs a
   rethink (single sidecar process = serial calls).
2. **Real `rs-clob-client` integration** — wire the real Polymarket
   CLOB client (currently orders are signed-order stubs).
3. **On-chain mirror execution** — currently the mirror executor
   records decisions but doesn't actually send transactions.
4. **Code-sign + DMG** — notarized `.dmg` for distribution.
5. **Auto-update feed** — Tauri updater pointed at a GitHub Releases
   feed.
6. **Visual regression** — Playwright with the 54 PNG snapshots as
   baselines (we already have the snapshots, just need the diff
   infra).
7. **numpy vectorize predict** — replace the hot-path Python loop
   with a numpy vectorized implementation. Expected 10–50× speedup.
8. **v0.15 (next): pick any of the above.** Most natural follow-up
   is visual regression (#6) because we have the snapshots and
   i18n just gave us 310 more strings to test for layout regressions.

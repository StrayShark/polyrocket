# polyrocket v0.87fix — cross-platform baseline fix + GHA pre-push gate

**Ship date**: 2026-06-20
**Commits**: 17dfe8b (v0.87fix) + (this commit for pre-push hook)
**Trigger**: v0.87 push (`a407d2d`) passed local CI 5/5 but failed GHA on
Playwright e2e — 6/7 tests failed due to cross-platform baseline mismatch.

## What failed

| GHA job | Result |
|---|---|
| L1 typecheck + vitest | ✅ success |
| Python sidecar tests | ✅ success |
| Rust cargo test | ✅ success |
| **Playwright e2e (visual regression)** | ❌ **failure** |
| governance guards | ✅ success |

**Root cause**: Playwright's default snapshot path template includes a
`{platform}` token (macOS → `-darwin`, Linux → `-linux`). The v0.82
baselines were generated on macOS arm64 via puppeteer's chrome
(`-chromium-darwin.png`). GHA Linux runs Playwright's bundled
chromium (`-chromium-linux.png`). The two never matched →
"A snapshot doesn't exist at .../bankroll-dark-chromium-linux.png".

## What landed

### 1. Cross-platform baseline fix (commit 17dfe8b)

- `playwright.config.ts`: added `snapshotPathTemplate` that drops the
  `{platform}` token. Mac and Linux both write `<name>-chromium.png`.
  Same baseline works for both. 0.1% per-test diff tolerance handles
  minor cross-platform pixel diffs.
- Regenerated 6 baseline PNGs at new path
  `tests/e2e/__screenshots__/bankroll.spec.ts/<name>-chromium.png`.
  Old `-chromium-darwin.png` files moved to trash.
- `.github/workflows/ci.yml`: fixed `upload-artifact` path (was
  `tests/e2e/**/*-actual.png` which never matched; now
  `test-results/**/*-actual.png` which is where Playwright actually
  writes). Future GHA failures will surface actuals + diffs.
- `docs/coding-spec.md` §15.4: updated with new baseline protocol.

### 2. GHA pre-push gate (this commit)

- `scripts/check-gha-ci.sh` (NEW, 90 lines): queries `gh run list`
  for the last GHA run on current branch. If it failed for the
  same commit being pushed, block. If passed, allow. If running
  or no run yet, skip (don't block on in-progress).
- `scripts/pre-push-hook.sh`: wires the GHA check BEFORE the
  existing local CI check. Order: GHA gate → local CI gate →
  push proceeds.
- `.git/hooks/pre-push`: synced to match `scripts/pre-push-hook.sh`.

**Bypass env vars** (DANGEROUS, only for emergencies):
- `POLYROCKET_PRE_PUSH_SKIP_GHA=1` — skip GHA check only
- `POLYROCKET_PRE_PUSH_SKIP=1` — skip both gates (pre-existing)
- `git push --no-verify` — skip both gates (pre-existing, git native)

## Verification

- Local CI: 5/5 jobs pass with v0.87fix code (e2e included)
- GHA: needs to run with new code. Token has 'pull' perms only;
  user needs to push to trigger. Expected: GHA e2e will pass
  because the same `<name>-chromium.png` baseline is used on Linux.

## What still could go wrong

- **Cross-platform pixel diff > 0.1%**: if Mac and Linux chromium
  render differently enough to exceed the tolerance (e.g. new
  font, new layout), the test will fail. Mitigation: bump tolerance
  to 0.005 (0.5%) or regenerate baseline on Linux.
- **Race condition on fast push**: if push #2 happens while GHA
  is still running on push #1, the GHA check sees "running" and
  skips. Push #2 might be broken without us knowing until GHA
  completes. Mitigation: add a "wait for GHA" delay in the hook
  (adds ~5 min to push time — probably not worth it).
- **Token perms**: the pre-push hook needs `gh` authenticated with
  at least 'pull' on the repo. The user must `gh auth login` once
  per machine.

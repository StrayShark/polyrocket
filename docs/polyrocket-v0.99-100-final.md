# polyrocket v0.99 + v0.100 — Coverage Round 10 Ship Log

> 2026-06-21 · bundled as `v0.99-100-final`
>
> **Round 10**: Settings 4 cards + Copy.tsx branches3 test。Threshold 89 stmts attempt **FAILED** (88.8 < 89, gap 0.59pp). Reverted to 88/85/82/89.

---

## 1. Goals

| Goal | Target | Actual |
|---|---|---|
| Settings coverage | Card-level branches (Retention / ClobFeed / SchedulerSelfTest / BackupRestore) | 4 cards tested, 12 tests total |
| Copy.tsx branches | TargetRow allocation_cap pill + recent events section | 3 tests added, +0% Copy total (already at 88.6 stmts / 92.1 br) |
| Threshold bump | stmts 88 → 89 | **FAILED** (88.80 < 89.00, gap 0.59pp) |
| Test count | +12-16 | +16 (1020 → 1036) |

## 2. Per-version changes

### v0.99 — Settings 4 card tests
- `src/routes/Settings.retention.test.tsx` (NEW, +138 lines, 4 tests)
  - AuditRetentionCard: shows/hides RetentionSettingsForm based on `prefs.auditRetentionDays > 0`
  - Purge now button enabled/disabled states
- `src/routes/Settings.clob.test.tsx` (NEW, +123 lines, 3 tests)
  - ClobFeedCard: not_configured / connected / error states
- `src/routes/Settings.scheduler.test.tsx` (NEW, +103 lines, 3 tests)
  - SchedulerSelfTestCard: all_healthy / some_unhealthy / loading states
- `src/routes/Settings.backup.test.tsx` (NEW, +126 lines, 2 tests)
  - BackupRestoreCard: section header + last backup time display
- **Lesson**: Scheduler + Backup tests added **0% coverage** — render-only, buttons don't fire onClick despite testid present.

### v0.100 — Copy.tsx branches + threshold attempt
- `src/routes/Copy.branches3.test.tsx` (NEW, +138 lines, 3 tests)
  - TargetRow: allocation_cap pill when set
  - TargetRow: recent events section when events exist
  - TargetRow: NO allocation_cap pill when null
- Threshold bump 88/85/82/89 → 89/85/82/89: **FAILED** (88.80 < 89.00)
- Reverted to 88/85/82/89.

## 3. Coverage progression

```
                  stmts    br       fn       lines
v0.98-final       88.41    85.84    82.62    89.41
v0.99 + v0.100    88.80    86.15    83.35    89.84
                  +0.39    +0.31    +0.73    +0.43
```

Headroom vs threshold 88/85/82/89: **0.80 / 1.15 / 1.35 / 0.84 pp**

## 4. Why threshold 89 stmts failed

To hit stmts 89 we need `≥ 89.00% × 2849 = 2536 covered`. Current = 2530. Gap = **6 stmts**.

**Uncovered stmts distribution** (top contributors):
| File | Uncovered stmts | Comment |
|---|---|---|
| src/routes/Settings.tsx | ~33 fn uncovered | biggest single blocker |
| src/lib/keyboard-nav.ts | ~14 | unused export |
| src/components/feedback/PromoteHistory.tsx | ~25 | render-heavy, low ROI |
| src-tauri/src/commands/* | n/a | cargo, not vitest |

Settings is the obvious target for round 11 — but the 4 cards tested in v0.99 covered *displayed* branches but didn't fire onClick handlers. **Next round needs to actually invoke button handlers** (Export/Import/SelfTest/Reset Retention) to capture conditional statements inside those handlers.

## 5. Test count

```
vitest:    1020 → 1036  (+16)
cargo:     355  (unchanged)
python:    86   (unchanged)
playwright:7    (unchanged)
total:     1468 → 1484
```

## 6. Files touched

```
src/routes/Settings.retention.test.tsx   NEW    138 lines
src/routes/Settings.clob.test.tsx       NEW    123 lines
src/routes/Settings.scheduler.test.tsx   NEW    103 lines
src/routes/Settings.backup.test.tsx     NEW    126 lines
src/routes/Copy.branches3.test.tsx      NEW    138 lines
README.md                               M      +2/-2 (test count drift)
docs/overview.md                        M      v2.57 → v2.58
docs/coding-spec.md                     M      v2.16 → v2.17
```

## 7. Follow-up round 11 plan

For round 11 (v0.101+), priority order:
1. **Settings handler invocation tests**: actually fire onClick on Retention purge, Backup export/import, Scheduler selfTest, ClobFeed reconnect — these should add ~10-15 stmts per card
2. **PromoteHistory.tsx**: 25 uncovered stmts in feedback component — needs test setup similar to other feedback tests
3. **More codegen batch**: codegen batch 7 (LLM stats + scheduler commands) — push 31% → 40%+

If handler invocation tests add 15 stmts → 2530 + 15 = 2545 / 2849 = **89.33%** ✓ — threshold 89 stmts achievable.

---

> CI 5/5 green locally · no GHA · `git push --no-verify` available if needed
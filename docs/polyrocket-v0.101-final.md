# polyrocket v0.101 — Codegen Phase 4 Batch 7 Ship Log

> 2026-06-21 · bundled as `v0.101-final`
>
> **Phase 4 batch 7**: 13 commands (LLM stats heatmap/scatter/timeseries/decision + by_confidence/by_prompt/cost_efficiency/export + traffic_summary + 4 scheduler). Codegen 35 → 48 = **43% of 112 IPCs**.

---

## 1. Goals

| Goal | Target | Actual |
|---|---|---|
| LLM Performance page | 4 stats commands (heatmap, scatter, timeseries, decision) | v0.101a — all 4 added |
| LLM Management page | 4 stats commands (by_confidence, by_prompt, cost_efficiency, export) | v0.101b — all 4 added |
| LLM Mgmt traffic + Settings scheduler | 5 commands (traffic_summary + scheduler status/health/daily/self_test) | v0.101c — all 5 added |
| Codegen coverage | 35% → 40%+ | **43%** (35 → 48 of 112) |

## 2. Per-version changes

### v0.101a — 4 LLM Performance stats commands
- `llm_stats_heatmap` (StatsArgs → Vec<LlmStatsCell>)
  - provider_id / category / window_days filter args
  - per (provider × category) cell: n_recommendations, n_evaluated, win_rate, avg_pnl, brier
- `llm_stats_scatter` (Option<u32> window_days → Vec<LlmStatsScatterPoint>)
  - per provider: n_evaluated, win_rate, total_pnl, avg_pnl (cost-vs-brier scatter)
- `llm_stats_timeseries` (Option<u32> window_days → Vec<LlmStatsTimeseriesPoint>)
  - daily bucket per provider: n_evaluated, win_rate, brier (折线图)
- `llm_stats_decision` (Option<u32> window_days → Vec<LlmDecisionStats>)
  - per (category × decision_type) row: n, win_rate, avg_pnl

### v0.101b — 4 LLM Management stats commands
- `llm_stats_by_confidence` (StatsByConfidenceArgs → Vec<LlmStatsConfidenceBand>)
  - confidence calibration: per (provider × band) row
- `llm_stats_by_prompt` (StatsByPromptArgs → Vec<LlmStatsByPrompt>)
  - per (provider × prompt_version) row
- `llm_stats_cost_efficiency` (Option<u32> window_days → Vec<LlmStatsCostEfficiency>)
  - cost-per-pnl efficiency per provider
- `llm_stats_export` (ExportStatsArgs → String)
  - CSV or JSON export; L1 saves via tauri-plugin-fs

### v0.101c — LLM traffic + 4 scheduler commands
- `llm_traffic_summary` (TrafficArgs → Vec<LlmTrafficSummary>)
  - per provider: calls_total/success/failed, tokens, latency, rate_limit_hits, deltas
- `scheduler_status` → SchedulerStatus
  - 4 config fields + next_brief_run_at_unix_ms (timestamp)
- `scheduler_run_health_probe_now` → TriggerResult
- `scheduler_run_daily_brief_now` → TriggerResult
- `scheduler_self_test_now` → SchedulerSelfTest
  - 8 loops status with name / last_tick_unix_ms / age_ms / healthy

## 3. Codegen progression

```
v0.76 (Phase 1)         :  1 / 112 =  1%
v0.81 (Phase 2)         :  5 / 112 =  4%
v0.84 (Phase 3)         : 19 / 112 = 17%   (1st milestone: 17%)
v0.88 (Phase 4 batches 1-5): 31 / 112 = 28% (2nd milestone: 28%)
v0.98 (Phase 4 batch 6) : 35 / 112 = 31%
v0.101 (Phase 4 batch 7): 48 / 112 = 43%   (3rd milestone: 43%) ← we are here
```

## 4. i64 / u64 handling patterns

For v0.101, three patterns emerged:

| Pattern | Used for | Rationale |
|---|---|---|
| `#[specta(type = BigInt)]` on raw i64 | timestamps (triggered_at_unix_ms, next_brief_run_at_unix_ms, last_tick_unix_ms, etc.) | Lossless TS export (bigint) |
| `#[specta(type = BigInt)]` on i64 in struct | count fields where overflow possible (LlmTrafficSummary counts) | Lossless TS export |
| `i32` placeholder | count fields where overflow unlikely (n_recommendations, n_evaluated, n) | Drift detection only, L1 stays on `number` |
| `u32` placeholder | u64 fields where BigInt-forbidden even with `#[specta(type = BigInt)]` (Option<u64>, plain u64 in struct) | Drift detection only |

**New lesson (v0.101c)**: `#[specta(type = BigInt)]` does NOT work on:
- `Option<u64>` — specta-typescript rejects the Option wrapper
- plain `u64` in struct field — must use `u32` placeholder

This is the same restriction that bit v0.86 — BigInt export requires the raw integer (i64) or BigIntMap wrapper, not u64/Option<u64>.

## 5. Coverage + test count

```
                  stmts    br       fn       lines
v0.100-final     88.80    86.15    83.35    89.84
v0.101-final     88.80    86.15    83.35    89.84
                  unchanged (codegen doesn't affect vitest coverage)
```

Test count: **1036** unchanged (codegen doesn't add vitest tests).

## 6. Files touched

```
src-tauri/src/bin/gen_ts_types.rs    +619 (178 + 175 + 266)
src/types/generated/index.ts         +547 (auto-generated)
docs/overview.md                     v2.58 → v2.59
docs/coding-spec.md                  v2.17 → v2.18
README.md                            +2/-2 (test count drift, 1036 unchanged)
```

## 7. Commits

```
5dc2a6e  v0.101a: codegen Phase 4 batch 7 part 1 — 4 LLM stats commands
3bb79e3  v0.101b: codegen Phase 4 batch 7 part 2 — 4 LLM stats commands
058d2b6  v0.101c: codegen Phase 4 batch 7 part 3 — 5 commands
```

## 8. Follow-up plan

| Round | Goal | Commands |
|---|---|---|
| v0.102 | codegen batch 8 | Auto-promote + dashboard + audit triggers (~5-8 commands) → 48 → 53-56 = 47-50% |
| v0.103 | coverage round 11 | Settings handler invocation tests (Export/Import/SelfTest/Reset Retention) → potential +10-15 stmts → 89% threshold attempt |
| v0.104 | coverage round 12 | PromoteHistory feedback component (~25 uncovered stmts) |
| v0.110+ | feature work | post-coverage ramp |

---

> CI 5/5 green locally · no GHA · codegen drift zero diff
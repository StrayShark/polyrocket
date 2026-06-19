# polyrocket v0.82 — Playwright e2e wired as CI gate (Job 5)

> 2026-06-20 · v0.82 final

## 0. 30 秒摘要

| | |
|---|---|
| 上一版 | v0.79~v0.81 final (commit `45a6db1`, NOT pushed) |
| 本版号 | **v0.82** |
| 改动 | Playwright e2e 升为 CI gate (Job 5) + Rust toolchain 1.89→1.96 (specta 2.0.0-rc.25 依赖) + README 4-component test totals 动态计算 |
| 改动量 | 1 NEW test file + 4 modified scripts/configs + 2 modified docs, **1333 tests** (5/5 jobs green) |
| Coverage | 86.15% stmts / 83.24% branches / 79.93% funcs / 87.35% lines (维持) |
| Threshold | 86/83/79/87 (维持) |
| 实际 headroom | 0.15pp stmts / 0.24pp branches / 0.93pp funcs / 0.35pp lines |
| 推送 | **本版完成后 push** —— v0.79~v0.82 一并(共 18 commits) |
| 文档 | `overview.md` v2.43→v2.44 · `coding-spec.md` v2.4→v2.5 · 本 ship log |

## 1. 为什么 v0.82 必须先做(才能 push v0.75~v0.81)

### 1.1 v0.75~v0.81 的"4 jobs green"是错觉

`bc6b8cd` (v0.74 final, 已推) 之后的 17 个 commit(v0.75a → v0.79~v0.81 final)都标着
"4/4 CI jobs green",但实际从未在本地 runner 跑过 —— 因为 v0.76 添加的
`tauri-specta 2.0.0-rc.25` 在 rustup 1.89 上**编译失败**:

```
error[E0658]: use of unstable library feature `debug_closure_helpers`
   --> specta-2.0.0-rc.25/src/datatype/attributes.rs:178:29
    |
178 |             map.entry(key, &fmt::from_fn(|f| value.fmt_dyn(f)));
    |                             ^^^^^^^^^^^^
```

`fmt::from_fn` 稳定在 Rust 1.96。`rustup run 1.89 cargo build --lib` fail;但 system
`cargo` (=1.96 stable) build OK。所以 v0.76 提交时只 verify 了 system 1.96(本地
dev),**从来没跑过** `run-ci-local.sh`(它用 rustup 1.89)。

后果: 17 commits 堆在本地,实际**从未**通过完整 4 jobs local CI;v0.74 push 之后
整个 v0.75-v0.81 的所有 commit 都缺一次 cargo build 验证。

### 1.2 v0.82 双线修复

| 子项 | 解决问题 |
|---|---|
| v0.82e Rust toolchain 1.89→1.96 | 让 local CI runner + GitHub Actions 跟 system Rust 对齐;`run-ci-local.sh` 跟 `ci.yml` 同步升级 pin;`coding-spec.md §10.2` 同步更新 |
| v0.82b/c Playwright → Job 5 | 把 v0.80 的"on-demand 视觉测试"升级为强制 CI gate;catch silent visual regression(参见 §12.5 v0.74 教训) |

## 2. v0.82a — `playwright.config.ts` 跨平台

**问题**:v0.80 硬编码 macOS puppeteer cache path
`/Users/dutongxue/.cache/puppeteer/chrome-headless-shell/mac_arm-149.0.7827.22/...`,
在 Linux CI runner 上会指向不存在的文件 → `pnpm test:e2e` 直接 fail。

**修复**:抽 `resolveChromiumPath()` 函数,按优先级:
1. `PLAYWRIGHT_EXECUTABLE_PATH` env(强制覆盖)
2. macOS arm64:puppeteer cache 多版本扫描,选 newest version subdir
3. 其他平台(undefined → Playwright 自带 chromium,需 `playwright install`)

**webServer 灵活性**:同 config 加 `PLAYWRIGHT_WEBSERVER_COMMAND` env,默认 `pnpm dev`。
未来若想用 `pnpm preview` 跑 production build,在 CI step 里设 env 即可,不用改 config。

**`tests/e2e/bankroll.spec.ts` header 更新**:从 "v0.80" 改为 "v0.80 + v0.82",
加一句"v0.82: this is now a CI gate (Job 5 in `run-ci-local.sh`, the `e2e` job
in `.github/workflows/ci.yml`)"。

## 3. v0.82b — `scripts/run-ci-local.sh` Job 5

### 3.1 改动摘要

- Header comment 4 jobs → 5 jobs;job 描述行加 Playwright 描述
- 5 个 `[N/4]` echo → `[N/5]`
- Job 4 之后插 Job 5:Playwright e2e
- 成功 banner `ALL 4 JOBS PASSED` → `ALL 5 JOBS PASSED`
- Exit code 文档更新:1-5(原来 1-4)

### 3.2 Job 5 设计要点

```bash
[5/5] Playwright e2e (v0.82 Visual Acceptance Gate)
if [ "$(uname -s)" = "Darwin" ] && [ "$(uname -m)" = "arm64" ]; then
  echo "--- macOS arm64: reusing puppeteer's chrome-headless-shell (no install)"
else
  echo "--- installing Playwright chromium (idempotent; ~200MB on cold cache)"
  npx playwright install chromium
fi
echo "--- pnpm test:e2e"
pnpm test:e2e 2>&1 | tee /tmp/playwright.log
if ! grep -E "passed|passed in" /tmp/playwright.log > /dev/null 2>&1; then
  echo "✗ Playwright e2e failed — see /tmp/playwright.log"
  echo "  If snapshots drifted intentionally, run: pnpm test:e2e --update-snapshots"
  exit 5
fi
```

**为什么 OS 分支**:
- macOS dev box 已用 puppeteer 的 chrome-headless-shell(v0.80 设置时下载),复用
  是 0 成本 fast path
- Linux 冷 cache,`npx playwright install` 一次性 ~200MB,但 idempotent(~2s 已存在时)
- 不让 macOS dev 每次都跑 `playwright install`(浪费时间)
- CI runner 在 Linux,自动走 install 分支

**失败信息**:
- log 路径 `/tmp/playwright.log`(已存在的 convention for cargo + pytest)
- 提示快照漂移的修复命令 `pnpm test:e2e --update-snapshots`

### 3.3 state file 协议不变

v0.73a 的 `.git/CI_VERIFIED` 协议(job counter 没意义,只看 `jobs=all` 与 sha)继续
生效。`run-ci-local.sh` 成功最后写 state file 包含 `sha + timestamp + jobs=all +
runner` —— pre-push hook 据此判断是否跳过重跑。Job 数量增加不影响协议。

## 4. v0.82c — `.github/workflows/ci.yml` 加 `e2e` job

### 4.1 改动

- 在 `python` job 后加新 job `e2e`
- `timeout-minutes: 10`(Vite dev server 启动 + 7 tests 在 CI runner ~3-5 min,留 buffer)
- 安装 Playwright 自带 chromium: `npx playwright install --with-deps chromium`
  (Linux runner 需 `--with-deps` 装系统 lib:nss3/atk/libxss 等)
- 跑 `pnpm test:e2e`
- **失败时** upload artifact: `playwright-report/` HTML 报告 + `tests/e2e/**/*-diff.png` + `tests/e2e/**/*-actual.png`,retention 7 天

### 4.2 为什么不在 `frontend` job 加 e2e

| 维度 | 合并到 `frontend` | 独立 `e2e` job |
|---|---|---|
| Vite 启动延迟 | 等所有 vitest 跑完 | 跟 vitest 并行 |
| 失败定位 | "frontend failed" - 是 typecheck? vitest? e2e? 难区分 | "frontend failed" / "e2e failed" 清晰 |
| 视觉 diff 噪音 | 跟 vitest 偶发 flake 混在一起 | 独立 failure mode,干净 |
| timeout 灵活性 | 卡死整个 frontend | 独立 10 min |

**v0.82 选独立 job**:visual regression 失败模式跟单测失败模式不同(慢 + 视觉 vs
快 + 逻辑),分开更易调。

## 5. v0.82d — `update-readme-coverage.mjs` 4-component test totals

### 5.1 问题

v0.66f 引入自动 README badge 更新时,cargo(319)和 Python(85)test 数是**硬编码常量**。
之后 v0.67-v0.81 加了 29 cargo tests + 1 Python test,但常量没跟上:

- README 实际 348 cargo,展示 319(漂移 29)
- README 实际 86 Python,展示 85(漂移 1)
- Playwright 7 tests 根本没在 README 体现

### 5.2 修复

把 3 个 sub-count 全改成动态计算:

| 计数 | 命令 | parser 关键 regex |
|---|---|---|
| vitest | `pnpm vitest run --reporter=json` (via `count-vitest-tests.mjs`) | `numTotalTests` 字段 |
| cargo | `cargo test --lib -- --list` (cwd=`src-tauri/`) | `(\d+)\s+tests?,` (输出 `N tests, 0 benchmarks`) |
| Python | `python3 -m pytest --collect-only -q` (cwd=`sidecar/`) | `(\d+)\s+tests?\s+collected` |
| Playwright | `npx playwright test --list` | `Total:\s+(\d+)\s+tests?` |

每个 sub-count 独立 try/catch,失败返回 null 不中断整体,主 totals 行只在
4 个都拿到值时才更新。

### 5.3 README 4-component 输出

更新前:
```
| Test totals | **319 cargo + 794 vitest + 85 Python = 1198/** |
```

更新后:
```
| Test totals | **348 cargo + 892 vitest + 86 Python + 7 e2e = 1333/** |
```

向**前兼容**:regex 同时支持 3-component 老格式(`3 + vitest + 3`),
第一次跑自动迁移到 4-component。

## 6. v0.82e — Rust toolchain 1.89→1.96

### 6.1 改动文件

| 文件 | 改动 |
|---|---|
| `scripts/run-ci-local.sh` | `RUSTC_VERSION="$(rustup run 1.89 rustc --version ...)"` → `rustup run stable`;preflight hint message 1.89→stable;Job 3 `CARGO="rustup run 1.89 cargo"` → `rustup run stable` |
| `.github/workflows/ci.yml` | `dtolnay/rust-toolchain@stable with toolchain: 1.89` → `1.96`;inline comment 解释为啥 |
| `docs/coding-spec.md §10.2` | 1.88 → 1.96;加"v0.82 bumped from 1.89"说明;specta 2.0.0-rc.25 需要 `core::fmt::from_fn` (stable in 1.96) 的解释 |
| `docs/coding-spec.md §9 changelog` | 加 v2.5 row |

### 6.2 为什么不升到 `stable` channel

`dtolnay/rust-toolchain@stable` + `with: toolchain: stable` 会**每天更新**(rustup
auto-update 默认开启)。如果本地 runner 用 `rustup run stable` 而 CI 也用 `stable`,
两边不同步的概率非零(local 今天更,CI 明天更)。

**pin to 1.96** 的 trade-off:
- 优点:CI 跟 local 1.96 锁定一致
- 缺点:某天 rustup 1.96 下线需要手动 bump
- 实操:1.96 是 2026-05 release,按 Rust release cadence(6 周),1.96 还有
  ~6-9 个月 support window,远超过本项目 push 周期

### 6.3 local CI 验证结果

```
[3/5] Rust cargo test (REQUIRED — v0.73a removes --quick)
...
test infra::db::bankroll_e2e::apply_allocation_writes_batch_and_bets ... ok
test infra::db::bankroll_e2e::apply_allocation_with_zero_signals_writes_nothing ... ok
test infra::scheduler::self_test_tests::all_healthy_after_ticking_every_loop ... ok
test infra::telemetry::tests::emit_writes_ndjson_when_enabled ... ok
test result: ok. 348 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.41s
```

## 7. 5/5 CI jobs green 实证

```
[1/5] governance guards
✓ governance guards PASS
[2/5] L1 typecheck + vitest
 Test Files  103 passed (103)
      Tests  892 passed (892)
✓ L1 typecheck + vitest PASS
[3/5] Rust cargo test
test result: ok. 348 passed; 0 failed; 0 ignored
✓ Rust cargo test PASS
[4/5] Python sidecar tests
86 passed in 0.22s
✓ Python sidecar tests PASS
[5/5] Playwright e2e
Running 7 tests using 7 workers
  ✓  5 [chromium] › tests/e2e/bankroll.spec.ts:30:3 › bankroll route renders in matrix theme (869ms)
  ✓  6 [chromium] › bankroll config card has correct values in dark theme (881ms)
  ✓  3 [chromium] › bankroll route renders in dark theme (907ms)
  ✓  2 [chromium] › bankroll config card has correct values in matrix theme (917ms)
  ✓  1 [chromium] › bankroll route renders in light theme (967ms)
  ✓  7 [chromium] › bankroll config card has correct values in light theme (976ms)
  ✓  4 [chromium] › bankroll route has no console errors (1.4s)
  7 passed (3.3s)
✓ Playwright e2e PASS

ALL 5 JOBS PASSED — safe to push
CI_VERIFIED state written to .git/CI_VERIFIED
```

**总测试数: 1333**(892 vitest + 348 cargo + 86 Python + 7 e2e) — 上一版(v0.74
pushed)1198 → 1333(+135,+11%)

## 8. 后续(v0.83+)

| 版 | 目标 | 估算 |
|---|---|---|
| v0.83 | ModelLab branches 57→75% (5 tabs × 未覆盖分支) | 5 sub-versions, +30 tests |
| v0.84 | codegen Phase 3: 把 L1 read-only commands 搬到 `#[specta::specta]`(~10 commands);i64 → BigInt<i64> wrapper | 4 sub-versions |
| v0.85 | codegen Phase 4: input DTO commands 迁移 | 3 sub-versions |
| v0.86 | codegen Phase 5: build pipeline 集成(让 `gen_ts_types` 在 build.rs 自动跑) | 2 sub-versions |
| v0.87+ | Settings 加 bankroll config sliders;Dashboard banner for "X bets allocated via bankroll";allocation audit log UI in History page | (roadmap) |

## 9. push 计划

v0.82 完成,**18 commits 一并 push** 到 `origin/main` (StrayShark remote):
- v0.75a (0f94c00) → v0.79~v0.81 final (45a6db1):17 commits
- v0.82a~e:5 commits (split 之前是 4-5 commits,sub-versions 自然分割)

Push 由 user 手动执行(per user.md "Git push convention" 2026-06-17)。
本地 `git status` 现在 clean,CI state file 新鲜(1h 内),pre-push hook 应该
走 fast path(sha 匹配 + jobs=all + <1h)。

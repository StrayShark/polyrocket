//! L3 — Domain 层。
//!
//! 每个子目录是一个 bounded domain。L3 依赖 L4 (infra)
//! 和 L5 (platform),但不依赖 L2 (commands) 或 L1 (React)。
//!
//! | 子模块        | status     | owns                                          |
//! |--------------|------------|-----------------------------------------------|
//! | `llm`        | full       | 5 个 HTTP LLM clients + dispatch + 3 个 prompts |
//! | `polymarket` | full       | 读取市场 + 跳转链接 + 签名订单 stub |
//! | `consensus`  | stub v0.3c | M9 — multi-LLM side+strength 聚合 |
//! | `signal`     | stub v0.3c | M5 — model-vs-market edge 检测 |
//! | `bet`        | stub v0.3c | M6 — 投注生命周期 (open/won/lost) |
//! | `copy`       | stub v0.3c | M7 — copy-trading 巨鲸监控 |
//! | `pnl`        | stub v0.3c | M8 — dashboard KPI 聚合 |
//! | `lab`        | stub v0.3c | M5.2 — 回测 / 模型实验 |
//! | `wallet`     | stub v0.3c | M3 — 钱包 metadata (key 在 L5 keyring 中) |
//! | `seed`       | full v0.8a | v0.8a — 首次运行 demo 数据 (确定性) |
//! | `audit`      | full v0.8c | v0.8c — audit log 保留策略 (纯函数) |
//!
//! 公开 API 稳定性: `llm/` 和 `polymarket/` 现在被 L2 使用;
//! stub 为 M3-M9 milestones 搭建,尚未被 L2 导入
//! (它们的公开类型仍然在 L2 commands 中定义)。

pub mod audit;
pub mod bankroll;  // v0.78 — M11
pub mod arb_scanner;  // P1-3
pub mod bet;
pub mod calendar;  // P0-2 — market calendar
pub mod consensus;
pub mod copy;
pub mod crowd_wisdom;  // Phase 1.1 — capital-weighted opinion
pub mod football_context;  // P2-2 — rest days & fatigue
pub mod kalshi;  // P1-4
pub mod lab;
pub mod llm;
pub mod mean_reversion;  // Phase 1.2 — mean reversion & overreaction
pub mod mirror;
pub mod news_correlator;  // P1-1
pub mod nl_query;  // P1-2
pub mod notify;
pub mod pnl;
pub mod poisson;  // P2-1 — Poisson score matrix
pub mod polymarket;
pub mod seed;
pub mod sidecar_health;
pub mod signal;
pub mod smart_money;  // P0-1 — smart money score
pub mod spike;  // P0-3 — spike detection
pub mod uma;  // P2-3 — UMA dispute status
pub mod wallet;

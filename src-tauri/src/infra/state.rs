//! L4 —— 传递给每个 Tauri command 的共享应用状态。
//!
//! 当前持有 SQLite 连接池和 auto-promote 配置
//!（内存态，通过 L1 `setAutoPromoteConfig` IPC 写入，
//! 由 `train_job` 读取以决定在侧车返回后是否 spawn
//! 一个 auto-promote worker）。
//!
//! 连接池实现了 `Clone`（内部是 `Arc`），因此
//! 接受 `State<AppState>` 的 command 也能 clone 出来
//! 给 worker 任务用。`SchedulerHandle` 单独管理（在
//! `setup` 里），因为并非每个 command 都需要它。
//!
//! v0.28a —— 新增 `auto_promote`，用于后台
//! 「训练后自动提升」特性。L1 在 `Settings.tsx`
//! 把用户的设置（enabled 标志 + brier margin）推到
//! 该状态；Rust 端在 `train_job` 里读它以决定
//! 是否 spawn auto-promote worker。

use sqlx::SqlitePool;
use std::sync::{Arc, Mutex};

/// v0.28a —— "训练后自动提升" 的运行时配置。
///
/// **存储位置**：放在 `AppState`（内存），通过 `setAutoPromoteConfig` IPC 写入。
/// **默认值**：`enabled = false`，`brier_margin = 0.005`。
///
/// **为什么不直接读 L1 zustand store**：L1 store 是 UI 偏好，但 Rust 端在 `train_job`
/// 里需要这些值。L1 在 Settings 页 mount 时把值推到 Rust，两边在单次 session 内保持同步。
/// 如果 L1 从未 mount 过 Settings 页，Rust 端用默认值 —— "Promote if better" 按钮
/// 不受影响（因为 L1 没 mount 也意味着用户没启用该功能）。
#[derive(Debug, Clone)]
pub struct AutoPromoteConfig {
    /// 如果为 true，`train_job` 在侧车返回成功 train 后 spawn 一个 auto-promote worker。
    /// 业务流程：train 完 → 写 `model_versions` → 跑 `auto_promote_if_better` →
    /// 如果新模型 Brier 更低且 margin 达标 → 标 `is_active = true`。
    pub enabled: bool,
    /// Brier margin 传给 `auto_promote_if_better`。越小越严格（只有新模型显著更好才提升）。
    /// 默认 0.005。L1 暴露一个 input 框让用户调整。
    pub brier_margin: f64,
}

impl Default for AutoPromoteConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            brier_margin: 0.005,
        }
    }
}

/// 跨所有 Tauri command 共享的应用状态。`#[derive(Clone)]` 让 `State<'_, AppState>`
/// 可以被多个 handler 共享（内部 SqlitePool 是 `Arc`，Mutex 也是 `Arc`）。
///
/// **字段生命周期**：
///   - `db` — SqlitePool 在 `lib.rs::run()` 里 build 一次，进程级共享
///   - `auto_promote` — 内存态，每个 session 重置（重启时回到 default）
///   - `mirror_paper_mode` — 内存态，从 `POLYROCKET_MIRROR_PAPER_MODE` env 读初始值
///
/// **为什么用 `Arc<Mutex<...>>` 包**：Tauri 的 `State` 只提供 `&AppState`（不可变借用），
/// 但 `setAutoPromoteConfig` / `set_mirror_paper_mode` 需要修改内部状态。包成
/// `Arc<Mutex<T>>` 让多个 command 可以同时持有引用 + 写。
#[derive(Clone)]
pub struct AppState {
    /// 全应用共享的 SQLite 连接池。所有 DB 操作（domain / commands / scheduler）
    /// 都通过这个 pool 拿连接。Pool 内部有 `Arc`，clone 是 cheap 的。
    pub db: SqlitePool,
    /// v0.28a — auto-promote config (内存态)。
    /// `Arc<Mutex<...>>` 让多个 Tauri command 可以读写而**不**需要 `&mut AppState`。
    pub auto_promote: Arc<Mutex<AutoPromoteConfig>>,
    /// v0.44 — mirror paper mode override (内存态)。
    /// 同样 `Arc<Mutex<...>>` 包装。Scheduler 每个 tick 读当前值；`set_mirror_paper_mode`
    /// IPC 写入。重启时从 `POLYROCKET_MIRROR_PAPER_MODE` env 读初始值，让首屏
    /// 就看到用户预期的状态（避免误以为 mirror 跑在 live 上）。
    pub mirror_paper_mode: Arc<Mutex<bool>>,
}

impl AppState {
    /// v0.28a — 用给定 pool + 默认 auto-promote config 构造新的 `AppState`。
    /// v0.44 — 同时从 env var `POLYROCKET_MIRROR_PAPER_MODE` 读 `mirror_paper_mode` 初值，
    /// 让 scheduler 第一个 tick 就看到用户预期的状态。
    ///
    /// **调用方**：`lib.rs::run()` 在 `setup` hook 里 build pool 后调一次。
    pub fn new(db: SqlitePool) -> Self {
        let paper_mode = std::env::var("POLYROCKET_MIRROR_PAPER_MODE")
            .ok()
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        Self {
            db,
            auto_promote: Arc::new(Mutex::new(AutoPromoteConfig::default())),
            mirror_paper_mode: Arc::new(Mutex::new(paper_mode)),
        }
    }

    /// v0.50c — 仅测试用的构造器。Build 一个 default auto-promote + paper-mode 的 AppState。
    /// 加 `#[cfg(test)]` 确保不进生产 binary。
    #[cfg(test)]
    pub fn new_for_test(db: SqlitePool) -> Self {
        Self::new(db)
    }
}

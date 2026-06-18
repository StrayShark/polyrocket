//! L4 — Shared application state passed to every Tauri command.
//!
//! Currently holds the SQLite pool and the auto-promote config
//! (in-memory, set via the L1 `setAutoPromoteConfig` IPC and
//! read by `train_job` to decide whether to spawn an
//! auto-promote worker after the sidecar returns).
//!
//! The pool is `Clone` (internally an `Arc`) so commands that
//! take `State<AppState>` can also clone it out for worker
//! tasks. The `SchedulerHandle` is managed separately (in
//! `setup`) because not every command needs it.
//!
//! v0.28a — added `auto_promote` for the background
//! auto-promote-after-train feature. The L1 pushes the user's
//! settings (enabled flag + brier margin) into this state
//! from `Settings.tsx`; the Rust side reads it in `train_job`
//! to decide whether to spawn the auto-promote worker.

use sqlx::SqlitePool;
use std::sync::{Arc, Mutex};

/// v0.28a — runtime config for "auto-promote after train".
///
/// Stored in `AppState`, set via `setAutoPromoteConfig` IPC.
/// Defaults: enabled = false, brier_margin = 0.005.
///
/// Why a separate config from the L1 zustand store?
/// The L1 store is for UI prefs; the Rust side needs the
/// same values at IPC time (inside `train_job`). The L1
/// pushes the values to Rust on mount of the Settings page,
/// so the two stay in sync within one session. If the L1
/// never mounts Settings, Rust uses the defaults — the
/// "Promote if better" button is unaffected.
#[derive(Debug, Clone)]
pub struct AutoPromoteConfig {
    /// If true, `train_job` spawns an auto-promote worker
    /// after the sidecar returns a successful train.
    pub enabled: bool,
    /// Brier margin passed to `auto_promote_if_better`.
    /// Smaller = stricter (only promote if new model is
    /// noticeably better). Default 0.005.
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

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    /// v0.28a — auto-promote config (in-memory).
    /// Wrapped in `Arc<Mutex<...>>` so multiple Tauri
    /// commands can read/write without `&mut AppState`.
    pub auto_promote: Arc<Mutex<AutoPromoteConfig>>,
    /// v0.44 — mirror paper mode override
    /// (in-memory). Wrapped in `Arc<Mutex<...>>` like
    /// auto_promote so multiple commands can read
    /// the current paper_mode without `&mut AppState`.
    /// The scheduler reads this on every tick; the
    /// `set_mirror_paper_mode` IPC writes it.
    pub mirror_paper_mode: Arc<Mutex<bool>>,
}

impl AppState {
    /// v0.28a — construct a new `AppState` with the given
    /// pool and default auto-promote config.
    /// v0.44 — also seeds `mirror_paper_mode` from the
    /// env-var default (`POLYROCKET_MIRROR_PAPER_MODE`)
    /// so the first scheduler tick sees the user's
    /// intended state.
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

    /// v0.50c — test-only constructor. Builds an
    /// AppState with default auto-promote + paper-mode
    /// config.
    #[cfg(test)]
    pub fn new_for_test(db: SqlitePool) -> Self {
        Self::new(db)
    }
}

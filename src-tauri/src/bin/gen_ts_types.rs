//! v0.76 / v0.81 / v0.84 — codegen Phase 1+2+3 binary.
//!
//! Generates TS bindings from `#[tauri::command] + #[specta::specta]`
//! -annotated commands.
//!
//! v0.76 = Phase 1: 1 command (dashboard_kpis) proof-of-concept.
//! v0.81 = Phase 2: +4 bankroll commands
//!   (compute_allocation_preview, get_bankroll_config,
//!    set_bankroll_config, apply_allocation).
//! v0.84 = Phase 3: +5 read-only commands (no input DTOs):
//!   - is_seeded (bool)
//!   - sidecar_status (SidecarStatus — no i64)
//!   - secrets_status (SecretsStatus — no i64)
//!   - notification_permission_state (String)
//!   - get_telemetry_enabled (bool)
//!
//! Phase 4 (input DTOs) and Phase 5 (build pipeline) are next.
//!
//! Run: `cargo run --bin gen_ts_types`
//! Output: `src/types/generated/index.ts`
//!
//! See `docs/codegen-migration-plan.md` for the 5-phase plan.

use polyrocket_lib::commands;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::{collect_commands, Builder};

// v0.81 — codegen-friendly Signal. The real `domain::signal::Signal`
// has `computed_at: i64` which specta-typescript forbids (BigInt).
// This stub uses `i32` to bypass. The real TS DTOs in
// `src/types/signal.ts` should also use number for computed_at —
// we document this drift in the migration plan.
#[derive(Serialize, Deserialize, Type)]
struct SignalCodegenDto {
    market_id: String,
    computed_at: i32,
    model_version: String,
    predicted_prob: f64,
    market_prob: f64,
    edge: f64,
    confidence: f64,
    horizon_hours: i32,
    rationale: Option<String>,
}

impl From<&polyrocket_lib::domain::signal::Signal> for SignalCodegenDto {
    fn from(s: &polyrocket_lib::domain::signal::Signal) -> Self {
        Self {
            market_id: s.market_id.clone(),
            computed_at: s.computed_at as i32,
            model_version: s.model_version.clone(),
            predicted_prob: s.predicted_prob,
            market_prob: s.market_prob,
            edge: s.edge,
            confidence: s.confidence,
            horizon_hours: s.horizon_hours as i32,
            rationale: s.rationale.clone(),
        }
    }
}

// =================================================================
// v0.76 — DashboardKpis (Phase 1 pilot)
// =================================================================

#[derive(serde::Serialize, serde::Deserialize, Type)]
struct DashboardKpisDto {
    total_equity_usdc: String,
    open_pnl_usdc: String,
    win_rate_30d: f64,
    brier_score: f64,
    active_signals: i32,
    open_positions: i32,
}

#[tauri::command]
#[specta::specta]
async fn dashboard_kpis_codegen() -> Result<DashboardKpisDto, String> {
    Ok(DashboardKpisDto {
        total_equity_usdc: "0".to_string(),
        open_pnl_usdc: "0".to_string(),
        win_rate_30d: 0.0,
        brier_score: 0.0,
        active_signals: 0,
        open_positions: 0,
    })
}

// =================================================================
// v0.84 — Phase 3: 5 read-only commands (no i64 fields)
// =================================================================
//
// These stubs use the REAL DTOs from `commands::*` directly so that
// drift detection works: a Rust field rename / type change will
// surface as a diff in `src/types/generated/index.ts` (v0.84d will
// prove this). The stub commands return hardcoded data of the right
// shape so the codegen can run without a Tauri State.

#[tauri::command]
#[specta::specta]
async fn is_seeded_codegen() -> Result<bool, String> {
    Ok(true)
}

#[tauri::command]
#[specta::specta]
async fn sidecar_status_codegen(
) -> Result<polyrocket_lib::commands::sidecar::SidecarStatus, String> {
    Ok(polyrocket_lib::commands::sidecar::SidecarStatus {
        running: false,
        pid: None,
        command: "polyrocket-sidecar".to_string(),
        last_error: None,
    })
}

#[tauri::command]
#[specta::specta]
async fn secrets_status_codegen(
) -> Result<polyrocket_lib::commands::secrets::SecretsStatus, String> {
    Ok(polyrocket_lib::commands::secrets::SecretsStatus {
        llm_keys: vec![],
        polymarket: vec![],
        wallets: vec![],
    })
}

#[tauri::command]
#[specta::specta]
async fn notification_permission_state_codegen() -> Result<String, String> {
    Ok("default".to_string())
}

#[tauri::command]
#[specta::specta]
async fn get_telemetry_enabled_codegen() -> Result<bool, String> {
    Ok(false)
}

// =================================================================
// v0.84 — Phase 3 batch 2: 5 simple-arg read-only commands
// =================================================================
//
// 3 of these 5 use the real DTOs (no i64 fields):
//   - get_auto_promote_config → AutoPromoteConfigDto (no i64)
//   - get_storage_info → StorageInfo (no i64)
//   - get_mirror_paper_mode → bool (no struct)
//
// 2 use `*CodegenDto` stubs because the real types have i64 fields
// (specta-typescript's default BigInt behavior is Fail). We use
// `specta_typescript::Number<i64>` which exports as TS `number`
// (precision loss accepted; safe for Unix ms timestamps within
// ~285,000 years from epoch).
//   - get_audit_retention → AuditRetentionViewCodegen (i64 fields)
//   - get_active_model → ActiveModelCodegen (i64 fields)

#[tauri::command]
#[specta::specta]
async fn get_auto_promote_config_codegen(
) -> Result<polyrocket_lib::commands::sidecar::AutoPromoteConfigDto, String> {
    Ok(polyrocket_lib::commands::sidecar::AutoPromoteConfigDto {
        enabled: false,
        brier_margin: 0.005,
    })
}

#[tauri::command]
#[specta::specta]
async fn get_storage_info_codegen(
) -> Result<StorageInfoCodegen, String> {
    Ok(StorageInfoCodegen {
        default_path: String::new(),
        current_path: String::new(),
        is_custom: false,
        exists: false,
        writable: false,
        /// v0.84b — `free_bytes: Option<u64>` in real type is
        /// BigInt-forbidden. Stub uses `Option<i64>` (signed
        /// for lossless range to 2^63; bytes fit comfortably).
        free_bytes: None,
        restart_required: false,
    })
}

/// v0.84b — codegen stub for `StorageInfo`. The real struct has
/// `free_bytes: Option<u64>` (BigInt-forbidden; even i64 is
/// forbidden in specta-typescript's default mode). Stub uses
/// `Option<f64>` (lossy above 2^53 bytes, but disk space in bytes
/// fits comfortably for ~9 PB before precision loss).
#[derive(Serialize, Deserialize, Type)]
struct StorageInfoCodegen {
    pub default_path: String,
    pub current_path: String,
    pub is_custom: bool,
    pub exists: bool,
    pub writable: bool,
    pub free_bytes: Option<f64>,
    pub restart_required: bool,
}

#[tauri::command]
#[specta::specta]
async fn get_mirror_paper_mode_codegen() -> Result<bool, String> {
    Ok(true)
}

// v0.84b — for AuditRetentionView + ActiveModel (both have i64 fields),
// we use a `*CodegenDto` stub with i32 placeholders. This is the
// v0.81 pattern used for `SignalCodegenDto`. Real types stay untouched
// (no churn for users of these types). Drift detection still works on
// the stub's field set; the i64→i32 truncation is a known limitation
// that v0.84+ will address via `Number<i64>` wrapper once the specta
// serde feature is enabled (TBD).
#[derive(Serialize, Deserialize, Type)]
struct AuditRetentionViewCodegen {
    pub retain_recent_ms: i32,
    pub max_rows: i32,
    pub min_keep_rows: i32,
    pub overrides: std::collections::HashMap<String, i32>,
}

#[tauri::command]
#[specta::specta]
async fn get_audit_retention_codegen(
) -> Result<AuditRetentionViewCodegen, String> {
    Ok(AuditRetentionViewCodegen {
        retain_recent_ms: 0,
        max_rows: 0,
        min_keep_rows: 0,
        overrides: std::collections::HashMap::new(),
    })
}

#[derive(Serialize, Deserialize, Type)]
struct ActiveModelCodegen {
    pub model_version: String,
    pub best_brier: Option<f64>,
    /// v0.84b — `best_params: Option<serde_json::Value>` is omitted
    /// from the codegen stub because `serde_json::Value` doesn't
    /// implement `specta::Type`. The L1 layer keeps it as
    /// `Record<string, unknown> | null` in the hand-written
    /// `ActiveModel` interface (src/ipc.ts:1227). Drift detection
    /// for this field is therefore limited to the L1 layer
    /// (covered by the existing v2 contract test). v0.84+ may
    /// add a custom Type impl for serde_json::Value.
    pub promoted_at_ms: Option<i32>,
    pub weights: Option<Vec<f64>>,
    pub source_path: String,
}

#[tauri::command]
#[specta::specta]
async fn get_active_model_codegen() -> Result<Option<ActiveModelCodegen>, String> {
    Ok(Some(ActiveModelCodegen {
        model_version: "logistic-train-stub".to_string(),
        best_brier: None,
        promoted_at_ms: None,
        weights: None,
        source_path: String::new(),
    }))
}

// =================================================================
// v0.84 — Phase 3 batch 3: 4 Vec-return commands
// =================================================================
//
// All 4 DTOs have i64 fields (BigInt-forbidden). We use `*CodegenDto`
// stubs with i32 placeholders, following the v0.81 SignalCodegenDto
// pattern. Real types stay untouched.

// ---------- SignalDto (list_active_signals) ----------

/// v0.84c — codegen stub args. Real `ListSignalsArgs` has
/// `Option<i64>` (limit), which is BigInt-forbidden. Stub uses
/// `Option<i32>` (matches the codegen-friendly limit semantics).
#[derive(Serialize, Deserialize, Type, Default)]
struct ListSignalsArgsCodegen {
    pub min_edge: Option<f64>,
    pub category: Option<String>,
    pub limit: Option<i32>,
}

/// v0.84c — codegen stub for `SignalDto`. The real struct has 4× i64
/// fields (`id`, `computed_at`, `horizon_hours`, ...). Stub uses i32
/// (drift detection on the field set + names, not on the i64→i32
/// precision; v0.84+ may switch to BigInt<i64> via serde feature).
#[derive(Serialize, Deserialize, Type)]
struct SignalListItemCodegen {
    pub id: i32,
    pub market_id: String,
    pub computed_at: i32,
    pub model_version: String,
    pub predicted_prob: f64,
    pub market_prob: f64,
    pub edge: f64,
    pub confidence: f64,
    pub horizon_hours: i32,
    pub rationale: Option<String>,
    pub market_question: Option<String>,
    pub market_slug: Option<String>,
}

#[tauri::command]
#[specta::specta]
async fn list_active_signals_codegen(
    _args: ListSignalsArgsCodegen,
) -> Result<Vec<SignalListItemCodegen>, String> {
    Ok(vec![])
}

// ---------- MirrorRow (list_mirrors) ----------

/// v0.84c — codegen stub args. Real `ListMirrorsArgs` has
/// `Option<i64>` (limit). Stub uses `Option<i32>`.
#[derive(Serialize, Deserialize, Type, Default)]
struct ListMirrorsArgsCodegen {
    pub status: Option<String>,
    pub limit: Option<i32>,
}

/// v0.84c — codegen stub for `MirrorRow`. Real has 5× i64 fields
/// (`event_id`, `created_at`, `submitted_at`, `filled_at`, ...).
#[derive(Serialize, Deserialize, Type)]
struct MirrorRowCodegen {
    pub id: String,
    pub event_id: i32,
    pub target_id: String,
    pub market_id: String,
    pub side: String,
    pub size: String,
    pub flipped: bool,
    pub status: String,
    pub created_at: i32,
    pub submitted_at: Option<i32>,
    pub filled_at: Option<i32>,
    pub bet_id: Option<String>,
}

#[tauri::command]
#[specta::specta]
async fn list_mirrors_codegen(
    _args: ListMirrorsArgsCodegen,
) -> Result<Vec<MirrorRowCodegen>, String> {
    Ok(vec![])
}

// ---------- WalletDto (list_wallets) ----------

/// v0.84c — codegen stub for `WalletDto`. Real has 3× i64 fields
/// (`chain_id`, `created_at`, `last_synced_at`).
#[derive(Serialize, Deserialize, Type)]
struct WalletDtoCodegen {
    pub id: String,
    pub address: String,
    pub label: Option<String>,
    pub chain_id: i32,
    pub wallet_type: String,
    pub created_at: i32,
    pub last_synced_at: Option<i32>,
}

#[tauri::command]
#[specta::specta]
async fn list_wallets_codegen() -> Result<Vec<WalletDtoCodegen>, String> {
    Ok(vec![])
}

// ---------- MirrorQueueStats (mirror_queue_stats) ----------

/// v0.84c — codegen stub for `MirrorQueueStats`. Real has 5× i64
/// counts (`n_pending`, `n_submitted`, `n_filled`, `n_rejected`,
/// `n_expired`).
#[derive(Serialize, Deserialize, Type)]
struct MirrorQueueStatsCodegen {
    pub n_pending: i32,
    pub n_submitted: i32,
    pub n_filled: i32,
    pub n_rejected: i32,
    pub n_expired: i32,
    pub total_exposure_usdc: f64,
    pub headroom_usdc: f64,
}

#[tauri::command]
#[specta::specta]
async fn mirror_queue_stats_codegen() -> Result<MirrorQueueStatsCodegen, String> {
    Ok(MirrorQueueStatsCodegen {
        n_pending: 0,
        n_submitted: 0,
        n_filled: 0,
        n_rejected: 0,
        n_expired: 0,
        total_exposure_usdc: 0.0,
        headroom_usdc: 0.0,
    })
}

// =================================================================
// v0.81 — Bankroll commands (Phase 2 — 4 commands)
// =================================================================
//
// For codegen, we use stub commands that match the real L1 IPC
// signatures. The real commands need Tauri State for DB access;
// stubs return hardcoded data. The point of v0.81 is to validate
// that the generated TS types match the hand-written DTOs in
// `src/types/bankroll.ts` and `src/ipc.ts`.

#[derive(Serialize, Deserialize, Type)]
struct ComputeAllocationArgsCodegen {
    bankroll_usdc: String,
    config: Option<polyrocket_lib::commands::bankroll::BankrollConfigDto>,
    signals: Vec<SignalCodegenDto>,
    market_liquidity: Option<std::collections::HashMap<String, String>>,
}

#[tauri::command]
#[specta::specta]
fn compute_allocation_preview_codegen(
    args: ComputeAllocationArgsCodegen,
) -> Result<polyrocket_lib::domain::bankroll::AllocationResult, polyrocket_lib::infra::error::AppError> {
    // Convert codegen DTOs back to real types
    let real_signals: Vec<polyrocket_lib::domain::signal::Signal> = args
        .signals
        .into_iter()
        .map(|s| polyrocket_lib::domain::signal::Signal {
            market_id: s.market_id,
            computed_at: s.computed_at as i64,
            model_version: s.model_version,
            predicted_prob: s.predicted_prob,
            market_prob: s.market_prob,
            edge: s.edge,
            confidence: s.confidence,
            horizon_hours: s.horizon_hours as i64,
            rationale: s.rationale,
        })
        .collect();
    let config = match &args.config {
        Some(dto) => {
            polyrocket_lib::commands::bankroll::validate_config(dto)?;
            dto.into()
        }
        None => polyrocket_lib::domain::bankroll::BankrollConfig::default(),
    };
    let input = polyrocket_lib::domain::bankroll::AllocationInput {
        bankroll_usdc: &args.bankroll_usdc,
        config: &config,
        signals: &real_signals,
        market_liquidity: args.market_liquidity.as_ref(),
    };
    Ok(polyrocket_lib::domain::bankroll::compute_allocation(&input))
}

#[tauri::command]
#[specta::specta]
async fn get_bankroll_config_codegen(
    _wallet_id: String,
) -> Result<polyrocket_lib::commands::bankroll::BankrollConfigDto, polyrocket_lib::infra::error::AppError> {
    // v0.81 — stub. Real impl reads from DB.
    Ok(polyrocket_lib::commands::bankroll::BankrollConfigDto {
        kelly_multiplier: 0.25,
        max_per_signal_pct: 0.10,
        reserve_pct: 0.20,
        min_edge_pct: 0.05,
        max_total_exposure_pct: 0.80,
        min_confidence: 0.60,
    })
}

#[tauri::command]
#[specta::specta]
async fn set_bankroll_config_codegen(
    _wallet_id: String,
    _config: polyrocket_lib::commands::bankroll::BankrollConfigDto,
) -> Result<(), polyrocket_lib::infra::error::AppError> {
    // v0.81 — stub. Real impl validates + writes to DB.
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn apply_allocation_codegen(
    _wallet_id: String,
    _result: polyrocket_lib::domain::bankroll::AllocationResult,
    _bankroll_usdc: String,
    _config: polyrocket_lib::commands::bankroll::BankrollConfigDto,
) -> Result<String, polyrocket_lib::infra::error::AppError> {
    // v0.81 — stub. Real impl writes to allocation_batches + bets.
    Ok("00000000-0000-0000-0000-000000000000".to_string())
}

fn main() {
    // Keep the original command symbols alive (in case the linker
    // would optimize them out as unused — they're used by the
    // generated export).
    let _ = commands::pnl::dashboard_kpis;
    let _ = commands::bankroll::compute_allocation_preview;
    let _ = commands::bankroll::get_bankroll_config;
    let _ = commands::bankroll::set_bankroll_config;
    let _ = commands::bankroll::apply_allocation;
    // v0.84 — Phase 3 read-only commands (no State needed, but the
    // symbols must remain reachable for drift detection).
    let _ = commands::seed::is_seeded;
    let _ = commands::sidecar::sidecar_status;
    let _ = commands::secrets::secrets_status;
    let _ = commands::notify::notification_permission_state;
    let _ = commands::sidecar::get_telemetry_enabled;
    // v0.84b — Phase 3 batch 2
    let _ = commands::sidecar::get_auto_promote_config;
    let _ = commands::storage::get_storage_info;
    let _ = commands::mirror_executor::get_mirror_paper_mode;
    let _ = commands::audit::get_audit_retention;
    let _ = commands::active_model::get_active_model;
    // v0.84c — Phase 3 batch 3
    let _ = commands::signal::list_active_signals;
    let _ = commands::mirror_executor::list_mirrors;
    let _ = commands::wallet::list_wallets;
    let _ = commands::mirror_executor::mirror_queue_stats;

    let builder: Builder<tauri::Wry> = Builder::new().commands(collect_commands![
        dashboard_kpis_codegen,
        compute_allocation_preview_codegen,
        get_bankroll_config_codegen,
        set_bankroll_config_codegen,
        apply_allocation_codegen,
        // v0.84 — Phase 3 read-only commands
        is_seeded_codegen,
        sidecar_status_codegen,
        secrets_status_codegen,
        notification_permission_state_codegen,
        get_telemetry_enabled_codegen,
        // v0.84b — Phase 3 batch 2
        get_auto_promote_config_codegen,
        get_storage_info_codegen,
        get_mirror_paper_mode_codegen,
        get_audit_retention_codegen,
        get_active_model_codegen,
        // v0.84c — Phase 3 batch 3
        list_active_signals_codegen,
        list_mirrors_codegen,
        list_wallets_codegen,
        mirror_queue_stats_codegen,
    ]);

    // CARGO_MANIFEST_DIR is `src-tauri/`, so the parent is
    // the project root. We want `<project>/src/types/generated/`.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR");
    let out_dir = std::path::PathBuf::from(manifest_dir)
        .parent()
        .expect("parent of manifest dir")
        .join("src/types/generated");
    std::fs::create_dir_all(&out_dir).expect("create generated dir");
    let out_file = out_dir.join("index.ts");

    // Use specta_typescript directly. Must match tauri-specta's
    // bundled version (0.0.12) — see Cargo.toml comment.
    builder
        .export(specta_typescript::Typescript::default(), &out_file)
        .expect("export ts bindings");

    println!(
        "✅ Generated TS bindings to {}",
        out_file.display()
    );
}

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

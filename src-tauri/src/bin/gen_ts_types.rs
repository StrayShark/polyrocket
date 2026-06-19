//! v0.76 — codegen Phase 1 binary.
//!
//! Generates TS bindings from `#[tauri::command] + #[specta::specta]`
//! -annotated commands. Phase 1 = proof-of-concept on a single
//! command (`dashboard_kpis`). Phase 2-5 will extend to all
//! 108 IPCs.
//!
//! Run: `cargo run --bin gen_ts_types`
//! Output: `src/types/generated/index.ts`
//!
//! See `docs/codegen-migration-plan.md` for the 5-phase plan.

use polyrocket_lib::commands;
use specta::Type;
use tauri_specta::{collect_commands, Builder};

/// v0.76 Phase 1 — proof of concept.
///
/// This bin generates TS bindings for ONE command
/// (`dashboard_kpis`). Phase 3-5 will expand to all
/// 108 IPCs. See `docs/codegen-migration-plan.md`.
///
/// **Known limitations**:
/// - AppError doesn't derive `specta::Type`, so we
///   wrap the result in a codegen-friendly type.
/// - Default TS export uses camelCase; polyrocket
///   uses snake_case. The drift is documented as
///   a Phase 2 follow-up.

#[derive(serde::Serialize, serde::Deserialize, Type)]
struct DashboardKpisDto {
    total_equity_usdc: String,
    open_pnl_usdc: String,
    win_rate_30d: f64,
    brier_score: f64,
    // v0.76 Phase 1 — i32 in the DTO. Real struct uses i64.
    // BigInt (i64/u64) requires opt-in via `BigInt` wrapper
    // or per-field `#[specta(type = BigInt)]`. Phase 3 will
    // either use the wrapper or add an export config that
    // maps i64→number. The polyrocket convention is "i64
    // serializes as number on the wire" — need a custom
    // `Type` impl for that to be reflected in the DTO.
    active_signals: i32,
    open_positions: i32,
}

#[tauri::command]
#[specta::specta]
async fn dashboard_kpis_codegen() -> Result<DashboardKpisDto, String> {
    // v0.76 — Phase 1: stub. Real impl in Phase 3.
    Ok(DashboardKpisDto {
        total_equity_usdc: "0".to_string(),
        open_pnl_usdc: "0".to_string(),
        win_rate_30d: 0.0,
        brier_score: 0.0,
        active_signals: 0,
        open_positions: 0,
    })
}

fn main() {
    let _ = commands::pnl::dashboard_kpis;
    let builder: Builder<tauri::Wry> = Builder::new()
        .commands(collect_commands![dashboard_kpis_codegen]);

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

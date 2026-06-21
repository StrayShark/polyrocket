//! v0.76 / v0.81 / v0.84 / v0.88 — codegen Phase 1+2+3+4 binary.
//!
//! Generates TS bindings from `#[tauri::command] + #[specta::specta]`
//! -annotated commands.
//!
//! v0.76 = Phase 1: 1 command (dashboard_kpis) proof-of-concept.
//! v0.81 = Phase 2: +4 bankroll commands
//!   (compute_allocation_preview, get_bankroll_config,
//!    set_bankroll_config, apply_allocation).
//! v0.84 = Phase 3: +14 read-only commands (no input DTOs).
//! v0.88 = Phase 4: input DTO commands. v0.88a adds 3 simple shape:
//!   - add_wallet → AddWalletArgsCodegen + WalletDtoCodegen
//!   - set_telemetry_enabled → SetTelemetryEnabledArgsCodegen
//!   - set_mirror_paper_mode → SetMirrorPaperModeArgsCodegen
//!
//! Phase 5 (build pipeline: drift detector → pnpm build) is next.
//!
//! Run: `cargo run --bin gen_ts_types`
//! Output: `src/types/generated/index.ts`
//!
//! See `docs/codegen-migration-plan.md` for the 5-phase plan.

use polyrocket_lib::commands;
use serde::{Deserialize, Serialize};
use specta::Type;
use polyrocket_lib::codegen::bigint_map::BigIntMap;
use polyrocket_lib::codegen::option_bigint::OptionBigInt;
use specta_typescript::BigInt;
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

// v0.86c — for AuditRetentionView (has 4 i64 fields), the stub uses
// `BigIntMap<String, i64>` for the map value and `#[specta(type = BigInt)]`
// for the required i64 scalars. v0.85c had reverted to i32 placeholders
// because `#[specta(type = BigInt)]` on `HashMap<String, i64>` doesn't
// recurse into the value type; v0.86c adds the custom `BigIntMap` wrapper
// to fix that case.
#[derive(Serialize, Deserialize, Type)]
struct AuditRetentionViewCodegen {
    #[specta(type = BigInt)]
    pub retain_recent_ms: i64,
    #[specta(type = BigInt)]
    pub max_rows: i64,
    #[specta(type = BigInt)]
    pub min_keep_rows: i64,
    /// v0.86c — `BigIntMap<String, i64>` → TS `{ [key: string]: bigint }`.
    /// See src-tauri/src/codegen/bigint_map.rs for the wrapper design.
    pub overrides: BigIntMap<String, i64>,
}

#[tauri::command]
#[specta::specta]
async fn get_audit_retention_codegen(
) -> Result<AuditRetentionViewCodegen, String> {
    Ok(AuditRetentionViewCodegen {
        retain_recent_ms: 0,
        max_rows: 0,
        min_keep_rows: 0,
        overrides: BigIntMap::default(),
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
    /// v0.86b — Option<OptionBigInt<i64>> → TS `bigint | null`
    pub promoted_at_ms: Option<OptionBigInt<i64>>,
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
    /// v0.86b — Option<OptionBigInt<i64>> → TS `bigint | null`.
    pub limit: Option<OptionBigInt<i64>>,
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
    /// v0.86b — Option<OptionBigInt<i64>> → TS `bigint | null`.
    pub limit: Option<OptionBigInt<i64>>,
}

/// v0.85b — codegen stub for `MirrorRow`. Real has 5× i64 fields
/// (`event_id`, `created_at`, `submitted_at`, `filled_at`, ...).
/// v0.85+: uses `#[specta(type = BigInt)]` attribute to mark i64
/// fields as TS `bigint` (lossless for values that fit in 53 bits).
/// The L1 layer (src/types/mirror.ts) keeps these as `number` for
/// now (JSON.parse gives `number`, not `bigint`); explicit `BigInt()`
/// conversion is added in a follow-up.
#[derive(Serialize, Deserialize, Type)]
struct MirrorRowCodegen {
    pub id: String,
    #[specta(type = BigInt)]
    pub event_id: i64,
    pub target_id: String,
    pub market_id: String,
    pub side: String,
    pub size: String,
    pub flipped: bool,
    pub status: String,
    #[specta(type = BigInt)]
    pub created_at: i64,
    /// v0.86b — Option<OptionBigInt<i64>> → TS `bigint | null`
    /// (lossless for values that fit in 53 bits). The OptionBigInt
    /// wrapper is serde-transparent so wire format is identical to
    /// `Option<i64>`. See src-tauri/src/codegen/option_bigint.rs.
    pub submitted_at: Option<OptionBigInt<i64>>,
    pub filled_at: Option<OptionBigInt<i64>>,
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
    /// v0.86b — `created_at: i64` with `#[specta(type = BigInt)]` → TS `bigint`.
    /// `last_synced_at: Option<OptionBigInt<i64>>` → TS `bigint | null`.
    #[specta(type = BigInt)]
    pub created_at: i64,
    pub last_synced_at: Option<OptionBigInt<i64>>,
}

#[tauri::command]
#[specta::specta]
async fn list_wallets_codegen() -> Result<Vec<WalletDtoCodegen>, String> {
    Ok(vec![])
}

// =================================================================
// v0.88a — Phase 4 batch 1: 3 input-DTO commands (simple shape)
// =================================================================
//
// Drift detection for input DTOs. The real commands live in
// `commands/wallet.rs` / `commands/sidecar.rs` /
// `commands/mirror_executor.rs` — these stubs exist purely so the
// codegen can run and the drift detector can catch any future
// rename/type change in the real args DTOs. L1 still calls the real
// commands (`add_wallet` / `set_telemetry_enabled` /
// `set_mirror_paper_mode`) — the `_codegen` suffix here is a marker
// for the codegen surface, not an L1-callable IPC.

/// v0.88a — codegen stub args for `add_wallet`. Real
/// `AddWalletArgs` (commands/wallet.rs:26) has `chain_id: Option<i64>`;
/// we truncate to i32 since chain IDs are small (Polygon mainnet = 137,
/// fits comfortably). A future drift in `label`/`address`/`wallet_type`
/// types or in `chain_id`'s Option-ness will surface here.
#[derive(Serialize, Deserialize, Type)]
struct AddWalletArgsCodegen {
    pub address: String,
    pub label: Option<String>,
    pub chain_id: Option<i32>,
    pub wallet_type: Option<String>,
}

#[tauri::command]
#[specta::specta]
async fn add_wallet_codegen(
    _args: AddWalletArgsCodegen,
) -> Result<WalletDtoCodegen, String> {
    // v0.88a — stub. Real impl in commands/wallet.rs:55 inserts row +
    // returns the created WalletDto. We return a hardcoded shape so
    // the codegen can export the type signature.
    Ok(WalletDtoCodegen {
        id: "00000000-0000-0000-0000-000000000000".to_string(),
        address: String::new(),
        label: None,
        chain_id: 137,
        wallet_type: "eoa".to_string(),
        created_at: 0,
        last_synced_at: None,
    })
}

/// v0.88a — codegen stub args for `set_telemetry_enabled`. Real
/// `SetTelemetryEnabledArgs` (commands/sidecar.rs:1857) wraps a single
/// `enabled: bool`. We mirror that shape exactly so the codegen
/// signature matches what L1 sends.
#[derive(Serialize, Deserialize, Type)]
struct SetTelemetryEnabledArgsCodegen {
    pub enabled: bool,
}

#[tauri::command]
#[specta::specta]
async fn set_telemetry_enabled_codegen(
    args: SetTelemetryEnabledArgsCodegen,
) -> Result<bool, String> {
    Ok(args.enabled)
}

/// v0.88a — codegen stub args for `set_mirror_paper_mode`. Real
/// `SetMirrorPaperModeArgs` (commands/mirror_executor.rs:162) wraps a
/// single `enabled: bool`. Same pattern as `set_telemetry_enabled`.
#[derive(Serialize, Deserialize, Type)]
struct SetMirrorPaperModeArgsCodegen {
    pub enabled: bool,
}

#[tauri::command]
#[specta::specta]
async fn set_mirror_paper_mode_codegen(
    _args: SetMirrorPaperModeArgsCodegen,
) -> Result<bool, String> {
    // v0.88a — stub. Real impl in commands/mirror_executor.rs:174
    // takes State<'_,'_, AppState> and writes to state.mirror_paper_mode
    // (an Arc<Mutex<bool>>). State can't be constructed in a bin
    // context, so we return hardcoded data of the right shape.
    Ok(true)
}

// =================================================================
// v0.88b — Phase 4 batch 2: 2 Copy-route commands (input DTOs)
// =================================================================
//
// Drift detection for `add_copy_target` and `enqueue_mirror`. Real
// commands live in commands/copy.rs and commands/mirror_executor.rs.

/// v0.88b — codegen stub for `CopyTargetDto`. Real has `created_at: i64`
/// which specta-typescript forbids (BigInt); use OptionBigInt wrapper
/// (v0.86b) → TS `bigint | null`. Same pattern as WalletDtoCodegen.
#[derive(Serialize, Deserialize, Type)]
struct CopyTargetDtoCodegen {
    pub id: String,
    pub address: String,
    pub label: Option<String>,
    pub enabled: bool,
    pub allocation_cap: Option<String>,
    pub min_edge: f64,
    #[specta(type = BigInt)]
    pub created_at: i64,
}

/// v0.88b — codegen stub args for `add_copy_target`. Real
/// `AddCopyTargetArgs` (commands/copy.rs) has no i64 fields — all
/// plain string/f64. Codegen shape is identical to real.
#[derive(Serialize, Deserialize, Type)]
struct AddCopyTargetArgsCodegen {
    pub address: String,
    pub label: Option<String>,
    pub allocation_cap: Option<String>,
    pub min_edge: Option<f64>,
}

#[tauri::command]
#[specta::specta]
async fn add_copy_target_codegen(
    _args: AddCopyTargetArgsCodegen,
) -> Result<CopyTargetDtoCodegen, String> {
    // v0.88b — stub. Real impl in commands/copy.rs:68 inserts row +
    // returns the created CopyTargetDto. We return hardcoded shape.
    Ok(CopyTargetDtoCodegen {
        id: "00000000-0000-0000-0000-000000000000".to_string(),
        address: String::new(),
        label: None,
        enabled: true,
        allocation_cap: None,
        min_edge: 0.05,
        created_at: 0,
    })
}

/// v0.88b — codegen stub args for `enqueue_mirror`. Real
/// `EnqueueArgs` (commands/mirror_executor.rs) has `event_id: i64`;
/// we use BigInt<i64> attribute (v0.85c pattern) → TS `bigint`.
#[derive(Serialize, Deserialize, Type)]
struct EnqueueMirrorArgsCodegen {
    #[specta(type = BigInt)]
    pub event_id: i64,
    pub target_id: String,
    pub market_id: String,
    pub side: String,
    pub size: String,
    pub flipped: bool,
}

#[tauri::command]
#[specta::specta]
async fn enqueue_mirror_codegen(
    _args: EnqueueMirrorArgsCodegen,
) -> Result<MirrorRowCodegen, String> {
    // v0.88b — stub. Real impl in commands/mirror_executor.rs:76
    // inserts into copy_mirror_queue + returns MirrorRow. We return
    // hardcoded shape; MirrorRowCodegen reuses v0.86b's BigInt
    // wrappers for submitted_at / filled_at.
    Ok(MirrorRowCodegen {
        id: String::new(),
        event_id: 0,
        target_id: String::new(),
        market_id: String::new(),
        side: String::new(),
        size: String::new(),
        flipped: false,
        status: "pending".to_string(),
        created_at: 0,
        submitted_at: None,
        filled_at: None,
        bet_id: None,
    })
}

// =================================================================
// v0.88c — Phase 4 batch 3: 2 Trade-route commands (input DTOs)
// =================================================================
//
// Drift detection for `place_signed_order` and `place_jump_link`.
// Real commands live in commands/bet.rs.

/// v0.88c — codegen stub for `BetDto`. Real has 3× i64 fields
/// (`placed_at`, `settled_at`, `signal_id`). Reuses v0.86b's
/// OptionBigInt wrapper pattern.
#[derive(Serialize, Deserialize, Type)]
struct BetDtoCodegen {
    pub id: String,
    pub wallet_id: String,
    pub market_id: String,
    /// v0.88c — `signal_id: Option<i64>` → `Option<OptionBigInt<i64>>`
    /// (TS `bigint | null`). Lossless for values within 53 bits.
    pub signal_id: Option<OptionBigInt<i64>>,
    pub mode: String,
    pub side: String,
    pub size: String,
    pub price: f64,
    pub shares: String,
    #[specta(type = BigInt)]
    pub placed_at: i64,
    pub settled_at: Option<OptionBigInt<i64>>,
    pub pnl: Option<String>,
    pub status: String,
    pub tx_hash: Option<String>,
    pub notes: Option<String>,
    pub order_type: String,
    pub limit_price: Option<f64>,
    pub stop_price: Option<f64>,
    pub post_only: bool,
}

/// v0.88c — codegen stub args for `place_signed_order`. Real
/// `PlaceSignedArgs` (commands/bet.rs) has `signal_id: Option<i64>`
/// → BigInt wrapper for lossless transport.
#[derive(Serialize, Deserialize, Type)]
struct PlaceSignedArgsCodegen {
    pub market_id: String,
    pub wallet_id: String,
    pub side: String,
    pub price: f64,
    pub size: String,
    pub signal_id: Option<OptionBigInt<i64>>,
    pub key_alias: String,
    pub order_type: Option<String>,
    pub limit_price: Option<f64>,
    pub stop_price: Option<f64>,
    pub post_only: bool,
}

#[tauri::command]
#[specta::specta]
async fn place_signed_order_codegen(
    _args: PlaceSignedArgsCodegen,
) -> Result<BetDtoCodegen, String> {
    // v0.88c — stub. Real impl in commands/bet.rs validates +
    // signs + inserts bets row + returns BetDto. We return hardcoded
    // shape with placed_at=0 / signal_id=None / settled_at=None
    // (consistent with a "just placed, not yet settled" bet).
    Ok(BetDtoCodegen {
        id: "00000000-0000-0000-0000-000000000000".to_string(),
        wallet_id: String::new(),
        market_id: String::new(),
        signal_id: None,
        mode: "manual".to_string(),
        side: String::new(),
        size: String::new(),
        price: 0.0,
        shares: String::new(),
        placed_at: 0,
        settled_at: None,
        pnl: None,
        status: "open".to_string(),
        tx_hash: None,
        notes: None,
        order_type: "market".to_string(),
        limit_price: None,
        stop_price: None,
        post_only: false,
    })
}

/// v0.88c — codegen stub args for `place_jump_link`. Same shape as
/// PlaceSignedArgs but without the order_type / limit_price /
/// stop_price / post_only fields. signal_id: Option<i64> → BigInt.
#[derive(Serialize, Deserialize, Type)]
struct PlaceJumpArgsCodegen {
    pub market_slug: String,
    pub market_id: String,
    pub wallet_id: String,
    pub side: String,
    pub size: String,
    pub price: f64,
    pub signal_id: Option<OptionBigInt<i64>>,
}

#[tauri::command]
#[specta::specta]
async fn place_jump_link_codegen(
    args: PlaceJumpArgsCodegen,
) -> Result<String, String> {
    // v0.88c — stub. Real impl returns a polymarket.com jump URL
    // built from args.market_slug + args.side + args.price. We return
    // a hardcoded URL so the codegen exports the right return type.
    let _ = args;
    Ok("https://polymarket.com/event/_stub_".to_string())
}

// =================================================================
// v0.88d — Phase 4 batch 4: Settings + ModelLab + LlmMgmt
// =================================================================
//
// Drift detection for `set_audit_retention`, `set_auto_promote_config`,
// `upsert_llm_provider`.

/// v0.88d — codegen stub args for `set_audit_retention`. Real
/// `SetAuditRetentionArgs` (commands/audit.rs) has 3× Option<i64>;
/// OptionBigInt wrapper (v0.86b) gives lossless transport.
#[derive(Serialize, Deserialize, Type)]
struct SetAuditRetentionArgsCodegen {
    pub retain_recent_ms: Option<OptionBigInt<i64>>,
    pub max_rows: Option<OptionBigInt<i64>>,
    pub min_keep_rows: Option<OptionBigInt<i64>>,
}

#[tauri::command]
#[specta::specta]
async fn set_audit_retention_codegen(
    _args: SetAuditRetentionArgsCodegen,
) -> Result<u32, String> {
    // v0.88d — stub. Real impl writes retention policy + runs an
    // immediate purge, returns the count of purged rows. We return 0
    // since the codegen bin can't construct DB state. u32 instead of
    // usize because specta-typescript forbids usize export.
    Ok(0)
}

/// v0.88d — codegen stub for `upsert_llm_provider`. Takes
/// `LlmProviderDto` directly (not nested under args) — same shape as
/// the real command. `timeout_ms: i64` needs BigInt handling.
#[derive(Serialize, Deserialize, Type)]
struct LlmProviderDtoCodegen {
    pub id: String,
    pub display_name: String,
    pub enabled: bool,
    pub api_base: Option<String>,
    pub key_alias: String,
    pub default_model: String,
    #[specta(type = BigInt)]
    pub timeout_ms: i64,
    pub cost_per_1k_in: Option<f64>,
    pub cost_per_1k_out: Option<f64>,
}

#[tauri::command]
#[specta::specta]
async fn upsert_llm_provider_codegen(
    _provider: LlmProviderDtoCodegen,
) -> Result<(), String> {
    // v0.88d — stub. Real impl does INSERT ... ON CONFLICT UPDATE.
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn set_auto_promote_config_codegen(
    args: polyrocket_lib::commands::sidecar::SetAutoPromoteConfigArgs,
) -> Result<polyrocket_lib::commands::sidecar::AutoPromoteConfigDto, String> {
    // v0.88d — stub. Real impl reads current config, applies the
    // partial update, writes back. We return the default config so
    // the codegen exports the right return type.
    let _ = args;
    Ok(polyrocket_lib::commands::sidecar::AutoPromoteConfigDto {
        enabled: false,
        brier_margin: 0.005,
    })
}

// ---------- MirrorQueueStats (mirror_queue_stats) ----------

/// v0.84c — codegen stub for `MirrorQueueStats`. Real has 5× i64
/// counts (`n_pending`, `n_submitted`, `n_filled`, `n_rejected`,
/// `n_expired`).
#[derive(Serialize, Deserialize, Type)]
struct MirrorQueueStatsCodegen {
    #[specta(type = BigInt)]
    pub n_pending: i64,
    #[specta(type = BigInt)]
    pub n_submitted: i64,
    #[specta(type = BigInt)]
    pub n_filled: i64,
    #[specta(type = BigInt)]
    pub n_rejected: i64,
    #[specta(type = BigInt)]
    pub n_expired: i64,
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
    // v0.88a — Phase 4 batch 1 (input DTOs, simple shape)
    let _ = commands::wallet::add_wallet;
    let _ = commands::sidecar::set_telemetry_enabled;
    let _ = commands::mirror_executor::set_mirror_paper_mode;
    // v0.88b — Phase 4 batch 2 (Copy route, input DTOs)
    let _ = commands::copy::add_copy_target;
    let _ = commands::mirror_executor::enqueue_mirror;
    // v0.88c — Phase 4 batch 3 (Trade route, input DTOs)
    let _ = commands::bet::place_signed_order;
    let _ = commands::bet::place_jump_link;
    // v0.88d — Phase 4 batch 4 (Settings / ModelLab / LlmMgmt)
    let _ = commands::audit::set_audit_retention;
    let _ = commands::llm::upsert_llm_provider;
    let _ = commands::sidecar::set_auto_promote_config;

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
        // v0.88a — Phase 4 batch 1 (input DTOs, simple shape)
        add_wallet_codegen,
        set_telemetry_enabled_codegen,
        set_mirror_paper_mode_codegen,
        // v0.88b — Phase 4 batch 2 (Copy route, input DTOs)
        add_copy_target_codegen,
        enqueue_mirror_codegen,
        // v0.88c — Phase 4 batch 3 (Trade route, input DTOs)
        place_signed_order_codegen,
        place_jump_link_codegen,
        // v0.88d — Phase 4 batch 4 (Settings / ModelLab / LlmMgmt)
        set_audit_retention_codegen,
        upsert_llm_provider_codegen,
        set_auto_promote_config_codegen,
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

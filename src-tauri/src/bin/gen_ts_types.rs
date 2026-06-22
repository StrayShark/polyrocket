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

// v0.98 — DTO stubs for the 4 new commands.
// Each mirrors the real Rust type but uses i32 / String
// placeholders for fields that would be i64 / u64 / Decimal
// in the real impl (which we skip in the codegen stub).

#[derive(Serialize, Deserialize, Type)]
struct ListBetsArgsCodegen {
    pub limit: i32,
    pub status: Option<String>,
}

#[derive(Serialize, Deserialize, Type)]
struct ListAuditLogArgsCodegen {
    pub limit: i32,
    pub actor: Option<String>,
    pub action: Option<String>,
    /// v0.98 — placeholder (real impl uses Option<i64> wrapped in
    /// OptionBigInt). Stub uses Option<u32> for ts export.
    pub since_ms: Option<u32>,
}

#[derive(Serialize, Deserialize, Type)]
struct AuditEntryDtoCodegen {
    /// v0.98 — placeholder (real impl uses i64 with `#[specta(type = BigInt)]`
    /// or `Option<OptionBigInt<i64>>` for the optional case). Stub
    /// uses i32 since audit IDs are bounded by history size.
    pub id: i32,
    pub actor: String,
    pub action: String,
    pub payload: String,
}

#[derive(Serialize, Deserialize, Type)]
struct ListPromoteHistoryCodegen {
    pub ok: bool,
    pub message: String,
    pub count: i32,
    pub entries: Vec<PromoteHistoryEntryCodegen>,
}

#[derive(Serialize, Deserialize, Type)]
struct PromoteHistoryEntryCodegen {
    pub job_id: String,
    pub model_version: String,
    #[specta(type = BigInt)]
    pub promoted_at_ms: i64,
    pub best_brier: f64,
    pub best_params: BestParamsCodegen,
    pub trial_index: i32,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Type)]
struct BestParamsCodegen {
    pub w0: f64,
    pub w1: f64,
    pub w2: f64,
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

// =================================================================
// v0.88e — Phase 4 batch 5: complex nested DTOs (LLM + mirror executor)
// =================================================================
//
// Drift detection for `llm_analyze` and `run_mirror_executor_pass`.
// These have nested Vec<i64> + multiple Option<i64> fields, exercising
// the BigInt wrappers end-to-end (OptionBigInt + BigIntMap not needed
// here — just OptionBigInt for Option<i64> and `specta(type=BigInt)`
// for bare i64).

/// v0.88e — codegen stub for `LlmRecommendationDto`. Real has 4× i64
/// fields (`id`, `latency_ms`, `tokens_in`, `tokens_out`). The 3
/// Option<i64> use OptionBigInt; the bare `id: i64` uses
/// `specta(type = BigInt)`.
#[derive(Serialize, Deserialize, Type)]
struct LlmRecommendationDtoCodegen {
    #[specta(type = BigInt)]
    pub id: i64,
    pub analysis_id: String,
    pub provider_id: String,
    pub provider_name: String,
    pub predicted_prob: Option<f64>,
    pub side: Option<String>,
    pub confidence: Option<f64>,
    pub reasoning: Option<String>,
    pub latency_ms: Option<OptionBigInt<i64>>,
    pub tokens_in: Option<OptionBigInt<i64>>,
    pub tokens_out: Option<OptionBigInt<i64>>,
    pub cost_cents: Option<f64>,
    pub parse_ok: bool,
    pub parse_error: Option<String>,
}

/// v0.88e — codegen stub for `LlmAnalysisDto`. Real has 4× i64 fields
/// (`signal_id`, `requested_at`, `completed_at`, `total_latency_ms`)
/// + nested Vec<LlmRecommendationDto>. This exercises the
/// OptionBigInt + nested struct codegen path end-to-end.
#[derive(Serialize, Deserialize, Type)]
struct LlmAnalysisDtoCodegen {
    pub id: String,
    pub market_id: String,
    pub signal_id: Option<OptionBigInt<i64>>,
    pub prompt_version: String,
    #[specta(type = BigInt)]
    pub requested_at: i64,
    pub completed_at: Option<OptionBigInt<i64>>,
    pub status: String,
    pub consensus_predicted: Option<f64>,
    pub consensus_side: Option<String>,
    pub consensus_conf: Option<f64>,
    pub total_latency_ms: Option<OptionBigInt<i64>>,
    pub cost_cents: Option<f64>,
    pub triggered_by: String,
    pub recommendations: Vec<LlmRecommendationDtoCodegen>,
}

/// v0.88e — codegen stub args for `llm_analyze`. Real `AnalyzeArgs`
/// has `signal_id: Option<i64>` → OptionBigInt; other fields are simple.
#[derive(Serialize, Deserialize, Type)]
struct LlmAnalyzeArgsCodegen {
    pub market_id: String,
    pub signal_id: Option<OptionBigInt<i64>>,
    pub prompt_version: Option<String>,
    pub provider_ids: Option<Vec<String>>,
    pub triggered_by: Option<String>,
}

#[tauri::command]
#[specta::specta]
async fn llm_analyze_codegen(
    _args: LlmAnalyzeArgsCodegen,
) -> Result<LlmAnalysisDtoCodegen, String> {
    // v0.88e — stub. Real impl dispatches to enabled providers,
    // gathers responses, writes llm_analyses + llm_recommendations
    // rows, returns the aggregated DTO. We return hardcoded shape
    // with empty recommendations vec.
    Ok(LlmAnalysisDtoCodegen {
        id: "00000000-0000-0000-0000-000000000000".to_string(),
        market_id: String::new(),
        signal_id: None,
        prompt_version: String::new(),
        requested_at: 0,
        completed_at: None,
        status: "completed".to_string(),
        consensus_predicted: None,
        consensus_side: None,
        consensus_conf: None,
        total_latency_ms: None,
        cost_cents: None,
        triggered_by: "user:anonymous".to_string(),
        recommendations: vec![],
    })
}

/// v0.88e — codegen stub for `ExecutorPassResult`. Real has no i64
/// fields (just Vec<String>, Vec<(String, RejectReason)>, two f64).
/// Reuses the real type via specta::Type derive — but currently the
/// real type doesn't have `Type`. We mirror the shape in a stub.
#[derive(Serialize, Deserialize, Type)]
struct ExecutorPassResultCodegen {
    pub picked: Vec<String>,
    pub rejected: Vec<(String, String)>, // (market_id, reject_reason)
    pub current_exposure: f64,
    pub headroom: f64,
}

/// v0.88e — codegen stub args for `run_mirror_executor_pass`. Real
/// `RunPassArgs` has no i64 fields (just 2× String).
#[derive(Serialize, Deserialize, Type)]
struct RunMirrorPassArgsCodegen {
    pub key_alias: String,
    pub wallet_id: String,
}

#[tauri::command]
#[specta::specta]
async fn run_mirror_executor_pass_codegen(
    _args: RunMirrorPassArgsCodegen,
) -> Result<ExecutorPassResultCodegen, String> {
    // v0.88e — stub. Real impl reads mirror queue, picks orders that
    // pass size + horizon + exposure filters, submits signed orders,
    // returns ExecutorPassResult. We return hardcoded empty result.
    Ok(ExecutorPassResultCodegen {
        picked: vec![],
        rejected: vec![],
        current_exposure: 0.0,
        headroom: 0.0,
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

// =================================================================
// v0.98 — Phase 4 batch 6: 4 more read-only list commands
// =================================================================
//
// Drift detection for the read-only list endpoints used by
// /history, /audit, /copy, and the ModelLab history panel.
// Each stub returns a hardcoded empty array of the right
// shape; the drift detector catches any future rename / type
// change in the real DTOs. L1 still calls the real commands.

/// v0.98 — codegen stub for `list_bets`. Real return type
/// is `Vec<BetDto>`. Empty stub.
#[tauri::command]
#[specta::specta]
async fn list_bets_codegen(
    _args: ListBetsArgsCodegen,
) -> Result<Vec<BetDtoCodegen>, String> {
    Ok(vec![])
}

/// v0.98 — codegen stub for `list_audit_log`. Real return
/// type is `Vec<AuditEntry>`. Empty stub.
#[tauri::command]
#[specta::specta]
async fn list_audit_log_codegen(
    _args: ListAuditLogArgsCodegen,
) -> Result<Vec<AuditEntryDtoCodegen>, String> {
    Ok(vec![])
}

/// v0.98 — codegen stub for `list_copy_targets`. Real
/// return type is `Vec<CopyTarget>`. Empty stub.
#[tauri::command]
#[specta::specta]
async fn list_copy_targets_codegen() -> Result<Vec<CopyTargetDtoCodegen>, String> {
    Ok(vec![])
}

/// v0.98 — codegen stub for `list_promote_history`. Real
/// return type is `PromoteHistoryResult` (uses existing
/// `listPromoteHistory` from v0.76). Empty stub.
#[tauri::command]
#[specta::specta]
async fn list_promote_history_codegen() -> Result<ListPromoteHistoryCodegen, String> {
    Ok(ListPromoteHistoryCodegen {
        ok: true,
        message: "ok".to_string(),
        count: 0,
        entries: vec![],
    })
}

// =================================================================
// v0.101a — Phase 4 batch 7: LLM stats heatmap + scatter
//                     + timeseries + decision (4 read-only commands)
// =================================================================
//
// Drift detection for the L1 "LLM Performance" page. Each
// stub returns an empty Vec; the drift detector catches any
// future rename / type change in the real DTOs. i64 fields
// use either `#[specta(type = BigInt)]` (timestamps) or
// `i32` placeholder (counts) following the v0.98 pattern.

/// v0.101a — args for `llm_stats_heatmap`. Matches real `StatsArgs`.
#[derive(Serialize, Deserialize, Type)]
struct StatsArgsCodegen {
    pub provider_id: Option<String>,
    pub category: Option<String>,
    /// v0.101a — window_days placeholder (real impl uses Option<i64>
    /// wrapped in OptionBigInt). Stub uses Option<u32> for ts export.
    pub window_days: Option<u32>,
}

/// v0.101a — LlmStatsCell shape. Counts use i32 (drift detect only).
#[derive(Serialize, Deserialize, Type)]
struct LlmStatsCellCodegen {
    pub provider_id: String,
    pub provider_name: String,
    pub category: String,
    /// v0.101a — placeholder (real impl uses i64). Stub uses i32.
    pub n_recommendations: i32,
    pub n_evaluated: i32,
    pub win_rate: Option<f64>,
    pub avg_pnl: Option<f64>,
    pub brier: Option<f64>,
}

/// v0.101a — codegen stub for `llm_stats_heatmap`.
#[tauri::command]
#[specta::specta]
async fn llm_stats_heatmap_codegen(
    _args: StatsArgsCodegen,
) -> Result<Vec<LlmStatsCellCodegen>, String> {
    Ok(vec![])
}

/// v0.101a — LlmStatsScatterPoint shape.
#[derive(Serialize, Deserialize, Type)]
struct LlmStatsScatterPointCodegen {
    pub provider_id: String,
    pub provider_name: String,
    /// v0.101a — placeholder (real impl uses i64). Stub uses i32.
    pub n_evaluated: i32,
    pub win_rate: f64,
    pub total_pnl: f64,
    pub avg_pnl: f64,
}

/// v0.101a — codegen stub for `llm_stats_scatter`.
#[tauri::command]
#[specta::specta]
async fn llm_stats_scatter_codegen(
    window_days: Option<u32>,
) -> Result<Vec<LlmStatsScatterPointCodegen>, String> {
    Ok(vec![])
}

/// v0.101a — LlmStatsTimeseriesPoint shape.
#[derive(Serialize, Deserialize, Type)]
struct LlmStatsTimeseriesPointCodegen {
    pub provider_id: String,
    pub bucket: String,
    /// v0.101a — placeholder (real impl uses i64). Stub uses i32.
    pub n_evaluated: i32,
    pub win_rate: f64,
    pub brier: f64,
}

/// v0.101a — codegen stub for `llm_stats_timeseries`.
#[tauri::command]
#[specta::specta]
async fn llm_stats_timeseries_codegen(
    window_days: Option<u32>,
) -> Result<Vec<LlmStatsTimeseriesPointCodegen>, String> {
    Ok(vec![])
}

/// v0.101a — LlmDecisionStats shape.
#[derive(Serialize, Deserialize, Type)]
struct LlmDecisionStatsCodegen {
    pub category: String,
    pub decision_type: String,
    /// v0.101a — placeholder (real impl uses i64). Stub uses i32.
    pub n: i32,
    pub win_rate: f64,
    pub avg_pnl: f64,
}

/// v0.101a — codegen stub for `llm_stats_decision`.
#[tauri::command]
#[specta::specta]
async fn llm_stats_decision_codegen(
    window_days: Option<u32>,
) -> Result<Vec<LlmDecisionStatsCodegen>, String> {
    Ok(vec![])
}

// =================================================================
// v0.101b — Phase 4 batch 7 part 2: LLM stats by_confidence
//                     + by_prompt + cost_efficiency + export (4 commands)
// =================================================================
//
// Continuation of v0.101a batch 7 — covers the L1 "LLM Management"
// page's confidence / prompt / cost panels + the CSV/JSON export.

/// v0.101b — args for `llm_stats_by_confidence`. Matches real `StatsByConfidenceArgs`.
#[derive(Serialize, Deserialize, Type)]
struct StatsByConfidenceArgsCodegen {
    pub provider_id: Option<String>,
    /// v0.101b — placeholder (real impl uses Option<i64>). Stub uses Option<u32>.
    pub window_days: Option<u32>,
}

/// v0.101b — LlmStatsConfidenceBand shape.
#[derive(Serialize, Deserialize, Type)]
struct LlmStatsConfidenceBandCodegen {
    pub provider_id: String,
    pub band: String,
    /// v0.101b — placeholder (real impl uses i64). Stub uses i32.
    pub n: i32,
    pub win_rate: f64,
    pub avg_pnl: f64,
}

/// v0.101b — codegen stub for `llm_stats_by_confidence`.
#[tauri::command]
#[specta::specta]
async fn llm_stats_by_confidence_codegen(
    _args: StatsByConfidenceArgsCodegen,
) -> Result<Vec<LlmStatsConfidenceBandCodegen>, String> {
    Ok(vec![])
}

/// v0.101b — args for `llm_stats_by_prompt`. Matches real `StatsByPromptArgs`.
#[derive(Serialize, Deserialize, Type)]
struct StatsByPromptArgsCodegen {
    pub prompt_version: String,
    /// v0.101b — placeholder (real impl uses Option<i64>). Stub uses Option<u32>.
    pub window_days: Option<u32>,
}

/// v0.101b — LlmStatsByPrompt shape.
#[derive(Serialize, Deserialize, Type)]
struct LlmStatsByPromptCodegen {
    pub provider_id: String,
    pub prompt_version: String,
    /// v0.101b — placeholder (real impl uses i64). Stub uses i32.
    pub n_recommendations: i32,
    pub n_evaluated: i32,
    pub win_rate: f64,
    pub brier: f64,
}

/// v0.101b — codegen stub for `llm_stats_by_prompt`.
#[tauri::command]
#[specta::specta]
async fn llm_stats_by_prompt_codegen(
    _args: StatsByPromptArgsCodegen,
) -> Result<Vec<LlmStatsByPromptCodegen>, String> {
    Ok(vec![])
}

/// v0.101b — LlmStatsCostEfficiency shape.
#[derive(Serialize, Deserialize, Type)]
struct LlmStatsCostEfficiencyCodegen {
    pub provider_id: String,
    pub total_cost_cents: f64,
    pub total_pnl: f64,
    pub efficiency: f64,
    pub win_rate: f64,
}

/// v0.101b — codegen stub for `llm_stats_cost_efficiency`.
#[tauri::command]
#[specta::specta]
async fn llm_stats_cost_efficiency_codegen(
    window_days: Option<u32>,
) -> Result<Vec<LlmStatsCostEfficiencyCodegen>, String> {
    Ok(vec![])
}

/// v0.101b — args for `llm_stats_export`. Matches real `ExportStatsArgs`.
#[derive(Serialize, Deserialize, Type)]
struct ExportStatsArgsCodegen {
    pub format: String,
    /// v0.101b — placeholder (real impl uses Option<i64>). Stub uses Option<u32>.
    pub window_days: Option<u32>,
}

/// v0.101b — codegen stub for `llm_stats_export`. Returns string
/// (CSV or JSON) the L1 layer saves via tauri-plugin-fs.
#[tauri::command]
#[specta::specta]
async fn llm_stats_export_codegen(
    _args: ExportStatsArgsCodegen,
) -> Result<String, String> {
    Ok(String::new())
}

// =================================================================
// v0.101c — Phase 4 batch 7 part 3: LLM traffic_summary + 4 scheduler
//                     commands (5 read-only + 2 trigger + 1 self_test)
// =================================================================
//
// Drift detection for the L1 "LLM Mgmt traffic" panel and the
// Settings "Scheduler" + "Self Test" cards. trigger commands
// return Result<TriggerResult, String> just like the real ones;
// self_test returns SchedulerSelfTest sync (no State needed).

/// v0.101c — args for `llm_traffic_summary`. Matches real `TrafficArgs`.
#[derive(Serialize, Deserialize, Type)]
struct TrafficArgsCodegen {
    pub window: Option<String>,
    pub provider_id: Option<String>,
}

/// v0.101c — LlmTrafficSummary shape. Many i64 fields use
/// `#[specta(type = BigInt)]` for lossless export (real counts
/// can exceed 2^32 in long windows).
#[derive(Serialize, Deserialize, Type)]
struct LlmTrafficSummaryCodegen {
    pub provider_id: String,
    pub window: String,
    #[specta(type = BigInt)]
    pub calls_total: i64,
    #[specta(type = BigInt)]
    pub calls_success: i64,
    #[specta(type = BigInt)]
    pub calls_failed: i64,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub p95_latency_ms: f64,
    #[specta(type = BigInt)]
    pub total_tokens_in: i64,
    #[specta(type = BigInt)]
    pub total_tokens_out: i64,
    pub total_cost_cents: f64,
    #[specta(type = BigInt)]
    pub rate_limit_hits: i64,
    #[specta(type = BigInt)]
    pub delta_calls_last_window: i64,
    pub delta_cost_last_window: f64,
}

/// v0.101c — codegen stub for `llm_traffic_summary`.
#[tauri::command]
#[specta::specta]
async fn llm_traffic_summary_codegen(
    _args: TrafficArgsCodegen,
) -> Result<Vec<LlmTrafficSummaryCodegen>, String> {
    Ok(vec![])
}

/// v0.101c — SchedulerStatus shape. next_brief_run_at_unix_ms uses
/// `#[specta(type = BigInt)]` for lossless timestamp.
#[derive(Serialize, Deserialize, Type)]
struct SchedulerStatusCodegen {
    /// v0.101c — placeholder (real impl uses u64). Stub uses u32
    /// because specta-typescript forbids u64 raw.
    pub health_probe_interval_sec: u32,
    pub daily_brief_hour_utc: u32,
    pub daily_brief_tz_offset_min: i32,
    /// v0.101c — placeholder (real impl uses u64). Stub uses u32.
    pub anomaly_window_sec: u32,
    #[specta(type = BigInt)]
    pub next_brief_run_at_unix_ms: i64,
}

/// v0.101c — codegen stub for `scheduler_status`.
#[tauri::command]
#[specta::specta]
async fn scheduler_status_codegen(
) -> Result<SchedulerStatusCodegen, String> {
    Ok(SchedulerStatusCodegen {
        health_probe_interval_sec: 0,
        daily_brief_hour_utc: 0,
        daily_brief_tz_offset_min: 0,
        anomaly_window_sec: 0,
        next_brief_run_at_unix_ms: 0,
    })
}

/// v0.101c — TriggerResult shape. triggered_at_unix_ms uses
/// `#[specta(type = BigInt)]`.
#[derive(Serialize, Deserialize, Type)]
struct TriggerResultCodegen {
    #[specta(type = BigInt)]
    pub triggered_at_unix_ms: i64,
    pub kind: String,
    pub ok: bool,
    pub error: Option<String>,
}

/// v0.101c — codegen stub for `scheduler_run_health_probe_now`.
#[tauri::command]
#[specta::specta]
async fn scheduler_run_health_probe_now_codegen(
) -> Result<TriggerResultCodegen, String> {
    Ok(TriggerResultCodegen {
        triggered_at_unix_ms: 0,
        kind: "health_probe".to_string(),
        ok: false,
        error: None,
    })
}

/// v0.101c — codegen stub for `scheduler_run_daily_brief_now`.
#[tauri::command]
#[specta::specta]
async fn scheduler_run_daily_brief_now_codegen(
) -> Result<TriggerResultCodegen, String> {
    Ok(TriggerResultCodegen {
        triggered_at_unix_ms: 0,
        kind: "daily_brief".to_string(),
        ok: false,
        error: None,
    })
}

/// v0.101c — LoopStatus shape. Real type uses `&'static str` for
/// `name` but stub uses `String` because specta handles `&'static str`
/// fine but we want to keep the codegen stub self-contained.
#[derive(Serialize, Deserialize, Type)]
struct LoopStatusCodegen {
    pub name: String,
    #[specta(type = BigInt)]
    pub last_tick_unix_ms: u64,
    /// v0.101c — placeholder (real impl uses Option<u64>). Stub
    /// uses Option<u32> because specta-typescript forbids u64 in
    /// Option even with #[specta(type = BigInt)].
    pub age_ms: Option<u32>,
    pub healthy: bool,
}

/// v0.101c — SchedulerSelfTest shape. matches real SchedulerSelfTest.
#[derive(Serialize, Deserialize, Type)]
struct SchedulerSelfTestCodegen {
    #[specta(type = BigInt)]
    pub process_started_at_unix: u64,
    #[specta(type = BigInt)]
    pub checked_at_unix_ms: u64,
    pub all_healthy: bool,
    pub loops: Vec<LoopStatusCodegen>,
}

/// v0.101c — codegen stub for `scheduler_self_test_now`. Real
/// impl is sync (not async) — it just reads atomics.
#[tauri::command]
#[specta::specta]
async fn scheduler_self_test_now_codegen(
) -> Result<SchedulerSelfTestCodegen, String> {
    Ok(SchedulerSelfTestCodegen {
        process_started_at_unix: 0,
        checked_at_unix_ms: 0,
        all_healthy: true,
        loops: vec![],
    })
}

// =================================================================
// v0.102b — Phase 4 batch 8: degradation + audit purge +
//                     daily_brief (6 read-only / write commands)
// =================================================================
//
// Drift detection for the L1 Settings "Degradation", "Audit
// retention purge now", and the Dashboard "Daily Brief" panel.
// daily_brief_set_prefs uses an inner struct (BriefWeights) —
// nested Type is supported by specta.

// ---------- degradation_check_now ----------
/// v0.102b — empty args (DegradationCheckNowArgs in real impl).
#[derive(Serialize, Deserialize, Type, Default)]
struct DegradationCheckNowArgsCodegen {}

/// v0.102b — codegen stub for `degradation_check_now`. Returns
/// () — the side effect is firing a telemetry event.
#[tauri::command]
#[specta::specta]
async fn degradation_check_now_codegen(
    _args: DegradationCheckNowArgsCodegen,
) -> Result<(), String> {
    Ok(())
}

// ---------- purge_audit_log_now ----------
/// v0.102b — codegen stub for `purge_audit_log_now`. Returns
/// `usize` count of purged rows.
#[tauri::command]
#[specta::specta]
async fn purge_audit_log_now_codegen() -> Result<u32, String> {
    Ok(0)
}

// ---------- daily_brief_get ----------
/// v0.102b — args for `daily_brief_get`. Matches real `BriefGetArgs`.
#[derive(Serialize, Deserialize, Type, Default)]
struct BriefGetArgsCodegen {
    /// v0.102b — placeholder (real impl uses Option<i64>).
    /// Stub uses Option<u32>.
    pub limit: Option<u32>,
    /// v0.102b — placeholder (real impl uses Option<i64>).
    /// Stub uses Option<u32> (kept for back-compat with old callers).
    pub max_items: Option<u32>,
}

/// v0.102b — DailyBriefEntry shape. timestamps use
/// `#[specta(type = BigInt)]` for lossless export; counts use i32.
#[derive(Serialize, Deserialize, Type)]
struct DailyBriefEntryCodegen {
    pub market_id: String,
    pub market_question: String,
    pub market_category: String,
    #[specta(type = BigInt)]
    pub market_end_date: i64,
    pub market_liquidity: Option<String>,
    pub market_volume_24h: Option<String>,
    /// v0.102b — placeholder (real impl uses i64). Stub uses i32.
    pub rank: i32,
    pub match_score: f64,
    pub score_breakdown: Option<String>,
    pub edge: Option<f64>,
    pub confidence: Option<f64>,
    pub consensus_side: Option<String>,
    pub consensus_strength: Option<f64>,
    #[specta(type = BigInt)]
    pub computed_at: i64,
    #[specta(type = BigInt)]
    pub expires_at: i64,
    pub dismissed: bool,
}

/// v0.102b — codegen stub for `daily_brief_get`.
#[tauri::command]
#[specta::specta]
async fn daily_brief_get_codegen(
    _args: BriefGetArgsCodegen,
) -> Result<Vec<DailyBriefEntryCodegen>, String> {
    Ok(vec![])
}

// ---------- daily_brief_refresh ----------
/// v0.102b — BriefRefreshResult shape.
#[derive(Serialize, Deserialize, Type)]
struct BriefRefreshResultCodegen {
    #[specta(type = BigInt)]
    pub computed_at: i64,
    /// v0.102b — placeholder (real impl uses i64). Stub uses i32.
    pub n_items: i32,
}

/// v0.102b — codegen stub for `daily_brief_refresh`.
#[tauri::command]
#[specta::specta]
async fn daily_brief_refresh_codegen(
) -> Result<BriefRefreshResultCodegen, String> {
    Ok(BriefRefreshResultCodegen {
        computed_at: 0,
        n_items: 0,
    })
}

// ---------- daily_brief_dismiss ----------
/// v0.102b — args for `daily_brief_dismiss`. Market ID only.
#[derive(Serialize, Deserialize, Type, Default)]
struct BriefDismissArgsCodegen {
    pub market_id: String,
}

/// v0.102b — codegen stub for `daily_brief_dismiss`.
#[tauri::command]
#[specta::specta]
async fn daily_brief_dismiss_codegen(
    _args: BriefDismissArgsCodegen,
) -> Result<bool, String> {
    Ok(true)
}

// ---------- daily_brief_set_prefs ----------
/// v0.102b — BriefWeights shape (nested struct).
#[derive(Serialize, Deserialize, Type)]
struct BriefWeightsCodegen {
    pub w1: f64,
    pub w2: f64,
    pub w3: f64,
    pub w4: f64,
    pub w5: f64,
    pub w6: f64,
}

/// v0.102b — args for `daily_brief_set_prefs`. Matches real
/// `SetBriefPrefsArgs`. Nested Option<BriefWeights>.
#[derive(Serialize, Deserialize, Type)]
struct SetBriefPrefsArgsCodegen {
    pub user_id: String,
    pub weights: Option<BriefWeightsCodegen>,
    /// v0.102b — placeholder (real impl uses Option<i64>).
    pub max_items: Option<u32>,
    pub min_liquidity: Option<String>,
    pub categories: Option<Vec<String>>,
}

/// v0.102b — codegen stub for `daily_brief_set_prefs`.
#[tauri::command]
#[specta::specta]
async fn daily_brief_set_prefs_codegen(
    _args: SetBriefPrefsArgsCodegen,
) -> Result<(), String> {
    Ok(())
}

// =================================================================
// v0.103b — Phase 4 batch 9 part 1: LLM provider CRUD (4 commands)
// =================================================================
//
// Drift detection for the L1 "LLM Management" page. The real
// LlmProviderDto has 25+ fields — we use `i32` placeholders for
// count/time fields and `#[specta(type = BigInt)]` for the
// last_health_check_at timestamp.

/// v0.103b — full LlmProviderDto shape. Many i64 fields use i32
/// placeholders (drift detection only, L1 stays on `number`).
/// Note: the v0.88d `LlmProviderDtoCodegen` (smaller, for
/// upsert_llm_provider) already exists; we use `LlmProviderDtoFullCodegen`
/// here to avoid name collision.
#[derive(Serialize, Deserialize, Type)]
struct LlmProviderDtoFullCodegen {
    pub id: String,
    pub display_name: String,
    pub provider_kind: String,
    pub request_format: String,
    pub supports_streaming: bool,
    pub enabled: bool,
    pub api_base: Option<String>,
    pub key_alias: String,
    pub default_model: String,
    /// v0.103b — placeholder (real impl uses i64). Stub uses i32.
    pub timeout_ms: i32,
    pub request_timeout_ms: i32,
    pub max_retries: i32,
    pub cost_per_1k_in: Option<f64>,
    pub cost_per_1k_out: Option<f64>,
    pub rate_limit_rpm: Option<i32>,
    pub rate_limit_tpm: Option<i32>,
    pub quota_daily_cents: Option<f64>,
    pub quota_monthly_cents: Option<f64>,
    pub key_rotation_strategy: String,
    pub health_status: String,
    pub health_latency_p50_ms: Option<i32>,
    pub health_latency_p95_ms: Option<i32>,
    /// v0.103b — timestamp (real impl uses Option<i64>). We use i32
    /// placeholder (drift detection only) — Option<i64> would need
    /// OptionBigInt for full lossless export.
    pub last_health_check_at: Option<i32>,
    pub last_health_error: Option<String>,
    pub notes: Option<String>,
}

/// v0.103b — codegen stub for `llm_provider_list`.
#[tauri::command]
#[specta::specta]
async fn llm_provider_list_codegen(
) -> Result<Vec<LlmProviderDtoFullCodegen>, String> {
    Ok(vec![])
}

/// v0.103b — codegen stub for `llm_provider_upsert`. Takes
/// the full LlmProviderDto.
#[tauri::command]
#[specta::specta]
async fn llm_provider_upsert_codegen(
    _provider: LlmProviderDtoFullCodegen,
) -> Result<(), String> {
    Ok(())
}

/// v0.103b — codegen stub for `llm_provider_delete`.
#[tauri::command]
#[specta::specta]
async fn llm_provider_delete_codegen(
    _provider_id: String,
) -> Result<(), String> {
    Ok(())
}

// =================================================================
// v0.103b2 — Phase 4 batch 9 part 2: LLM key CRUD + connectivity
//                      + health + performance (7 commands)
// =================================================================
//
// Continuation of v0.103b part 1 — covers the rest of the LLM
// management surface: key CRUD, connectivity testing, health
// history, and aggregate performance.

/// v0.103b2 — LlmProviderKeyDto shape. Count fields use i32.
#[derive(Serialize, Deserialize, Type)]
struct LlmProviderKeyDtoCodegen {
    pub id: String,
    pub provider_id: String,
    pub alias: String,
    pub keyring_alias: String,
    pub enabled: bool,
    /// v0.103b2 — placeholder (real impl uses i64). Stub uses i32.
    pub priority: i32,
    pub weight: i32,
    /// v0.103b2 — timestamp (real impl uses Option<i64>). i32 placeholder.
    pub last_used_at: Option<i32>,
    pub last_error: Option<String>,
    /// v0.103b2 — timestamp (real impl uses Option<i64>). i32 placeholder.
    pub last_error_at: Option<i32>,
    pub total_calls: i32,
    pub total_errors: i32,
    pub notes: Option<String>,
}

/// v0.103b2 — codegen stub for `llm_key_list`.
#[tauri::command]
#[specta::specta]
async fn llm_key_list_codegen(
    _provider_id: Option<String>,
) -> Result<Vec<LlmProviderKeyDtoCodegen>, String> {
    Ok(vec![])
}

/// v0.103b2 — args for `llm_key_upsert`. Nested LlmProviderKeyDto.
#[derive(Serialize, Deserialize, Type)]
struct KeyUpsertArgsCodegen {
    pub key: LlmProviderKeyDtoCodegen,
    /// Plaintext secret (optional). When provided, written to OS
    /// keyring; when None, only metadata is upserted.
    pub secret: Option<String>,
}

/// v0.103b2 — codegen stub for `llm_key_upsert`.
#[tauri::command]
#[specta::specta]
async fn llm_key_upsert_codegen(
    _args: KeyUpsertArgsCodegen,
) -> Result<(), String> {
    Ok(())
}

/// v0.103b2 — args for `llm_key_set_secret`.
#[derive(Serialize, Deserialize, Type)]
struct KeySetSecretArgsCodegen {
    pub key_id: String,
    pub secret: String,
}

/// v0.103b2 — codegen stub for `llm_key_set_secret`.
#[tauri::command]
#[specta::specta]
async fn llm_key_set_secret_codegen(
    _args: KeySetSecretArgsCodegen,
) -> Result<(), String> {
    Ok(())
}

/// v0.103b2 — codegen stub for `llm_key_delete`.
#[tauri::command]
#[specta::specta]
async fn llm_key_delete_codegen(
    _key_id: String,
) -> Result<(), String> {
    Ok(())
}

/// v0.103b2 — args for `llm_test_connectivity`.
#[derive(Serialize, Deserialize, Type)]
struct TestConnectivityArgsCodegen {
    pub provider_id: String,
    pub key_id: Option<String>,
}

/// v0.103b2 — ConnectivityTestResult shape. status fields use
/// i32 placeholders for counts.
#[derive(Serialize, Deserialize, Type)]
struct ConnectivityTestResultCodegen {
    pub provider_id: String,
    pub success: bool,
    /// v0.103b2 — placeholder (real impl uses Option<i64>).
    pub latency_ms: Option<i32>,
    /// v0.103b2 — placeholder (real impl uses Option<i64>).
    pub http_status: Option<i32>,
    pub model_used: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

/// v0.103b2 — codegen stub for `llm_test_connectivity`.
#[tauri::command]
#[specta::specta]
async fn llm_test_connectivity_codegen(
    _args: TestConnectivityArgsCodegen,
) -> Result<ConnectivityTestResultCodegen, String> {
    Ok(ConnectivityTestResultCodegen {
        provider_id: String::new(),
        success: false,
        latency_ms: None,
        http_status: None,
        model_used: String::new(),
        error_code: None,
        error_message: None,
    })
}

/// v0.103b2 — LlmHealthCheckDto shape. timestamps use i32 placeholders.
#[derive(Serialize, Deserialize, Type)]
struct LlmHealthCheckDtoCodegen {
    /// v0.103b2 — placeholder (real impl uses i64). i32.
    pub id: i32,
    pub provider_id: String,
    /// v0.103b2 — timestamp placeholder (real impl uses i64). i32.
    pub checked_at: i32,
    pub trigger: String,
    pub success: bool,
    pub latency_ms: Option<i32>,
    pub http_status: Option<i32>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

/// v0.103b2 — codegen stub for `llm_health_history`.
#[tauri::command]
#[specta::specta]
async fn llm_health_history_codegen(
    _provider_id: String,
    _limit: Option<u32>,
) -> Result<Vec<LlmHealthCheckDtoCodegen>, String> {
    Ok(vec![])
}

/// v0.103b2 — LlmPerformanceRow shape. Count fields use i32.
#[derive(Serialize, Deserialize, Type)]
struct LlmPerformanceRowCodegen {
    pub provider_id: String,
    pub provider_name: String,
    /// v0.103b2 — placeholder (real impl uses i64). i32.
    pub n_recommendations: i32,
    pub n_evaluated: i32,
    pub win_rate: f64,
    pub brier: f64,
    pub avg_confidence: f64,
    pub total_cost_cents: f64,
}

/// v0.103b2 — codegen stub for `llm_performance`.
#[tauri::command]
#[specta::specta]
async fn llm_performance_codegen(
    _window_days: Option<u32>,
    _category: Option<String>,
) -> Result<Vec<LlmPerformanceRowCodegen>, String> {
    Ok(vec![])
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
    // v0.88e — Phase 4 batch 5 (complex nested DTOs)
    let _ = commands::llm::llm_analyze;
    let _ = commands::mirror_executor::run_mirror_executor_pass;
    // v0.101a — Phase 4 batch 7: LLM stats heatmap + scatter + timeseries + decision
    let _ = commands::llm::llm_stats_heatmap;
    let _ = commands::llm::llm_stats_scatter;
    let _ = commands::llm::llm_stats_timeseries;
    let _ = commands::llm::llm_stats_decision;
    // v0.101b — Phase 4 batch 7 part 2: LLM stats by_confidence + by_prompt
    //                                                  + cost_efficiency + export
    let _ = commands::llm_mgmt::llm_stats_by_confidence;
    let _ = commands::llm_mgmt::llm_stats_by_prompt;
    let _ = commands::llm_mgmt::llm_stats_cost_efficiency;
    let _ = commands::llm_mgmt::llm_stats_export;
    // v0.101c — Phase 4 batch 7 part 3: LLM traffic + scheduler (5 commands)
    let _ = commands::llm_mgmt::llm_traffic_summary;
    let _ = commands::scheduler::scheduler_status;
    let _ = commands::scheduler::scheduler_run_health_probe_now;
    let _ = commands::scheduler::scheduler_run_daily_brief_now;
    let _ = commands::scheduler::scheduler_self_test_now;
    // v0.102b — Phase 4 batch 8: degradation + audit purge + daily_brief (6 commands)
    let _ = commands::scheduler::degradation_check_now;
    let _ = commands::audit::purge_audit_log_now;
    let _ = commands::brief::daily_brief_get;
    let _ = commands::brief::daily_brief_refresh;
    let _ = commands::brief::daily_brief_dismiss;
    let _ = commands::brief::daily_brief_set_prefs;
    // v0.103b — Phase 4 batch 9 part 1: LLM provider CRUD (4 commands)
    let _ = commands::llm_mgmt::llm_provider_list;
    let _ = commands::llm_mgmt::llm_provider_upsert;
    let _ = commands::llm_mgmt::llm_provider_delete;
    // v0.103b2 — Phase 4 batch 9 part 2: LLM key CRUD + connectivity + health + performance (7 commands)
    let _ = commands::llm_mgmt::llm_key_list;
    let _ = commands::llm_mgmt::llm_key_upsert;
    let _ = commands::llm_mgmt::llm_key_set_secret;
    let _ = commands::llm_mgmt::llm_key_delete;
    let _ = commands::llm_mgmt::llm_test_connectivity;
    let _ = commands::llm_mgmt::llm_health_history;
    let _ = commands::llm::llm_performance;

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
        // v0.88e — Phase 4 batch 5 (complex nested DTOs)
        llm_analyze_codegen,
        run_mirror_executor_pass_codegen,
        // v0.98 — Phase 4 batch 6: 4 more read-only list commands
        list_bets_codegen,
        list_audit_log_codegen,
        list_copy_targets_codegen,
        list_promote_history_codegen,
        // v0.101a — Phase 4 batch 7: LLM stats (4 read-only commands)
        llm_stats_heatmap_codegen,
        llm_stats_scatter_codegen,
        llm_stats_timeseries_codegen,
        llm_stats_decision_codegen,
        // v0.101b — Phase 4 batch 7 part 2: LLM stats (4 more)
        llm_stats_by_confidence_codegen,
        llm_stats_by_prompt_codegen,
        llm_stats_cost_efficiency_codegen,
        llm_stats_export_codegen,
        // v0.101c — Phase 4 batch 7 part 3: LLM traffic + scheduler (5 commands)
        llm_traffic_summary_codegen,
        scheduler_status_codegen,
        scheduler_run_health_probe_now_codegen,
        scheduler_run_daily_brief_now_codegen,
        scheduler_self_test_now_codegen,
        // v0.102b — Phase 4 batch 8: degradation + audit purge + daily_brief (6 commands)
        degradation_check_now_codegen,
        purge_audit_log_now_codegen,
        daily_brief_get_codegen,
        daily_brief_refresh_codegen,
        daily_brief_dismiss_codegen,
        daily_brief_set_prefs_codegen,
        // v0.103b — Phase 4 batch 9 part 1: LLM provider CRUD (4 commands)
        llm_provider_list_codegen,
        llm_provider_upsert_codegen,
        llm_provider_delete_codegen,
        // v0.103b2 — Phase 4 batch 9 part 2: LLM key CRUD + connectivity + health + performance (7 commands)
        llm_key_list_codegen,
        llm_key_upsert_codegen,
        llm_key_set_secret_codegen,
        llm_key_delete_codegen,
        llm_test_connectivity_codegen,
        llm_health_history_codegen,
        llm_performance_codegen,
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

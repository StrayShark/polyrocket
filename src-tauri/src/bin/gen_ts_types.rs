//! v0.76 / v0.81 / v0.84 / v0.88 —— codegen Phase 1+2+3+4 二进制。
//!
//! 从带 `#[tauri::command] + #[specta::specta]` 标注的命令
//! 生成 TS 绑定。
//!
//! v0.76 = Phase 1：1 个命令（dashboard_kpis）概念验证。
//! v0.81 = Phase 2：+4 个 bankroll 命令
//!   （compute_allocation_preview、get_bankroll_config、
//!    set_bankroll_config、apply_allocation）。
//! v0.84 = Phase 3：+14 个只读命令（无输入 DTO）。
//! v0.88 = Phase 4：输入 DTO 命令。v0.88a 新增 3 种简单结构：
//!   - add_wallet → AddWalletArgsCodegen + WalletDtoCodegen
//!   - set_telemetry_enabled → SetTelemetryEnabledArgsCodegen
//!   - set_mirror_paper_mode → SetMirrorPaperModeArgsCodegen
//!
//! Phase 5（构建流水线：drift 检测器 → pnpm build）是下一步。
//!
//! 运行：`cargo run --bin gen_ts_types`
//! 输出：`src/types/generated/index.ts`
//!
//! 参见 `docs/codegen-migration-plan.md` 中的 5 阶段计划。

use polyrocket_lib::commands;
use serde::{Deserialize, Serialize};
use specta::Type;
use polyrocket_lib::codegen::bigint_map::BigIntMap;
use polyrocket_lib::codegen::option_bigint::OptionBigInt;
use specta_typescript::BigInt;
use tauri_specta::{collect_commands, Builder};

// v0.81 —— 友好的 codegen Signal。真实的 `domain::signal::Signal`
// 拥有 `computed_at: i64`，这在 specta-typescript 中被禁止（BigInt）。
// 该存根使用 `i32` 来绕过。`src/types/signal.ts` 中的真实 TS DTO
// 也应为 computed_at 使用 number ——
// 我们在迁移计划中记录了这一漂移。
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
// v0.76 —— DashboardKpis（Phase 1 试点）
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
// v0.84 —— Phase 3：5 个只读命令（无 i64 字段）
// =================================================================
//
// 这些存根直接使用 `commands::*` 中的真实 DTO，
// 以便 drift 检测能够生效：Rust 中的字段重命名 / 类型变更
// 会在 `src/types/generated/index.ts` 中以 diff 的形式体现
// （v0.84d 将证明这一点）。存根命令返回正确
// 形状的硬编码数据，使 codegen 可以在没有 Tauri State 时运行。

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
// v0.84 —— Phase 3 batch 2：5 个简单参数的只读命令
// =================================================================
//
// 这 5 个中 3 个使用真实 DTO（无 i64 字段）：
//   - get_auto_promote_config → AutoPromoteConfigDto（无 i64）
//   - get_storage_info → StorageInfo（无 i64）
//   - get_mirror_paper_mode → bool（无 struct）
//
// 2 个使用 `*CodegenDto` 存根，因为真实类型包含 i64 字段
//（specta-typescript 默认 BigInt 行为是 Fail）。
// 我们使用 `specta_typescript::Number<i64>`，导出为 TS `number`
//（接受精度损失；在距 epoch ~285,000 年内的
// Unix ms 时间戳是安全的）。
//   - get_audit_retention → AuditRetentionViewCodegen（i64 字段）
//   - get_active_model → ActiveModelCodegen（i64 字段）

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
        /// v0.84b —— 真实类型中 `free_bytes: Option<u64>` 被 BigInt 禁止。
        /// 存根使用 `Option<i64>`（有符号，
        /// 以在 2^63 范围内无损；字节数可以轻松容纳）。
        free_bytes: None,
        restart_required: false,
    })
}

/// v0.84b —— `StorageInfo` 的 codegen 存根。真实结构体
/// 拥有 `free_bytes: Option<u64>`（BigInt 禁止；specta-typescript
/// 默认模式下连 i64 也被禁止）。存根使用
/// `Option<f64>`（超过 2^53 字节会出现精度损失，
/// 但在 ~9 PB 以内的磁盘空间字节数可以轻松容纳）。
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

// v0.86c —— 针对 AuditRetentionView（拥有 4 个 i64 字段），
// 存根对 map value 使用 `BigIntMap<String, i64>`，
// 对必需的 i64 标量使用 `#[specta(type = BigInt)]`。
// v0.85c 曾经回退到 i32 占位符，因为
// `HashMap<String, i64>` 上的 `#[specta(type = BigInt)]`
// 不会递归到 value 类型；v0.86c 新增自定义 `BigIntMap`
// 包装器以修复该情况。
#[derive(Serialize, Deserialize, Type)]
struct AuditRetentionViewCodegen {
    #[specta(type = BigInt)]
    pub retain_recent_ms: i64,
    #[specta(type = BigInt)]
    pub max_rows: i64,
    #[specta(type = BigInt)]
    pub min_keep_rows: i64,
    /// v0.86c —— `BigIntMap<String, i64>` → TS `{ [key: string]: bigint }`。
    /// 有关包装器设计，请参见 src-tauri/src/codegen/bigint_map.rs。
    pub overrides: BigIntMap<String, i64>,
}

// v0.98 —— 4 个新命令的 DTO 存根。
// 每个都镜像真实的 Rust 类型，但对那些
// 在真实实现中为 i64 / u64 / Decimal 的字段
//（我们在 codegen 存根中跳过）使用 i32 / String
// 占位符。

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
    /// v0.98 —— 占位符（真实实现使用包在
    /// OptionBigInt 中的 Option<i64>）。存根使用 Option<u32> 用于 ts 导出。
    pub since_ms: Option<u32>,
}

#[derive(Serialize, Deserialize, Type)]
struct AuditEntryDtoCodegen {
    /// v0.98 —— 占位符（真实实现使用带 `#[specta(type = BigInt)]` 的 i64，
    /// 或者在可选情况下使用 `Option<OptionBigInt<i64>>`）。
    /// 存根使用 i32，因为 audit ID 受历史大小的限制。
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
    /// v0.84b —— `best_params: Option<serde_json::Value>` 在 codegen
    /// 存根中被省略，因为 `serde_json::Value` 没有实现
    /// `specta::Type`。L1 层在手写的
    /// `ActiveModel` 接口（src/ipc.ts:1227）中将其保留为
    /// `Record<string, unknown> | null`。因此该字段的
    /// drift 检测仅限于 L1 层（由现有的 v2 contract
    /// 测试覆盖）。v0.84+ 可能为 serde_json::Value
    /// 添加自定义 Type 实现。
    /// v0.86b —— Option<OptionBigInt<i64>> → TS `bigint | null`
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
// v0.84 —— Phase 3 batch 3：4 个返回 Vec 的命令
// =================================================================
//
// 4 个 DTO 都包含 i64 字段（BigInt 禁止）。我们使用带
// i32 占位符的 `*CodegenDto` 存根，遵循 v0.81 SignalCodegenDto
// 模式。真实类型保持不变。

// ---------- SignalDto（list_active_signals）----------

/// v0.84c —— codegen 存根参数。真实的 `ListSignalsArgs` 拥有
/// `Option<i64>`（limit），这在 BigInt 中被禁止。存根使用
/// `Option<i32>`（与 codegen 友好的 limit 语义一致）。
#[derive(Serialize, Deserialize, Type, Default)]
struct ListSignalsArgsCodegen {
    pub min_edge: Option<f64>,
    pub category: Option<String>,
    /// v0.86b —— Option<OptionBigInt<i64>> → TS `bigint | null`。
    pub limit: Option<OptionBigInt<i64>>,
}

/// v0.84c —— `SignalDto` 的 codegen 存根。真实结构体拥有 4× i64
/// 字段（`id`、`computed_at`、`horizon_hours` 等）。存根使用 i32
///（针对字段集 + 名称的 drift 检测，而非 i64→i32
/// 精度；v0.84+ 可能通过 serde feature 切换到 BigInt<i64>）。
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

// ---------- MirrorRow（list_mirrors）----------

/// v0.84c —— codegen 存根参数。真实的 `ListMirrorsArgs` 拥有
/// `Option<i64>`（limit）。存根使用 `Option<i32>`。
#[derive(Serialize, Deserialize, Type, Default)]
struct ListMirrorsArgsCodegen {
    pub status: Option<String>,
    /// v0.86b —— Option<OptionBigInt<i64>> → TS `bigint | null`。
    pub limit: Option<OptionBigInt<i64>>,
}

/// v0.85b —— `MirrorRow` 的 codegen 存根。真实结构体包含 5× i64 字段
///（`event_id`、`created_at`、`submitted_at`、`filled_at` 等）。
/// v0.85+：使用 `#[specta(type = BigInt)]` 属性将 i64
/// 字段标记为 TS `bigint`（对于 53 位以内可容纳的值无损）。
/// L1 层（src/types/mirror.ts）暂时将其保留为 `number`
///（JSON.parse 给出 `number` 而非 `bigint`）；后续将添加
/// 显式的 `BigInt()` 转换。
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
    /// v0.86b —— Option<OptionBigInt<i64>> → TS `bigint | null`
    ///（对于 53 位以内可容纳的值无损）。OptionBigInt
    /// 包装器对 serde 是透明的，因此线缆格式与
    /// `Option<i64>` 相同。请参见 src-tauri/src/codegen/option_bigint.rs。
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

// ---------- WalletDto（list_wallets）----------

/// v0.84c —— `WalletDto` 的 codegen 存根。真实结构体包含 3× i64 字段
///（`chain_id`、`created_at`、`last_synced_at`）。
#[derive(Serialize, Deserialize, Type)]
struct WalletDtoCodegen {
    pub id: String,
    pub address: String,
    pub label: Option<String>,
    pub chain_id: i32,
    pub wallet_type: String,
    /// v0.86b —— `created_at: i64` 加 `#[specta(type = BigInt)]` → TS `bigint`。
    /// `last_synced_at: Option<OptionBigInt<i64>>` → TS `bigint | null`。
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
// v0.88a —— Phase 4 batch 1：3 个输入 DTO 命令（简单结构）
// =================================================================
//
// 输入 DTO 的 drift 检测。真实命令位于
// `commands/wallet.rs` / `commands/sidecar.rs` /
// `commands/mirror_executor.rs` —— 这些存根纯粹为了让 codegen
// 运行，并让 drift 检测器能够捕捉真实 args DTO
// 中的任何未来重命名 / 类型变更。L1 仍然调用真实
// 命令（`add_wallet` / `set_telemetry_enabled` /
// `set_mirror_paper_mode`）—— 此处的 `_codegen` 后缀
// 是 codegen 面的标记，而非 L1 可调用的 IPC。

/// v0.88a —— `add_wallet` 的 codegen 存根参数。真实的
/// `AddWalletArgs`（commands/wallet.rs:26）拥有 `chain_id: Option<i64>`；
/// 我们截断到 i32，因为 chain ID 较小（Polygon 主网 = 137，
/// 可轻松容纳）。`label`/`address`/`wallet_type` 类型或
/// `chain_id` 是否可选的未来漂移会在这里显现。
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
    // v0.88a —— 存根。真实实现在 commands/wallet.rs:55 插入行并
    // 返回创建的 WalletDto。我们返回硬编码的形状，以便
    // codegen 可以导出类型签名。
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

/// v0.88a —— `set_telemetry_enabled` 的 codegen 存根参数。真实的
/// `SetTelemetryEnabledArgs`（commands/sidecar.rs:1857）
/// 包装单个 `enabled: bool`。我们精确地镜像该形状，
/// 使 codegen 签名与 L1 发送的内容匹配。
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

/// v0.88a —— `set_mirror_paper_mode` 的 codegen 存根参数。真实的
/// `SetMirrorPaperModeArgs`（commands/mirror_executor.rs:162）
/// 包装单个 `enabled: bool`。与 `set_telemetry_enabled` 模式相同。
#[derive(Serialize, Deserialize, Type)]
struct SetMirrorPaperModeArgsCodegen {
    pub enabled: bool,
}

#[tauri::command]
#[specta::specta]
async fn set_mirror_paper_mode_codegen(
    _args: SetMirrorPaperModeArgsCodegen,
) -> Result<bool, String> {
    // v0.88a —— 存根。commands/mirror_executor.rs:174 中的真实实现
    // 接受 State<'_,'_, AppState> 并写入 state.mirror_paper_mode
    //（一个 Arc<Mutex<bool>>）。在 bin 上下文中无法构造 State，
    // 因此我们返回正确形状的硬编码数据。
    Ok(true)
}

// =================================================================
// v0.88b —— Phase 4 batch 2：2 个 Copy 路由命令（输入 DTO）
// =================================================================
//
// `add_copy_target` 和 `enqueue_mirror` 的 drift 检测。真实
// 命令位于 commands/copy.rs 和 commands/mirror_executor.rs。

/// v0.88b —— `CopyTargetDto` 的 codegen 存根。真实类型拥有 `created_at: i64`，
/// specta-typescript 禁止该类型（BigInt）；使用 OptionBigInt 包装器
///（v0.86b）→ TS `bigint | null`。与 WalletDtoCodegen 模式相同。
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

/// v0.88b —— `add_copy_target` 的 codegen 存根参数。真实的
/// `AddCopyTargetArgs`（commands/copy.rs）没有 i64 字段 —— 都是
/// 普通的 string/f64。Codegen 形状与真实相同。
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
    // v0.88b —— 存根。commands/copy.rs:68 中的真实实现插入行并
    // 返回创建的 CopyTargetDto。我们返回硬编码的形状。
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

/// v0.88b —— `enqueue_mirror` 的 codegen 存根参数。真实的
/// `EnqueueArgs`（commands/mirror_executor.rs）拥有 `event_id: i64`；
/// 我们使用 BigInt<i64> 属性（v0.85c 模式）→ TS `bigint`。
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
    // v0.88b —— 存根。commands/mirror_executor.rs:76 中的真实实现
    // 插入到 copy_mirror_queue 并返回 MirrorRow。我们返回
    // 硬编码的形状；MirrorRowCodegen 复用 v0.86b 的 BigInt
    // 包装器处理 submitted_at / filled_at。
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
// v0.88c —— Phase 4 batch 3：2 个 Trade 路由命令（输入 DTO）
// =================================================================
//
// `place_signed_order` 和 `place_jump_link` 的 drift 检测。
// 真实命令位于 commands/bet.rs。

/// v0.88c —— `BetDto` 的 codegen 存根。真实结构体包含 3× i64 字段
///（`placed_at`、`settled_at`、`signal_id`）。复用 v0.86b 的
/// OptionBigInt 包装器模式。
#[derive(Serialize, Deserialize, Type)]
struct BetDtoCodegen {
    pub id: String,
    pub wallet_id: String,
    pub market_id: String,
    /// v0.88c —— `signal_id: Option<i64>` → `Option<OptionBigInt<i64>>`
    ///（TS `bigint | null`）。对于 53 位以内的值无损。
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

/// v0.88c —— `place_signed_order` 的 codegen 存根参数。真实的
/// `PlaceSignedArgs`（commands/bet.rs）拥有 `signal_id: Option<i64>`
/// → BigInt 包装器以实现无损传输。
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
    // v0.88c —— 存根。commands/bet.rs 中的真实实现会验证 +
    // 签名 + 插入 bets 行 + 返回 BetDto。我们返回硬编码的形状，
    // 其中 placed_at=0 / signal_id=None / settled_at=None
    //（与"刚下单，尚未结算"的 bet 一致）。
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

/// v0.88c —— `place_jump_link` 的 codegen 存根参数。形状与
/// PlaceSignedArgs 相同，但不包含 order_type / limit_price /
/// stop_price / post_only 字段。signal_id: Option<i64> → BigInt。
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
    // v0.88c —— 存根。真实实现返回由 args.market_slug + args.side +
    // args.price 构建的 polymarket.com 跳转 URL。我们返回
    // 硬编码的 URL，使 codegen 导出正确的返回类型。
    let _ = args;
    Ok("https://polymarket.com/event/_stub_".to_string())
}

// =================================================================
// v0.88d —— Phase 4 batch 4：Settings + ModelLab + LlmMgmt
// =================================================================
//
// `set_audit_retention`、`set_auto_promote_config`、
// `upsert_llm_provider` 的 drift 检测。

/// v0.88d —— `set_audit_retention` 的 codegen 存根参数。真实的
/// `SetAuditRetentionArgs`（commands/audit.rs）包含 3× Option<i64>；
/// OptionBigInt 包装器（v0.86b）可实现无损传输。
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
    // v0.88d —— 存根。真实实现写入保留策略并立即
    // 执行一次清理，返回被清理的行数。我们返回 0，
    // 因为 codegen bin 无法构造 DB 状态。u32 而非
    // usize，因为 specta-typescript 禁止 usize 导出。
    Ok(0)
}

/// v0.88d —— `upsert_llm_provider` 的 codegen 存根。直接接受
/// `LlmProviderDto`（不嵌套在 args 下）—— 与真实命令形状相同。
/// `timeout_ms: i64` 需要 BigInt 处理。
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
    // v0.88d —— 存根。真实实现执行 INSERT ... ON CONFLICT UPDATE。
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn set_auto_promote_config_codegen(
    args: polyrocket_lib::commands::sidecar::SetAutoPromoteConfigArgs,
) -> Result<polyrocket_lib::commands::sidecar::AutoPromoteConfigDto, String> {
    // v0.88d —— 存根。真实实现读取当前配置，应用
    // 部分更新，然后写回。我们返回默认配置，使
    // codegen 导出正确的返回类型。
    let _ = args;
    Ok(polyrocket_lib::commands::sidecar::AutoPromoteConfigDto {
        enabled: false,
        brier_margin: 0.005,
    })
}

// =================================================================
// v0.88e —— Phase 4 batch 5：复杂嵌套 DTO（LLM + mirror executor）
// =================================================================
//
// `llm_analyze` 和 `run_mirror_executor_pass` 的 drift 检测。
// 它们包含嵌套的 Vec<i64> + 多个 Option<i64> 字段，
// 端到端地测试 BigInt 包装器（这里不需要
// OptionBigInt + BigIntMap —— 只需要为 Option<i64> 使用 OptionBigInt，
// 并为裸 i64 使用 `specta(type=BigInt)`）。

/// v0.88e —— `LlmRecommendationDto` 的 codegen 存根。真实结构体包含 4× i64
/// 字段（`id`、`latency_ms`、`tokens_in`、`tokens_out`）。3 个
/// Option<i64> 使用 OptionBigInt；裸的 `id: i64` 使用
/// `specta(type = BigInt)`。
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

/// v0.88e —— `LlmAnalysisDto` 的 codegen 存根。真实结构体包含 4× i64 字段
///（`signal_id`、`requested_at`、`completed_at`、`total_latency_ms`）
/// + 嵌套的 Vec<LlmRecommendationDto>。这端到端地测试了
/// OptionBigInt + 嵌套结构体的 codegen 路径。
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

/// v0.88e —— `llm_analyze` 的 codegen 存根参数。真实的 `AnalyzeArgs`
/// 拥有 `signal_id: Option<i64>` → OptionBigInt；其他字段较为简单。
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
    // v0.88e —— 存根。真实实现将请求分发到已启用的 providers，
    // 收集响应，写入 llm_analyses + llm_recommendations
    // 行，并返回聚合后的 DTO。我们返回硬编码的形状，
    // recommendations vec 为空。
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

/// v0.88e —— `ExecutorPassResult` 的 codegen 存根。真实结构体没有 i64
/// 字段（只有 Vec<String>、Vec<(String, RejectReason)>、两个 f64）。
/// 通过 specta::Type derive 复用真实类型 —— 但目前真实
/// 类型没有 `Type`。我们在存根中镜像其形状。
#[derive(Serialize, Deserialize, Type)]
struct ExecutorPassResultCodegen {
    pub picked: Vec<String>,
    pub rejected: Vec<(String, String)>, // (market_id, reject_reason)
    pub current_exposure: f64,
    pub headroom: f64,
}

/// v0.88e —— `run_mirror_executor_pass` 的 codegen 存根参数。真实的
/// `RunPassArgs` 没有 i64 字段（只有 2× String）。
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
    // v0.88e —— 存根。真实实现读取 mirror queue，挑选通过
    // size + horizon + exposure 过滤的订单，提交已签名的订单，
    // 返回 ExecutorPassResult。我们返回硬编码的空结果。
    Ok(ExecutorPassResultCodegen {
        picked: vec![],
        rejected: vec![],
        current_exposure: 0.0,
        headroom: 0.0,
    })
}

// ---------- MirrorQueueStats（mirror_queue_stats）----------

/// v0.84c —— `MirrorQueueStats` 的 codegen 存根。真实结构体包含 5× i64
/// 计数（`n_pending`、`n_submitted`、`n_filled`、`n_rejected`、
/// `n_expired`）。
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
// v0.81 —— Bankroll 命令（Phase 2 —— 4 个命令）
// =================================================================
//
// 对于 codegen，我们使用与真实 L1 IPC 签名匹配的存根命令。
// 真实命令需要 Tauri State 来访问 DB；
// 存根返回硬编码数据。v0.81 的目的在于验证
// 生成的 TS 类型与 `src/types/bankroll.ts` 和
// `src/ipc.ts` 中手写的 DTO 相匹配。

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
    // 将 codegen DTO 转换回真实类型
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
    // v0.81 —— 存根。真实实现从 DB 读取。
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
    // v0.81 —— 存根。真实实现会校验并写入 DB。
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
    // v0.81 —— 存根。真实实现写入 allocation_batches + bets。
    Ok("00000000-0000-0000-0000-000000000000".to_string())
}

// =================================================================
// v0.98 —— Phase 4 batch 6：另外 4 个只读 list 命令
// =================================================================
//
// 用于 /history、/audit、/copy 以及 ModelLab 历史面板的
// 只读 list 端点 drift 检测。
// 每个存根返回正确形状的硬编码空数组；drift 检测器
// 会捕捉真实 DTO 中任何未来的重命名 / 类型变更。
// L1 仍然调用真实命令。

/// v0.98 —— `list_bets` 的 codegen 存根。真实返回类型为
/// `Vec<BetDto>`。空存根。
#[tauri::command]
#[specta::specta]
async fn list_bets_codegen(
    _args: ListBetsArgsCodegen,
) -> Result<Vec<BetDtoCodegen>, String> {
    Ok(vec![])
}

/// v0.98 —— `list_audit_log` 的 codegen 存根。真实返回类型为
/// `Vec<AuditEntry>`。空存根。
#[tauri::command]
#[specta::specta]
async fn list_audit_log_codegen(
    _args: ListAuditLogArgsCodegen,
) -> Result<Vec<AuditEntryDtoCodegen>, String> {
    Ok(vec![])
}

/// v0.98 —— `list_copy_targets` 的 codegen 存根。真实返回类型为
/// `Vec<CopyTarget>`。空存根。
#[tauri::command]
#[specta::specta]
async fn list_copy_targets_codegen() -> Result<Vec<CopyTargetDtoCodegen>, String> {
    Ok(vec![])
}

/// v0.98 —— `list_promote_history` 的 codegen 存根。真实返回类型为
/// `PromoteHistoryResult`（复用 v0.76 中的 `listPromoteHistory`）。
/// 空存根。
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
// v0.101a —— Phase 4 batch 7：LLM stats heatmap + scatter
//                     + timeseries + decision（4 个只读命令）
// =================================================================
//
// L1 "LLM Performance" 页面的 drift 检测。每个
// 存根返回空 Vec；drift 检测器会捕捉真实 DTO 中任何
// 未来的重命名 / 类型变更。i64 字段使用
// `#[specta(type = BigInt)]`（时间戳）或
// 遵循 v0.98 模式的 `i32` 占位符（计数）。

/// v0.101a —— `llm_stats_heatmap` 的参数。与真实 `StatsArgs` 匹配。
#[derive(Serialize, Deserialize, Type)]
struct StatsArgsCodegen {
    pub provider_id: Option<String>,
    pub category: Option<String>,
    /// v0.101a —— window_days 占位符（真实实现使用
    /// 包在 OptionBigInt 中的 Option<i64>）。存根使用 Option<u32> 用于 ts 导出。
    pub window_days: Option<u32>,
}

/// v0.101a —— LlmStatsCell 形状。计数使用 i32（仅用于 drift 检测）。
#[derive(Serialize, Deserialize, Type)]
struct LlmStatsCellCodegen {
    pub provider_id: String,
    pub provider_name: String,
    pub category: String,
    /// v0.101a —— 占位符（真实实现使用 i64）。存根使用 i32。
    pub n_recommendations: i32,
    pub n_evaluated: i32,
    pub win_rate: Option<f64>,
    pub avg_pnl: Option<f64>,
    pub brier: Option<f64>,
}

/// v0.101a —— `llm_stats_heatmap` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn llm_stats_heatmap_codegen(
    _args: StatsArgsCodegen,
) -> Result<Vec<LlmStatsCellCodegen>, String> {
    Ok(vec![])
}

/// v0.101a —— LlmStatsScatterPoint 形状。
#[derive(Serialize, Deserialize, Type)]
struct LlmStatsScatterPointCodegen {
    pub provider_id: String,
    pub provider_name: String,
    /// v0.101a —— 占位符（真实实现使用 i64）。存根使用 i32。
    pub n_evaluated: i32,
    pub win_rate: f64,
    pub total_pnl: f64,
    pub avg_pnl: f64,
}

/// v0.101a —— `llm_stats_scatter` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn llm_stats_scatter_codegen(
    window_days: Option<u32>,
) -> Result<Vec<LlmStatsScatterPointCodegen>, String> {
    Ok(vec![])
}

/// v0.101a —— LlmStatsTimeseriesPoint 形状。
#[derive(Serialize, Deserialize, Type)]
struct LlmStatsTimeseriesPointCodegen {
    pub provider_id: String,
    pub bucket: String,
    /// v0.101a —— 占位符（真实实现使用 i64）。存根使用 i32。
    pub n_evaluated: i32,
    pub win_rate: f64,
    pub brier: f64,
}

/// v0.101a —— `llm_stats_timeseries` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn llm_stats_timeseries_codegen(
    window_days: Option<u32>,
) -> Result<Vec<LlmStatsTimeseriesPointCodegen>, String> {
    Ok(vec![])
}

/// v0.101a —— LlmDecisionStats 形状。
#[derive(Serialize, Deserialize, Type)]
struct LlmDecisionStatsCodegen {
    pub category: String,
    pub decision_type: String,
    /// v0.101a —— 占位符（真实实现使用 i64）。存根使用 i32。
    pub n: i32,
    pub win_rate: f64,
    pub avg_pnl: f64,
}

/// v0.101a —— `llm_stats_decision` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn llm_stats_decision_codegen(
    window_days: Option<u32>,
) -> Result<Vec<LlmDecisionStatsCodegen>, String> {
    Ok(vec![])
}

// =================================================================
// v0.101b —— Phase 4 batch 7 part 2：LLM stats by_confidence
//                     + by_prompt + cost_efficiency + export（4 个命令）
// =================================================================
//
// v0.101a batch 7 的延续 —— 覆盖 L1 "LLM Management" 页面的
// confidence / prompt / cost 面板以及 CSV/JSON 导出。

/// v0.101b —— `llm_stats_by_confidence` 的参数。与真实 `StatsByConfidenceArgs` 匹配。
#[derive(Serialize, Deserialize, Type)]
struct StatsByConfidenceArgsCodegen {
    pub provider_id: Option<String>,
    /// v0.101b —— 占位符（真实实现使用 Option<i64>）。存根使用 Option<u32>。
    pub window_days: Option<u32>,
}

/// v0.101b —— LlmStatsConfidenceBand 形状。
#[derive(Serialize, Deserialize, Type)]
struct LlmStatsConfidenceBandCodegen {
    pub provider_id: String,
    pub band: String,
    /// v0.101b —— 占位符（真实实现使用 i64）。存根使用 i32。
    pub n: i32,
    pub win_rate: f64,
    pub avg_pnl: f64,
}

/// v0.101b —— `llm_stats_by_confidence` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn llm_stats_by_confidence_codegen(
    _args: StatsByConfidenceArgsCodegen,
) -> Result<Vec<LlmStatsConfidenceBandCodegen>, String> {
    Ok(vec![])
}

/// v0.101b —— `llm_stats_by_prompt` 的参数。与真实 `StatsByPromptArgs` 匹配。
#[derive(Serialize, Deserialize, Type)]
struct StatsByPromptArgsCodegen {
    pub prompt_version: String,
    /// v0.101b —— 占位符（真实实现使用 Option<i64>）。存根使用 Option<u32>。
    pub window_days: Option<u32>,
}

/// v0.101b —— LlmStatsByPrompt 形状。
#[derive(Serialize, Deserialize, Type)]
struct LlmStatsByPromptCodegen {
    pub provider_id: String,
    pub prompt_version: String,
    /// v0.101b —— 占位符（真实实现使用 i64）。存根使用 i32。
    pub n_recommendations: i32,
    pub n_evaluated: i32,
    pub win_rate: f64,
    pub brier: f64,
}

/// v0.101b —— `llm_stats_by_prompt` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn llm_stats_by_prompt_codegen(
    _args: StatsByPromptArgsCodegen,
) -> Result<Vec<LlmStatsByPromptCodegen>, String> {
    Ok(vec![])
}

/// v0.101b —— LlmStatsCostEfficiency 形状。
#[derive(Serialize, Deserialize, Type)]
struct LlmStatsCostEfficiencyCodegen {
    pub provider_id: String,
    pub total_cost_cents: f64,
    pub total_pnl: f64,
    pub efficiency: f64,
    pub win_rate: f64,
}

/// v0.101b —— `llm_stats_cost_efficiency` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn llm_stats_cost_efficiency_codegen(
    window_days: Option<u32>,
) -> Result<Vec<LlmStatsCostEfficiencyCodegen>, String> {
    Ok(vec![])
}

/// v0.101b —— `llm_stats_export` 的参数。与真实 `ExportStatsArgs` 匹配。
#[derive(Serialize, Deserialize, Type)]
struct ExportStatsArgsCodegen {
    pub format: String,
    /// v0.101b —— 占位符（真实实现使用 Option<i64>）。存根使用 Option<u32>。
    pub window_days: Option<u32>,
}

/// v0.101b —— `llm_stats_export` 的 codegen 存根。返回字符串
///（CSV 或 JSON），L1 层通过 tauri-plugin-fs 保存。
#[tauri::command]
#[specta::specta]
async fn llm_stats_export_codegen(
    _args: ExportStatsArgsCodegen,
) -> Result<String, String> {
    Ok(String::new())
}

// =================================================================
// v0.101c —— Phase 4 batch 7 part 3：LLM traffic_summary + 4 个 scheduler
//                     命令（5 个只读 + 2 个 trigger + 1 个 self_test）
// =================================================================
//
// L1 "LLM Mgmt traffic" 面板以及 Settings "Scheduler" + "Self Test"
// 卡片的 drift 检测。trigger 命令与真实命令一样
// 返回 Result<TriggerResult, String>；
// self_test 返回 SchedulerSelfTest 同步结果（无需 State）。

/// v0.101c —— `llm_traffic_summary` 的参数。与真实 `TrafficArgs` 匹配。
#[derive(Serialize, Deserialize, Type)]
struct TrafficArgsCodegen {
    pub window: Option<String>,
    pub provider_id: Option<String>,
}

/// v0.101c —— LlmTrafficSummary 形状。许多 i64 字段使用
/// `#[specta(type = BigInt)]` 以实现无损导出（真实计数
/// 在长时间窗口下可能超过 2^32）。
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

/// v0.101c —— `llm_traffic_summary` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn llm_traffic_summary_codegen(
    _args: TrafficArgsCodegen,
) -> Result<Vec<LlmTrafficSummaryCodegen>, String> {
    Ok(vec![])
}

/// v0.101c —— SchedulerStatus 形状。next_brief_run_at_unix_ms 使用
/// `#[specta(type = BigInt)]` 实现无损时间戳。
#[derive(Serialize, Deserialize, Type)]
struct SchedulerStatusCodegen {
    /// v0.101c —— 占位符（真实实现使用 u64）。存根使用 u32，
    /// 因为 specta-typescript 禁止 u64 原样导出。
    pub health_probe_interval_sec: u32,
    pub daily_brief_hour_utc: u32,
    pub daily_brief_tz_offset_min: i32,
    /// v0.101c —— 占位符（真实实现使用 u64）。存根使用 u32。
    pub anomaly_window_sec: u32,
    #[specta(type = BigInt)]
    pub next_brief_run_at_unix_ms: i64,
}

/// v0.101c —— `scheduler_status` 的 codegen 存根。
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

/// v0.101c —— TriggerResult 形状。triggered_at_unix_ms 使用
/// `#[specta(type = BigInt)]`。
#[derive(Serialize, Deserialize, Type)]
struct TriggerResultCodegen {
    #[specta(type = BigInt)]
    pub triggered_at_unix_ms: i64,
    pub kind: String,
    pub ok: bool,
    pub error: Option<String>,
}

/// v0.101c —— `scheduler_run_health_probe_now` 的 codegen 存根。
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

/// v0.101c —— `scheduler_run_daily_brief_now` 的 codegen 存根。
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

/// v0.101c —— LoopStatus 形状。真实类型对 `name` 使用 `&'static str`，
/// 但存根使用 `String`，因为 specta 对 `&'static str`
/// 的处理没问题，但我们希望让 codegen 存根保持自包含。
#[derive(Serialize, Deserialize, Type)]
struct LoopStatusCodegen {
    pub name: String,
    #[specta(type = BigInt)]
    pub last_tick_unix_ms: u64,
    /// v0.101c —— 占位符（真实实现使用 Option<u64>）。存根
    /// 使用 Option<u32>，因为 specta-typescript 即使加上
    /// #[specta(type = BigInt)] 也禁止 Option 中的 u64。
    pub age_ms: Option<u32>,
    pub healthy: bool,
}

/// v0.101c —— SchedulerSelfTest 形状。与真实 SchedulerSelfTest 匹配。
#[derive(Serialize, Deserialize, Type)]
struct SchedulerSelfTestCodegen {
    #[specta(type = BigInt)]
    pub process_started_at_unix: u64,
    #[specta(type = BigInt)]
    pub checked_at_unix_ms: u64,
    pub all_healthy: bool,
    pub loops: Vec<LoopStatusCodegen>,
}

/// v0.101c —— `scheduler_self_test_now` 的 codegen 存根。真实实现
/// 是同步的（非 async）—— 仅读取原子变量。
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
// v0.102b —— Phase 4 batch 8：degradation + audit purge +
//                     daily_brief（6 个只读 / 写命令）
// =================================================================
//
// L1 Settings 的 "Degradation"、"Audit retention purge now"
// 以及 Dashboard "Daily Brief" 面板的 drift 检测。
// daily_brief_set_prefs 使用内部结构体（BriefWeights）——
// specta 支持嵌套 Type。

// ---------- degradation_check_now ----------
/// v0.102b —— 空参数（真实实现中的 DegradationCheckNowArgs）。
#[derive(Serialize, Deserialize, Type, Default)]
struct DegradationCheckNowArgsCodegen {}

/// v0.102b —— `degradation_check_now` 的 codegen 存根。返回
/// () —— 副作用是触发一个 telemetry 事件。
#[tauri::command]
#[specta::specta]
async fn degradation_check_now_codegen(
    _args: DegradationCheckNowArgsCodegen,
) -> Result<(), String> {
    Ok(())
}

// ---------- purge_audit_log_now ----------
/// v0.102b —— `purge_audit_log_now` 的 codegen 存根。返回
/// `usize` 表示被清理的行数。
#[tauri::command]
#[specta::specta]
async fn purge_audit_log_now_codegen() -> Result<u32, String> {
    Ok(0)
}

// ---------- daily_brief_get ----------
/// v0.102b —— `daily_brief_get` 的参数。与真实 `BriefGetArgs` 匹配。
#[derive(Serialize, Deserialize, Type, Default)]
struct BriefGetArgsCodegen {
    /// v0.102b —— 占位符（真实实现使用 Option<i64>）。
    /// 存根使用 Option<u32>。
    pub limit: Option<u32>,
    /// v0.102b —— 占位符（真实实现使用 Option<i64>）。
    /// 存根使用 Option<u32>（为旧调用方保留向后兼容）。
    pub max_items: Option<u32>,
}

/// v0.102b —— DailyBriefEntry 形状。时间戳使用
/// `#[specta(type = BigInt)]` 以实现无损导出；计数使用 i32。
#[derive(Serialize, Deserialize, Type)]
struct DailyBriefEntryCodegen {
    pub market_id: String,
    pub market_question: String,
    pub market_category: String,
    #[specta(type = BigInt)]
    pub market_end_date: i64,
    pub market_liquidity: Option<String>,
    pub market_volume_24h: Option<f64>,
    /// v0.102b —— 占位符（真实实现使用 i64）。存根使用 i32。
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

/// v0.102b —— `daily_brief_get` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn daily_brief_get_codegen(
    _args: BriefGetArgsCodegen,
) -> Result<Vec<DailyBriefEntryCodegen>, String> {
    Ok(vec![])
}

// ---------- daily_brief_refresh ----------
/// v0.102b —— BriefRefreshResult 形状。
#[derive(Serialize, Deserialize, Type)]
struct BriefRefreshResultCodegen {
    #[specta(type = BigInt)]
    pub computed_at: i64,
    /// v0.102b —— 占位符（真实实现使用 i64）。存根使用 i32。
    pub n_items: i32,
}

/// v0.102b —— `daily_brief_refresh` 的 codegen 存根。
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
/// v0.102b —— `daily_brief_dismiss` 的参数。仅包含 Market ID。
#[derive(Serialize, Deserialize, Type, Default)]
struct BriefDismissArgsCodegen {
    pub market_id: String,
}

/// v0.102b —— `daily_brief_dismiss` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn daily_brief_dismiss_codegen(
    _args: BriefDismissArgsCodegen,
) -> Result<bool, String> {
    Ok(true)
}

// ---------- daily_brief_set_prefs ----------
/// v0.102b —— BriefWeights 形状（嵌套结构体）。
#[derive(Serialize, Deserialize, Type)]
struct BriefWeightsCodegen {
    pub w1: f64,
    pub w2: f64,
    pub w3: f64,
    pub w4: f64,
    pub w5: f64,
    pub w6: f64,
}

/// v0.102b —— `daily_brief_set_prefs` 的参数。与真实
/// `SetBriefPrefsArgs` 匹配。嵌套 Option<BriefWeights>。
#[derive(Serialize, Deserialize, Type)]
struct SetBriefPrefsArgsCodegen {
    pub user_id: String,
    pub weights: Option<BriefWeightsCodegen>,
    /// v0.102b —— 占位符（真实实现使用 Option<i64>）。
    pub max_items: Option<u32>,
    pub min_liquidity: Option<String>,
    pub categories: Option<Vec<String>>,
}

/// v0.102b —— `daily_brief_set_prefs` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn daily_brief_set_prefs_codegen(
    _args: SetBriefPrefsArgsCodegen,
) -> Result<(), String> {
    Ok(())
}

// =================================================================
// v0.103b —— Phase 4 batch 9 part 1：LLM provider CRUD（4 个命令）
// =================================================================
//
// L1 "LLM Management" 页面的 drift 检测。真实的
// LlmProviderDto 拥有 25+ 字段 —— 我们对
// 计数/时间字段使用 `i32` 占位符,
// 对 last_health_check_at 时间戳使用 `#[specta(type = BigInt)]`。

/// v0.103b —— 完整 LlmProviderDto 形状。许多 i64 字段使用 i32
/// 占位符(仅用于 drift 检测,L1 仍使用 `number`)。
/// 注意:v0.88d 中的 `LlmProviderDtoCodegen`(较小,用于
/// upsert_llm_provider)已存在;此处我们使用 `LlmProviderDtoFullCodegen`
/// 以避免名称冲突。
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
    /// v0.103b —— 占位符(真实实现使用 i64)。存根使用 i32。
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
    /// v0.103b —— 时间戳(真实实现使用 Option<i64>)。我们使用 i32
    /// 占位符(仅用于 drift 检测)—— Option<i64> 需要
    /// OptionBigInt 才能实现完全无损导出。
    pub last_health_check_at: Option<i32>,
    pub last_health_error: Option<String>,
    pub notes: Option<String>,
}

/// v0.103b —— `llm_provider_list` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn llm_provider_list_codegen(
) -> Result<Vec<LlmProviderDtoFullCodegen>, String> {
    Ok(vec![])
}

/// v0.103b —— `llm_provider_upsert` 的 codegen 存根。接收
/// 完整的 LlmProviderDto。
#[tauri::command]
#[specta::specta]
async fn llm_provider_upsert_codegen(
    _provider: LlmProviderDtoFullCodegen,
) -> Result<(), String> {
    Ok(())
}

/// v0.103b —— `llm_provider_delete` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn llm_provider_delete_codegen(
    _provider_id: String,
) -> Result<(), String> {
    Ok(())
}

// =================================================================
// v0.103b2 —— Phase 4 batch 9 part 2：LLM key CRUD + connectivity
//                      + health + performance（7 个命令）
// =================================================================
//
// v0.103b part 1 的延续 —— 覆盖其余 LLM
// 管理界面:key CRUD、连通性测试、health
// 历史记录以及聚合性能。

/// v0.103b2 —— LlmProviderKeyDto 形状。计数字段使用 i32。
#[derive(Serialize, Deserialize, Type)]
struct LlmProviderKeyDtoCodegen {
    pub id: String,
    pub provider_id: String,
    pub alias: String,
    pub keyring_alias: String,
    pub enabled: bool,
    /// v0.103b2 —— 占位符(真实实现使用 i64)。存根使用 i32。
    pub priority: i32,
    pub weight: i32,
    /// v0.103b2 —— 时间戳(真实实现使用 Option<i64>)。i32 占位符。
    pub last_used_at: Option<i32>,
    pub last_error: Option<String>,
    /// v0.103b2 —— 时间戳(真实实现使用 Option<i64>)。i32 占位符。
    pub last_error_at: Option<i32>,
    pub total_calls: i32,
    pub total_errors: i32,
    pub notes: Option<String>,
}

/// v0.103b2 —— `llm_key_list` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn llm_key_list_codegen(
    _provider_id: Option<String>,
) -> Result<Vec<LlmProviderKeyDtoCodegen>, String> {
    Ok(vec![])
}

/// v0.103b2 —— `llm_key_upsert` 的参数。嵌套的 LlmProviderKeyDto。
#[derive(Serialize, Deserialize, Type)]
struct KeyUpsertArgsCodegen {
    pub key: LlmProviderKeyDtoCodegen,
    /// 明文密钥(可选)。提供时写入 OS
    /// 钥匙串;为 None 时,仅 upsert 元数据。
    pub secret: Option<String>,
}

/// v0.103b2 —— `llm_key_upsert` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn llm_key_upsert_codegen(
    _args: KeyUpsertArgsCodegen,
) -> Result<(), String> {
    Ok(())
}

/// v0.103b2 —— `llm_key_set_secret` 的参数。
#[derive(Serialize, Deserialize, Type)]
struct KeySetSecretArgsCodegen {
    pub key_id: String,
    pub secret: String,
}

/// v0.103b2 —— `llm_key_set_secret` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn llm_key_set_secret_codegen(
    _args: KeySetSecretArgsCodegen,
) -> Result<(), String> {
    Ok(())
}

/// v0.103b2 —— `llm_key_delete` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn llm_key_delete_codegen(
    _key_id: String,
) -> Result<(), String> {
    Ok(())
}

/// v0.103b2 —— `llm_test_connectivity` 的参数。
#[derive(Serialize, Deserialize, Type)]
struct TestConnectivityArgsCodegen {
    pub provider_id: String,
    pub key_id: Option<String>,
}

/// v0.103b2 —— ConnectivityTestResult 形状。状态字段使用
/// i32 占位符表示计数。
#[derive(Serialize, Deserialize, Type)]
struct ConnectivityTestResultCodegen {
    pub provider_id: String,
    pub success: bool,
    /// v0.103b2 —— 占位符(真实实现使用 Option<i64>)。
    pub latency_ms: Option<i32>,
    /// v0.103b2 —— 占位符(真实实现使用 Option<i64>)。
    pub http_status: Option<i32>,
    pub model_used: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

/// v0.103b2 —— `llm_test_connectivity` 的 codegen 存根。
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

/// v0.103b2 —— LlmHealthCheckDto 形状。时间戳使用 i32 占位符。
#[derive(Serialize, Deserialize, Type)]
struct LlmHealthCheckDtoCodegen {
    /// v0.103b2 —— 占位符(真实实现使用 i64)。i32。
    pub id: i32,
    pub provider_id: String,
    /// v0.103b2 —— 时间戳占位符(真实实现使用 i64)。i32。
    pub checked_at: i32,
    pub trigger: String,
    pub success: bool,
    pub latency_ms: Option<i32>,
    pub http_status: Option<i32>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

/// v0.103b2 —— `llm_health_history` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn llm_health_history_codegen(
    _provider_id: String,
    _limit: Option<u32>,
) -> Result<Vec<LlmHealthCheckDtoCodegen>, String> {
    Ok(vec![])
}

/// v0.103b2 —— LlmPerformanceRow 形状。计数字段使用 i32。
#[derive(Serialize, Deserialize, Type)]
struct LlmPerformanceRowCodegen {
    pub provider_id: String,
    pub provider_name: String,
    /// v0.103b2 —— 占位符(真实实现使用 i64)。i32。
    pub n_recommendations: i32,
    pub n_evaluated: i32,
    pub win_rate: f64,
    pub brier: f64,
    pub avg_confidence: f64,
    pub total_cost_cents: f64,
}

/// v0.103b2 —— `llm_performance` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn llm_performance_codegen(
    _window_days: Option<u32>,
    _category: Option<String>,
) -> Result<Vec<LlmPerformanceRowCodegen>, String> {
    Ok(vec![])
}

// =================================================================
// v0.104b —— Phase 4 batch 10：sidecar lifecycle + train +
//                     markets + wallet key mgmt（8 个命令）
// =================================================================
//
// L1 ModelLab / Settings / Wallets 面板的 drift 检测。
// 包含复杂的 TrainResult(嵌套 TrainTrialDto + serde_json::Value)
// 以及 SidecarHealthSnapshot（5 个 i64 字段）。

/// v0.104b —— TrainTrialDto 形状。计数字段使用 i32。
#[derive(Serialize, Deserialize, Type)]
struct TrainTrialDtoCodegen {
    /// v0.104b —— 占位符(真实实现使用 i64)。i32。
    pub trial_index: i32,
    /// v0.104b —— 占位符(真实实现使用 i64)。i32。
    pub duration_ms: i32,
    pub brier: f64,
    /// v0.104b —— 真实实现中 params 是 serde_json::Value。
    /// 存根使用 String(仅用于 drift 检测 —— L1 保留为
    /// `Record<string, unknown>`)。
    pub params: String,
}

/// v0.104b —— TrainResult 形状。
#[derive(Serialize, Deserialize, Type)]
struct TrainResultCodegen {
    pub job_id: String,
    pub status: String,
    pub best_brier: Option<f64>,
    /// v0.104b —— 真实实现中 best_params 是 Option<serde_json::Value>。
    /// 存根使用 Option<String>(仅用于 drift 检测)。
    pub best_params: Option<String>,
    pub trials: Vec<TrainTrialDtoCodegen>,
    /// v0.104b —— duration_ms(真实实现使用 i64)。i32 占位符。
    pub duration_ms: i32,
    pub candidate_path: Option<String>,
    pub message: Option<String>,
}

/// v0.104b —— `train_job` 的 codegen 存根。接收 TrainJobArgs。
#[tauri::command]
#[specta::specta]
async fn train_job_codegen(
    _args: TrainJobArgsCodegen,
) -> Result<TrainResultCodegen, String> {
    Ok(TrainResultCodegen {
        job_id: String::new(),
        status: "completed".to_string(),
        best_brier: None,
        best_params: None,
        trials: vec![],
        duration_ms: 0,
        candidate_path: None,
        message: None,
    })
}

/// v0.104b —— `train_job` 的参数。真实 TrainJobArgs 拥有
/// n_trials: Option<u32>、epochs: Option<u32>、timeout_ms: Option<u64>。
/// 存根使用 u32 占位符(对 u64 的 BigInt 禁止变通方案)。
#[derive(Serialize, Deserialize, Type, Default)]
struct TrainJobArgsCodegen {
    pub n_trials: Option<u32>,
    pub epochs: Option<u32>,
    /// v0.104b —— 占位符(真实实现使用 Option<u64>)。
    /// 存根使用 Option<u32>。
    pub timeout_ms: Option<u32>,
}

/// v0.104b —— `sync_markets` 的 codegen 存根。返回行数。
#[tauri::command]
#[specta::specta]
async fn sync_markets_codegen() -> Result<u32, String> {
    Ok(0)
}

/// v0.104b —— `recompute_signals` 的 codegen 存根。返回行数。
#[tauri::command]
#[specta::specta]
async fn recompute_signals_codegen() -> Result<u32, String> {
    Ok(0)
}

/// v0.104b —— SidecarHealthKind 枚举。
#[derive(Serialize, Deserialize, Type)]
enum SidecarHealthKindCodegen {
    Ok,
    Failed,
    Unknown,
}

/// v0.104b —— SidecarHealthRow 形状。
#[derive(Serialize, Deserialize, Type)]
struct SidecarHealthRowCodegen {
    /// v0.104b —— 占位符(真实实现使用 i64)。BigInt 实现无损。
    #[specta(type = BigInt)]
    pub at_ms: i64,
    pub kind: SidecarHealthKindCodegen,
    pub error: Option<String>,
}

/// v0.104b —— SidecarHealthSnapshot 形状。5 个 i64 字段使用 BigInt。
#[derive(Serialize, Deserialize, Type)]
struct SidecarHealthSnapshotCodegen {
    pub last_24h: Vec<SidecarHealthRowCodegen>,
    #[specta(type = BigInt)]
    pub success_count: i64,
    #[specta(type = BigInt)]
    pub failure_count: i64,
    #[specta(type = BigInt)]
    pub last_success_at_ms: Option<i64>,
    #[specta(type = BigInt)]
    pub last_failure_at_ms: Option<i64>,
}

/// v0.104b —— `sidecar_health_now` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn sidecar_health_now_codegen(
) -> Result<SidecarHealthSnapshotCodegen, String> {
    Ok(SidecarHealthSnapshotCodegen {
        last_24h: vec![],
        success_count: 0,
        failure_count: 0,
        last_success_at_ms: None,
        last_failure_at_ms: None,
    })
}

/// v0.104b —— `sidecar_health_snapshot` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn sidecar_health_snapshot_codegen(
) -> Result<SidecarHealthSnapshotCodegen, String> {
    Ok(SidecarHealthSnapshotCodegen {
        last_24h: vec![],
        success_count: 0,
        failure_count: 0,
        last_success_at_ms: None,
        last_failure_at_ms: None,
    })
}

/// v0.104b —— SidecarStatus 形状。`pid: Option<u32>` 没问题。
#[derive(Serialize, Deserialize, Type)]
struct SidecarStatusCodegen {
    pub running: bool,
    pub pid: Option<u32>,
    pub command: String,
    pub last_error: Option<String>,
}

/// v0.104b —— `start_sidecar` 的参数。
#[derive(Serialize, Deserialize, Type, Default)]
struct StartSidecarArgsCodegen {
    pub path: Option<String>,
    pub auto_restart: Option<bool>,
}

/// v0.104b —— `start_sidecar` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn start_sidecar_codegen(
    _args: StartSidecarArgsCodegen,
) -> Result<SidecarStatusCodegen, String> {
    Ok(SidecarStatusCodegen {
        running: false,
        pid: None,
        command: String::new(),
        last_error: None,
    })
}

/// v0.104b —— `stop_sidecar` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn stop_sidecar_codegen() -> Result<SidecarStatusCodegen, String> {
    Ok(SidecarStatusCodegen {
        running: false,
        pid: None,
        command: String::new(),
        last_error: None,
    })
}

/// v0.104b —— `polyrocket_wallet_set_pk` 的参数。真实的 WalletSetPkArgs。
#[derive(Serialize, Deserialize, Type, Default)]
struct WalletSetPkArgsCodegen {
    pub private_key: String,
    pub address: String,
    pub alias: Option<String>,
}

/// v0.104b —— `polyrocket_wallet_set_pk` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn polyrocket_wallet_set_pk_codegen(
    _args: WalletSetPkArgsCodegen,
) -> Result<(), String> {
    Ok(())
}

// =================================================================
// v0.105b —— Phase 4 batch 11：audit + promote + backtest（6 个命令）
// =================================================================
//
// L1 Audit/ModeLab/Backtest 面板的 drift 检测。包含
// 嵌套在 BacktestModelArgs 中的 BacktestSample 以及大量时间戳使用。

// ---------- audit_count_for_actor ----------
/// v0.105b —— `audit_count_for_actor` 的 codegen 存根。返回
/// `i64`(真实实现),存根使用 i32 占位符。
#[tauri::command]
#[specta::specta]
async fn audit_count_for_actor_codegen(
    _actor: String,
) -> Result<u32, String> {
    Ok(0)
}

// ---------- rollback_model ----------
/// v0.105b —— RollbackModelArgs 形状。
#[derive(Serialize, Deserialize, Type, Default)]
struct RollbackModelArgsCodegen {
    pub model_version: String,
}

/// v0.105b —— RollbackResult 形状。时间戳使用 BigInt。
#[derive(Serialize, Deserialize, Type)]
struct RollbackResultCodegen {
    pub rolled_back: bool,
    pub status: String,
    pub previous_path: Option<String>,
    pub active_path: Option<String>,
    /// v0.105b —— 时间戳占位符(真实实现使用 Option<i64>)。
    /// BigInt 实现无损导出。
    #[specta(type = BigInt)]
    pub rolled_back_at_ms: Option<i64>,
}

/// v0.105b —— `rollback_model` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn rollback_model_codegen(
    _args: RollbackModelArgsCodegen,
) -> Result<RollbackResultCodegen, String> {
    Ok(RollbackResultCodegen {
        rolled_back: false,
        status: String::new(),
        previous_path: None,
        active_path: None,
        rolled_back_at_ms: None,
    })
}

// ---------- auto_promote_if_better ----------
/// v0.105b —— AutoPromoteIfBetterArgs 形状。
#[derive(Serialize, Deserialize, Type, Default)]
struct AutoPromoteIfBetterArgsCodegen {
    /// v0.105b —— 占位符(真实实现使用 f64)。brier_margin 默认 0.005。
    pub brier_margin: f64,
    /// v0.105b —— 占位符(真实实现使用 Option<u32>)。存根使用 Option<u32>。
    pub trial_index: Option<u32>,
}

/// v0.105b —— AutoPromoteIfBetterResult 形状。
#[derive(Serialize, Deserialize, Type)]
struct AutoPromoteIfBetterResultCodegen {
    pub promoted: bool,
    pub skipped: bool,
    pub reason: String,
    pub candidate_brier: Option<f64>,
    pub active_brier: Option<f64>,
    pub margin: f64,
    pub model_version: Option<String>,
    /// v0.105b —— 时间戳占位符(真实实现使用 Option<i64>)。
    /// BigInt 实现无损导出。
    #[specta(type = BigInt)]
    pub promoted_at_ms: Option<i64>,
    pub message: Option<String>,
}

/// v0.105b —— `auto_promote_if_better` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn auto_promote_if_better_codegen(
    _args: AutoPromoteIfBetterArgsCodegen,
) -> Result<AutoPromoteIfBetterResultCodegen, String> {
    Ok(AutoPromoteIfBetterResultCodegen {
        promoted: false,
        skipped: false,
        reason: String::new(),
        candidate_brier: None,
        active_brier: None,
        margin: 0.005,
        model_version: None,
        promoted_at_ms: None,
        message: None,
    })
}

// ---------- backtest_model ----------
/// v0.105b —— BacktestSample 形状。全部为 f64。
#[derive(Serialize, Deserialize, Type)]
struct BacktestSampleCodegen {
    pub price: f64,
    pub market_age_hours: f64,
    pub outcome: f64,
    pub label: Option<String>,
}

/// v0.105b —— BacktestModelArgs 形状。嵌套 Vec<BacktestSample>。
#[derive(Serialize, Deserialize, Type)]
struct BacktestModelArgsCodegen {
    pub model_version: String,
    pub samples: Vec<BacktestSampleCodegen>,
}

/// v0.105b —— BacktestResult 形状。
#[derive(Serialize, Deserialize, Type)]
struct BacktestResultCodegen {
    pub ok: bool,
    pub model_version: String,
    /// v0.105b —— 占位符(真实实现使用 usize)。存根使用 u32。
    pub sample_count: u32,
    pub brier_mean: Option<f64>,
    pub brier_breakdown: Vec<f64>,
    /// [0, 1] 区间内的 5 个校准桶。每个都是 Vec<f64>。
    pub calibration: Vec<Vec<f64>>,
    /// v0.105b —— 占位符(真实实现使用 usize)。存根使用 u32。
    pub n_winners: u32,
    pub winners: Vec<String>,
    /// v0.105b —— 占位符(真实实现使用 usize)。存根使用 u32。
    pub n_losers: u32,
    pub losers: Vec<String>,
    pub message: Option<String>,
}

/// v0.105b —— `backtest_model` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn backtest_model_codegen(
    _args: BacktestModelArgsCodegen,
) -> Result<BacktestResultCodegen, String> {
    Ok(BacktestResultCodegen {
        ok: false,
        model_version: String::new(),
        sample_count: 0,
        brier_mean: None,
        brier_breakdown: vec![],
        calibration: vec![],
        n_winners: 0,
        winners: vec![],
        n_losers: 0,
        losers: vec![],
        message: None,
    })
}

// ---------- promote_model + promote_all_trials ----------
/// v0.105b —— PromoteModelArgs 形状。
#[derive(Serialize, Deserialize, Type, Default)]
struct PromoteModelArgsCodegen {
    pub model_version: Option<String>,
    /// v0.105b —— 占位符(真实实现使用 Option<u32>)。
    pub trial_index: Option<u32>,
}

/// v0.105b —— PromoteModelResult 形状。
#[derive(Serialize, Deserialize, Type)]
struct PromoteModelResultCodegen {
    pub ok: bool,
    pub message: String,
    /// v0.105b —— 占位符(真实实现使用 Option<i64>)。
    #[specta(type = BigInt)]
    pub promoted_at_ms: Option<i64>,
    pub model_version: Option<String>,
    pub best_brier: Option<f64>,
}

/// v0.105b —— `promote_model` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn promote_model_codegen(
    _args: PromoteModelArgsCodegen,
) -> Result<PromoteModelResultCodegen, String> {
    Ok(PromoteModelResultCodegen {
        ok: false,
        message: String::new(),
        promoted_at_ms: None,
        model_version: None,
        best_brier: None,
    })
}

/// v0.105b —— PromoteTrialResult 形状(由 promote_all_trials 使用)。
#[derive(Serialize, Deserialize, Type)]
struct PromoteTrialResultCodegen {
    pub trial_index: i32,
    pub promoted: bool,
    pub message: Option<String>,
    pub model_version: Option<String>,
    #[specta(type = BigInt)]
    pub promoted_at_ms: Option<i64>,
}

/// v0.105b —— PromoteAllTrialsResult 形状。
#[derive(Serialize, Deserialize, Type)]
struct PromoteAllTrialsResultCodegen {
    pub ok: bool,
    pub message: String,
    /// v0.105b —— 占位符(真实实现使用 usize)。存根使用 u32。
    pub count: u32,
    pub results: Vec<PromoteTrialResultCodegen>,
}

/// v0.105b —— `promote_all_trials` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn promote_all_trials_codegen(
) -> Result<PromoteAllTrialsResultCodegen, String> {
    Ok(PromoteAllTrialsResultCodegen {
        ok: false,
        message: String::new(),
        count: 0,
        results: vec![],
    })
}

// =================================================================
// v0.126 —— Phase 4 batch 12：Football Feature Pack
//                     (13 个 IPC 命令:arb/calendar/crowd_wisdom/
//                      cross_platform_arb/football_context/mean_reversion/
//                      news/nl_query/poisson/smart_money/spike/uma)
// =================================================================
//
// L1 Analysis / MarketDetail / ArbBoard / Signals 页面调用的 13 个
// football IPC。存根返回硬编码形状使 codegen 在没有 Tauri State 时
// 也能运行。drift 检测器捕捉未来 Rust DTO 的字段重命名 / 类型变更。
// i64 字段使用 `#[specta(type = BigInt)]` (时间戳)或 `i32` (计数)。

// ---------- arb ----------
/// v0.126 —— `ArbOpportunityDto` 形状。
#[derive(Serialize, Deserialize, Type)]
struct ArbOpportunityDtoCodegen {
    pub market_id: String,
    pub question: String,
    pub yes_cost: f64,
    pub no_cost: f64,
    pub total_cost: f64,
    pub profit_margin: f64,
    pub category: String,
}

/// v0.126 —— `arb_scan` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn arb_scan_codegen() -> Result<Vec<ArbOpportunityDtoCodegen>, String> {
    Ok(vec![])
}

/// v0.126 —— `list_arb_opportunities` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn list_arb_opportunities_codegen(
    _limit: Option<i32>,
) -> Result<Vec<ArbOpportunityDtoCodegen>, String> {
    Ok(vec![])
}

// ---------- calendar ----------
/// v0.126 —— `CalendarFixtureDto` 形状。
#[derive(Serialize, Deserialize, Type)]
struct CalendarFixtureDtoCodegen {
    pub market_id: String,
    pub home: String,
    pub away: String,
    pub time: String,
    pub edge: Option<f64>,
    pub competition: Option<String>,
}

/// v0.126 —— `CalendarDayDto` 形状。
#[derive(Serialize, Deserialize, Type)]
struct CalendarDayDtoCodegen {
    pub date: String,
    pub fixtures: Vec<CalendarFixtureDtoCodegen>,
}

/// v0.126 —— `market_calendar` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn market_calendar_codegen(
    _year: i32,
    _month: u32,
) -> Result<Vec<CalendarDayDtoCodegen>, String> {
    Ok(vec![])
}

// ---------- cross_platform_arb ----------
/// v0.126 —— `CrossPlatformArbDto` 形状。
#[derive(Serialize, Deserialize, Type)]
struct CrossPlatformArbDtoCodegen {
    pub match_name: String,
    pub market_question: String,
    pub pm_price: f64,
    pub kalshi_price: f64,
    pub spread: f64,
    pub direction: String,
    pub est_profit_per_1000: f64,
}

/// v0.126 —— `cross_platform_arb_scan` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn cross_platform_arb_scan_codegen(
) -> Result<Vec<CrossPlatformArbDtoCodegen>, String> {
    Ok(vec![])
}

// ---------- crowd_wisdom ----------
/// v0.126 —— `CrowdOpinionDto` 形状。
#[derive(Serialize, Deserialize, Type)]
struct CrowdOpinionDtoCodegen {
    pub market_id: String,
    pub yes_capital: f64,
    pub no_capital: f64,
    pub yes_weighted_pct: f64,
    pub no_weighted_pct: f64,
    pub hhi: f64,
    pub top3_share: f64,
    /// v0.126 —— 占位符(真实类型 usize)。存根使用 i32。
    pub n_holders: i32,
    pub smart_yes_pct: f64,
    pub smart_divergence: f64,
    #[specta(type = BigInt)]
    pub computed_at: i64,
}

/// v0.126 —— `crowd_opinion` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn crowd_opinion_codegen(
    _market_id: String,
) -> Result<CrowdOpinionDtoCodegen, String> {
    Ok(CrowdOpinionDtoCodegen {
        market_id: String::new(),
        yes_capital: 0.0,
        no_capital: 0.0,
        yes_weighted_pct: 0.0,
        no_weighted_pct: 0.0,
        hhi: 0.0,
        top3_share: 0.0,
        n_holders: 0,
        smart_yes_pct: 0.0,
        smart_divergence: 0.0,
        computed_at: 0,
    })
}

// ---------- football_context ----------
/// v0.126 —— `FootballContextDto` 形状。
#[derive(Serialize, Deserialize, Type)]
struct FootballContextDtoCodegen {
    pub market_id: String,
    pub home_team: String,
    pub away_team: String,
    /// v0.126 —— 占位符(真实类型 Option<i64>)。
    pub home_rest_days: Option<i32>,
    pub away_rest_days: Option<i32>,
    pub home_matches_7d: i32,
    pub away_matches_7d: i32,
    pub home_fatigue: String,
    pub away_fatigue: String,
}

/// v0.126 —— `get_football_context` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn get_football_context_codegen(
    _market_id: String,
) -> Result<FootballContextDtoCodegen, String> {
    Ok(FootballContextDtoCodegen {
        market_id: String::new(),
        home_team: String::new(),
        away_team: String::new(),
        home_rest_days: None,
        away_rest_days: None,
        home_matches_7d: 0,
        away_matches_7d: 0,
        home_fatigue: String::new(),
        away_fatigue: String::new(),
    })
}

// ---------- mean_reversion ----------
/// v0.126 —— `ReversionSignalDto` 形状。
#[derive(Serialize, Deserialize, Type)]
struct ReversionSignalDtoCodegen {
    pub market_id: String,
    pub current: f64,
    pub mean: f64,
    pub std_dev: f64,
    pub z_score: f64,
    pub bollinger_upper: f64,
    pub bollinger_lower: f64,
    pub is_overextended: bool,
    pub direction: String,
    /// v0.126 —— 占位符(真实类型 usize)。存根使用 i32。
    pub window_size: i32,
    pub fade_signal: f64,
    pub confidence: f64,
    #[specta(type = BigInt)]
    pub computed_at: i64,
}

/// v0.126 —— `reversion_signal` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn reversion_signal_codegen(
    _market_id: String,
    _window: Option<i32>,
) -> Result<ReversionSignalDtoCodegen, String> {
    Ok(ReversionSignalDtoCodegen {
        market_id: String::new(),
        current: 0.0,
        mean: 0.0,
        std_dev: 0.0,
        z_score: 0.0,
        bollinger_upper: 0.0,
        bollinger_lower: 0.0,
        is_overextended: false,
        direction: String::new(),
        window_size: 0,
        fade_signal: 0.0,
        confidence: 0.0,
        computed_at: 0,
    })
}

// ---------- news ----------
/// v0.126 —— `NewsItemDto` 形状。
#[derive(Serialize, Deserialize, Type)]
struct NewsItemDtoCodegen {
    #[specta(type = BigInt)]
    pub id: i64,
    pub title: String,
    pub source: String,
    pub url: String,
    #[specta(type = BigInt)]
    pub published_at: i64,
    pub market_id: Option<String>,
    pub relevance_score: Option<f64>,
    pub impact_direction: Option<String>,
    pub summary: Option<String>,
}

/// v0.126 —— `market_news` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn market_news_codegen(
    _market_id: String,
) -> Result<Vec<NewsItemDtoCodegen>, String> {
    Ok(vec![])
}

/// v0.126 —— `list_all_news` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn list_all_news_codegen(
    _limit: Option<i32>,
) -> Result<Vec<NewsItemDtoCodegen>, String> {
    Ok(vec![])
}

// ---------- nl_query ----------
/// v0.126 —— `NlQueryRowDto` 形状。
#[derive(Serialize, Deserialize, Type)]
struct NlQueryRowDtoCodegen {
    pub market_id: String,
    pub question: String,
    pub yes_price: Option<f64>,
    pub model_prob: Option<f64>,
    pub edge: Option<f64>,
    pub category: String,
}

/// v0.126 —— `NlQueryResultDto` 形状。
#[derive(Serialize, Deserialize, Type)]
struct NlQueryResultDtoCodegen {
    pub sql: String,
    pub results: Vec<NlQueryRowDtoCodegen>,
    pub explanation: String,
}

/// v0.126 —— `nl_query` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn nl_query_codegen(
    _query: String,
) -> Result<NlQueryResultDtoCodegen, String> {
    Ok(NlQueryResultDtoCodegen {
        sql: String::new(),
        results: vec![],
        explanation: String::new(),
    })
}

// ---------- poisson ----------
/// v0.126 —— `ScoreMatrixResultDto` 形状。
#[derive(Serialize, Deserialize, Type)]
struct ScoreMatrixResultDtoCodegen {
    pub market_id: String,
    pub lambda_h: f64,
    pub lambda_a: f64,
    pub matrix: [[f64; 5]; 5],
    /// v0.126 —— 元组数组(speca 支持 Vec<(String, f64)>)。
    pub most_likely: Vec<(String, f64)>,
}

/// v0.126 —— `poisson_score_matrix` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn poisson_score_matrix_codegen(
    _market_id: String,
) -> Result<ScoreMatrixResultDtoCodegen, String> {
    Ok(ScoreMatrixResultDtoCodegen {
        market_id: String::new(),
        lambda_h: 0.0,
        lambda_a: 0.0,
        matrix: [[0.0; 5]; 5],
        most_likely: vec![],
    })
}

// ---------- smart_money ----------
/// v0.126 —— `TopWalletDto` 形状。
#[derive(Serialize, Deserialize, Type)]
struct TopWalletDtoCodegen {
    pub address: String,
    pub pnl: f64,
    pub win_rate: f64,
}

/// v0.126 —— `SideBreakdownDto` 形状。
#[derive(Serialize, Deserialize, Type)]
struct SideBreakdownDtoCodegen {
    #[specta(type = BigInt)]
    pub wallet_count: i64,
    pub avg_pnl: f64,
    pub win_rate: f64,
    pub median_position: f64,
    pub top_wallets: Vec<TopWalletDtoCodegen>,
}

/// v0.126 —— `SmartMoneyScoreDto` 形状。
#[derive(Serialize, Deserialize, Type)]
struct SmartMoneyScoreDtoCodegen {
    pub market_id: String,
    pub yes_score: f64,
    pub no_score: f64,
    pub yes_breakdown: SideBreakdownDtoCodegen,
    pub no_breakdown: SideBreakdownDtoCodegen,
    #[specta(type = BigInt)]
    pub computed_at: i64,
}

/// v0.126 —— `smart_money_score` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn smart_money_score_codegen(
    _market_id: String,
) -> Result<SmartMoneyScoreDtoCodegen, String> {
    Ok(SmartMoneyScoreDtoCodegen {
        market_id: String::new(),
        yes_score: 0.0,
        no_score: 0.0,
        yes_breakdown: SideBreakdownDtoCodegen {
            wallet_count: 0,
            avg_pnl: 0.0,
            win_rate: 0.0,
            median_position: 0.0,
            top_wallets: vec![],
        },
        no_breakdown: SideBreakdownDtoCodegen {
            wallet_count: 0,
            avg_pnl: 0.0,
            win_rate: 0.0,
            median_position: 0.0,
            top_wallets: vec![],
        },
        computed_at: 0,
    })
}

// ---------- spike ----------
/// v0.126 —— `SpikeAlertDto` 形状。
#[derive(Serialize, Deserialize, Type)]
struct SpikeAlertDtoCodegen {
    #[specta(type = BigInt)]
    pub id: i64,
    pub market_id: String,
    pub old_price: f64,
    pub new_price: f64,
    pub change_pct: f64,
    #[specta(type = BigInt)]
    pub detected_at: i64,
    pub market_question: Option<String>,
}

/// v0.126 —— `list_spike_alerts` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn list_spike_alerts_codegen(
    _limit: Option<i32>,
) -> Result<Vec<SpikeAlertDtoCodegen>, String> {
    Ok(vec![])
}

/// v0.126 —— `run_spike_scan` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn run_spike_scan_codegen() -> Result<i32, String> {
    Ok(0)
}

// ---------- uma ----------
/// v0.126 —— `UmaDisputeStatusDto` 形状。
#[derive(Serialize, Deserialize, Type)]
struct UmaDisputeStatusDtoCodegen {
    pub market_id: String,
    pub status: String,
    pub detail: Option<String>,
    /// v0.126 —— 占位符(真实类型 Option<i64>)。
    pub raised_at: Option<i32>,
    pub raised_by: Option<String>,
}

/// v0.126 —— `uma_dispute_status` 的 codegen 存根。
#[tauri::command]
#[specta::specta]
async fn uma_dispute_status_codegen(
    _market_id: String,
) -> Result<UmaDisputeStatusDtoCodegen, String> {
    Ok(UmaDisputeStatusDtoCodegen {
        market_id: String::new(),
        status: String::new(),
        detail: None,
        raised_at: None,
        raised_by: None,
    })
}

fn main() {
    // 保留原始命令符号(防止链接器
    // 将它们优化掉 —— 它们会被生成的导出使用)。
    let _ = commands::pnl::dashboard_kpis;
    let _ = commands::bankroll::compute_allocation_preview;
    let _ = commands::bankroll::get_bankroll_config;
    let _ = commands::bankroll::set_bankroll_config;
    let _ = commands::bankroll::apply_allocation;
    // v0.84 —— Phase 3 只读命令(不需要 State,但
    // 符号必须保持可访问以用于 drift 检测)。
    let _ = commands::seed::is_seeded;
    let _ = commands::sidecar::sidecar_status;
    let _ = commands::secrets::secrets_status;
    let _ = commands::notify::notification_permission_state;
    let _ = commands::sidecar::get_telemetry_enabled;
    // v0.84b —— Phase 3 batch 2
    let _ = commands::sidecar::get_auto_promote_config;
    let _ = commands::storage::get_storage_info;
    let _ = commands::mirror_executor::get_mirror_paper_mode;
    let _ = commands::audit::get_audit_retention;
    let _ = commands::active_model::get_active_model;
    // v0.84c —— Phase 3 batch 3
    let _ = commands::signal::list_active_signals;
    let _ = commands::mirror_executor::list_mirrors;
    let _ = commands::wallet::list_wallets;
    let _ = commands::mirror_executor::mirror_queue_stats;
    // v0.88a —— Phase 4 batch 1(输入 DTO,简单形状)
    let _ = commands::wallet::add_wallet;
    let _ = commands::sidecar::set_telemetry_enabled;
    let _ = commands::mirror_executor::set_mirror_paper_mode;
    // v0.88b —— Phase 4 batch 2(Copy 路由,输入 DTO)
    let _ = commands::copy::add_copy_target;
    let _ = commands::mirror_executor::enqueue_mirror;
    // v0.88c —— Phase 4 batch 3(Trade 路由,输入 DTO)
    let _ = commands::bet::place_signed_order;
    let _ = commands::bet::place_jump_link;
    // v0.88d —— Phase 4 batch 4(Settings / ModelLab / LlmMgmt)
    let _ = commands::audit::set_audit_retention;
    let _ = commands::llm::upsert_llm_provider;
    let _ = commands::sidecar::set_auto_promote_config;
    // v0.88e —— Phase 4 batch 5(复杂嵌套 DTO)
    let _ = commands::llm::llm_analyze;
    let _ = commands::mirror_executor::run_mirror_executor_pass;
    // v0.101a —— Phase 4 batch 7：LLM stats heatmap + scatter + timeseries + decision
    let _ = commands::llm::llm_stats_heatmap;
    let _ = commands::llm::llm_stats_scatter;
    let _ = commands::llm::llm_stats_timeseries;
    let _ = commands::llm::llm_stats_decision;
    // v0.101b —— Phase 4 batch 7 part 2：LLM stats by_confidence + by_prompt
    //                                                  + cost_efficiency + export
    let _ = commands::llm_mgmt::llm_stats_by_confidence;
    let _ = commands::llm_mgmt::llm_stats_by_prompt;
    let _ = commands::llm_mgmt::llm_stats_cost_efficiency;
    let _ = commands::llm_mgmt::llm_stats_export;
    // v0.101c —— Phase 4 batch 7 part 3：LLM traffic + scheduler（5 个命令）
    let _ = commands::llm_mgmt::llm_traffic_summary;
    let _ = commands::scheduler::scheduler_status;
    let _ = commands::scheduler::scheduler_run_health_probe_now;
    let _ = commands::scheduler::scheduler_run_daily_brief_now;
    let _ = commands::scheduler::scheduler_self_test_now;
    // v0.102b —— Phase 4 batch 8：degradation + audit purge + daily_brief（6 个命令）
    let _ = commands::scheduler::degradation_check_now;
    let _ = commands::audit::purge_audit_log_now;
    let _ = commands::brief::daily_brief_get;
    let _ = commands::brief::daily_brief_refresh;
    let _ = commands::brief::daily_brief_dismiss;
    let _ = commands::brief::daily_brief_set_prefs;
    // v0.103b —— Phase 4 batch 9 part 1：LLM provider CRUD（4 个命令）
    let _ = commands::llm_mgmt::llm_provider_list;
    let _ = commands::llm_mgmt::llm_provider_upsert;
    let _ = commands::llm_mgmt::llm_provider_delete;
    // v0.103b2 —— Phase 4 batch 9 part 2：LLM key CRUD + connectivity + health + performance（7 个命令）
    let _ = commands::llm_mgmt::llm_key_list;
    let _ = commands::llm_mgmt::llm_key_upsert;
    let _ = commands::llm_mgmt::llm_key_set_secret;
    let _ = commands::llm_mgmt::llm_key_delete;
    let _ = commands::llm_mgmt::llm_test_connectivity;
    let _ = commands::llm_mgmt::llm_health_history;
    let _ = commands::llm::llm_performance;
    // v0.104b —— Phase 4 batch 10：sidecar + train + markets + wallet（8 个命令）
    let _ = commands::sidecar::train_job;
    let _ = commands::market::sync_markets;
    let _ = commands::signal::recompute_signals;
    let _ = commands::sidecar_health::sidecar_health_now;
    let _ = commands::sidecar_health::sidecar_health_snapshot;
    let _ = commands::sidecar::start_sidecar;
    let _ = commands::sidecar::stop_sidecar;
    let _ = commands::secrets::polyrocket_wallet_set_pk;
    // v0.105b —— Phase 4 batch 11：audit + promote + backtest（6 个命令）
    let _ = commands::audit::audit_count_for_actor;
    let _ = commands::sidecar::rollback_model;
    let _ = commands::sidecar::auto_promote_if_better;
    let _ = commands::sidecar::backtest_model;
    let _ = commands::sidecar::promote_model;
    let _ = commands::sidecar::promote_all_trials;
    // v0.126 —— Phase 4 batch 12：Football Feature Pack（13 个 IPC 命令）
    let _ = commands::arb::arb_scan;
    let _ = commands::arb::list_arb_opportunities;
    let _ = commands::calendar::market_calendar;
    let _ = commands::cross_platform_arb::cross_platform_arb_scan;
    let _ = commands::crowd_wisdom::crowd_opinion;
    let _ = commands::football_context::get_football_context;
    let _ = commands::mean_reversion::reversion_signal;
    let _ = commands::news::market_news;
    let _ = commands::news::list_all_news;
    let _ = commands::nl_query::nl_query;
    let _ = commands::poisson::poisson_score_matrix;
    let _ = commands::smart_money::smart_money_score;
    let _ = commands::spike::list_spike_alerts;
    let _ = commands::spike::run_spike_scan;
    let _ = commands::uma::uma_dispute_status;

    let builder: Builder<tauri::Wry> = Builder::new().commands(collect_commands![
        dashboard_kpis_codegen,
        compute_allocation_preview_codegen,
        get_bankroll_config_codegen,
        set_bankroll_config_codegen,
        apply_allocation_codegen,
        // v0.84 —— Phase 3 只读命令
        is_seeded_codegen,
        sidecar_status_codegen,
        secrets_status_codegen,
        notification_permission_state_codegen,
        get_telemetry_enabled_codegen,
        // v0.84b —— Phase 3 batch 2
        get_auto_promote_config_codegen,
        get_storage_info_codegen,
        get_mirror_paper_mode_codegen,
        get_audit_retention_codegen,
        get_active_model_codegen,
        // v0.84c —— Phase 3 batch 3
        list_active_signals_codegen,
        list_mirrors_codegen,
        list_wallets_codegen,
        mirror_queue_stats_codegen,
        // v0.88a —— Phase 4 batch 1(输入 DTO,简单形状)
        add_wallet_codegen,
        set_telemetry_enabled_codegen,
        set_mirror_paper_mode_codegen,
        // v0.88b —— Phase 4 batch 2(Copy 路由,输入 DTO)
        add_copy_target_codegen,
        enqueue_mirror_codegen,
        // v0.88c —— Phase 4 batch 3(Trade 路由,输入 DTO)
        place_signed_order_codegen,
        place_jump_link_codegen,
        // v0.88d —— Phase 4 batch 4(Settings / ModelLab / LlmMgmt)
        set_audit_retention_codegen,
        upsert_llm_provider_codegen,
        set_auto_promote_config_codegen,
        // v0.88e —— Phase 4 batch 5(复杂嵌套 DTO)
        llm_analyze_codegen,
        run_mirror_executor_pass_codegen,
        // v0.98 —— Phase 4 batch 6：另外 4 个只读 list 命令
        list_bets_codegen,
        list_audit_log_codegen,
        list_copy_targets_codegen,
        list_promote_history_codegen,
        // v0.101a —— Phase 4 batch 7：LLM stats（4 个只读命令）
        llm_stats_heatmap_codegen,
        llm_stats_scatter_codegen,
        llm_stats_timeseries_codegen,
        llm_stats_decision_codegen,
        // v0.101b —— Phase 4 batch 7 part 2：LLM stats（另外 4 个）
        llm_stats_by_confidence_codegen,
        llm_stats_by_prompt_codegen,
        llm_stats_cost_efficiency_codegen,
        llm_stats_export_codegen,
        // v0.101c —— Phase 4 batch 7 part 3：LLM traffic + scheduler（5 个命令）
        llm_traffic_summary_codegen,
        scheduler_status_codegen,
        scheduler_run_health_probe_now_codegen,
        scheduler_run_daily_brief_now_codegen,
        scheduler_self_test_now_codegen,
        // v0.102b —— Phase 4 batch 8：degradation + audit purge + daily_brief（6 个命令）
        degradation_check_now_codegen,
        purge_audit_log_now_codegen,
        daily_brief_get_codegen,
        daily_brief_refresh_codegen,
        daily_brief_dismiss_codegen,
        daily_brief_set_prefs_codegen,
        // v0.103b —— Phase 4 batch 9 part 1：LLM provider CRUD（4 个命令）
        llm_provider_list_codegen,
        llm_provider_upsert_codegen,
        llm_provider_delete_codegen,
        // v0.103b2 —— Phase 4 batch 9 part 2：LLM key CRUD + connectivity + health + performance（7 个命令）
        llm_key_list_codegen,
        llm_key_upsert_codegen,
        llm_key_set_secret_codegen,
        llm_key_delete_codegen,
        llm_test_connectivity_codegen,
        llm_health_history_codegen,
        llm_performance_codegen,
        // v0.104b —— Phase 4 batch 10：sidecar + train + markets + wallet（8 个命令）
        train_job_codegen,
        sync_markets_codegen,
        recompute_signals_codegen,
        sidecar_health_now_codegen,
        sidecar_health_snapshot_codegen,
        start_sidecar_codegen,
        stop_sidecar_codegen,
        polyrocket_wallet_set_pk_codegen,
        // v0.105b —— Phase 4 batch 11：audit + promote + backtest（6 个命令）
        audit_count_for_actor_codegen,
        rollback_model_codegen,
        auto_promote_if_better_codegen,
        backtest_model_codegen,
        promote_model_codegen,
        promote_all_trials_codegen,
        // v0.126 —— Phase 4 batch 12：Football Feature Pack（13 个 IPC 命令）
        arb_scan_codegen,
        list_arb_opportunities_codegen,
        market_calendar_codegen,
        cross_platform_arb_scan_codegen,
        crowd_opinion_codegen,
        get_football_context_codegen,
        reversion_signal_codegen,
        market_news_codegen,
        list_all_news_codegen,
        nl_query_codegen,
        poisson_score_matrix_codegen,
        smart_money_score_codegen,
        list_spike_alerts_codegen,
        run_spike_scan_codegen,
        uma_dispute_status_codegen,
    ]);

    // CARGO_MANIFEST_DIR 是 `src-tauri/`,因此父目录是
    // 项目根目录。我们需要 `<project>/src/types/generated/`。
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR");
    let out_dir = std::path::PathBuf::from(manifest_dir)
        .parent()
        .expect("parent of manifest dir")
        .join("src/types/generated");
    std::fs::create_dir_all(&out_dir).expect("create generated dir");
    let out_file = out_dir.join("index.ts");

    // 直接使用 specta_typescript。必须匹配 tauri-specta 的
    // 捆绑版本(0.0.12)—— 参见 Cargo.toml 注释。
    builder
        .export(specta_typescript::Typescript::default(), &out_file)
        .expect("export ts bindings");

    println!(
        "✅ Generated TS bindings to {}",
        out_file.display()
    );
}

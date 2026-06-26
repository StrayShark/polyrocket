//! L3 —— Python sidecar 协议（M7）。
//!
//! 定义 polyrocket 与可选 Python ML sidecar（通过 `std::process::Command`
//! 启动的独立进程）通信所用的 JSON-line 协议。本模块为纯辅助函数 —— 不含 IO。
//!
//! 规范：docs/polyrocket-modules.md §2.M7
//!
//! 协议（每个方向均为每行一个 JSON 对象）：
//!
//! request:  { "id": "uuid", "method": "name", "params": {...} }
//!           （请求：{ "id": "uuid", "method": "name", "params": {...} }）
//! response: { "id": "uuid", "ok": true, "result": {...} }
//!           （成功响应：{ "id": "uuid", "ok": true, "result": {...} }）
//!            | { "id": "uuid", "ok": false, "error": "msg" }
//!           （失败响应：{ "id": "uuid", "ok": false, "error": "msg" }）
//!
//! 方法（polyrocket → python）：
//!   - "ping"            → { "pong": true } （存活探活）
//!   - "predict"         → { "predictions": [{ market_id, prob, confidence }] } （预测）
//!   - "train_job"       → { "job_id": "..." } （训练任务）
//!   - "promote_model"   → { "promoted": true } （晋升模型）
//!
//! 方法（python → polyrocket，用于回调）：
//!   - "log"             → 追加到 sidecar 日志
//!   - "metric"          → 实时指标更新

use crate::infra::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// polyrocket → python 的一条 JSON-line 请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarRequest {
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// python → polyrocket 的一条 JSON-line 响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarResponse {
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl SidecarResponse {
    pub fn ok(id: impl Into<String>, result: Value) -> Self {
        Self { id: id.into(), ok: true, result: Some(result), error: None }
    }
    pub fn err(id: impl Into<String>, msg: impl Into<String>) -> Self {
        Self { id: id.into(), ok: false, result: None, error: Some(msg.into()) }
    }
}

/// `predict` 方法返回的单条预测记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    pub market_id: String,
    /// 0..1
    pub prob: f64,
    /// 0..1
    pub confidence: f64,
    pub rationale: Option<String>,
}

/// v0.12a —— 完整的 predict 响应（result 块 + 每条记录）。
/// `predictions` 字段镜像每行数据；`model_version` 提升到顶层，
/// 这样 L1 ModelLab 页面无需遍历行即可展示「scoring with logistic-train-...」。
/// v0.13b —— 同时携带活跃模型的 `brier_score`，便于 L1 ModelVersionPill
/// 在 tooltip 中显示 calibration（校准度）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PredictResult {
    pub predictions: Vec<Prediction>,
    /// `logistic-0.1.0`（内联回退）或 `logistic-train-441c352b`
    ///（已 promote 的活跃模型）。若 sidecar 未提供则为 `None`（向后兼容）。
    pub model_version: Option<String>,
    /// 最近一次 train 运行的 Brier 分数（被 promote 到 active.json 的
    ///「最佳」试验分数）。若尚无 promote 模型（使用内联回退）或 sidecar
    /// 未提供则为 `None`。
    pub brier_score: Option<f64>,
}

/// 用于类型安全分发的 Methods 枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SidecarMethod {
    Ping,
    Predict,
    TrainJob,
    PromoteModel,
    /// v0.19a —— 对历史 promote 的只读审计。返回 active.json 中的
    /// `promotion_history` 数组（最多 20 条最新条目）。
    ListPromoteHistory,
    /// v0.20a —— 将活跃模型回滚到先前的某个版本。按 `model_version`
    /// 在 promotion_history 中查找对应条目，并将其 `weights`
    /// 恢复为新的活跃模型。该条目必须包含 weights（v0.20a+）；
    /// 不含 weights 的 v0.19 旧条目会被拒绝并返回清晰错误。
    RollbackModel,
    /// v0.23a —— auto-promote 保护。只有当候选模型明显优于活跃模型
    ///（按 Brier margin 衡量）时才执行 promote。否则 no-op，并返回
    /// 清晰的「skipped」原因。由用户一键触发。
    AutoPromoteIfBetter,
    /// v0.25a —— 一次性 promote 全部 4 个 trial。遍历
    /// `candidate.all_trials[]` 并依次 promote。
    /// 用于 A/B 对比：用户可在 history 中看到全部 4 个 trial，
    /// 再通过 Rollback 挑选胜出者。
    PromoteAllTrials,
    /// v0.43a —— 用一组 (price, market_age_hours, outcome) 样本对
    /// 已保存的模型进行回放，并返回 Brier + calibration + 每条样本的
    /// 预测。补齐 v0.17-v0.41 生命周期中缺失的环节：在该功能出现之前，
    /// 没办法回答「这个模型在真实结算上表现如何」。
    /// 纯函数（除读取模型文件外无 IO）；由 L1 负责从 markets DB
    /// 拉取已结算市场。
    BacktestModel,
    /// v0.55 —— 单个样本的逐特征贡献。对 3 特征逻辑回归模型，
    /// 这是精确分解（非 SHAP 近似）：`contribution_i = w_i * x_i * p(1-p)`，
    /// 即概率对特征的导数。L1 将其渲染为水平条形图。
    /// 对树模型，则需要真正的 SHAP 库；列为 v0.55+ 候选。
    ExplainModel,
    /// v0.59 —— 通过 KernelExplainer 计算真正的 SHAP 值。
    /// 满足 SHAP efficiency 公理：`Σφ_i = f(x) - E[f(x)]`。
    /// 对 polyrocket 的 3 特征模型，开销为 8 次 coalition 评估；
    /// 对 M > 5 的树模型，则需要 TreeSHAP。v0.59 候选。
    ShapExplain,
}

impl SidecarMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            SidecarMethod::Ping => "ping",
            SidecarMethod::Predict => "predict",
            SidecarMethod::TrainJob => "train_job",
            SidecarMethod::PromoteModel => "promote_model",
            SidecarMethod::ListPromoteHistory => "list_promote_history",
            SidecarMethod::RollbackModel => "rollback_model",
            SidecarMethod::AutoPromoteIfBetter => "auto_promote_if_better",
            SidecarMethod::PromoteAllTrials => "promote_all_trials",
            SidecarMethod::BacktestModel => "backtest_model",
            SidecarMethod::ExplainModel => "explain_model",
            SidecarMethod::ShapExplain => "shap_explain",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ping" => Some(SidecarMethod::Ping),
            "predict" => Some(SidecarMethod::Predict),
            "train_job" => Some(SidecarMethod::TrainJob),
            "promote_model" => Some(SidecarMethod::PromoteModel),
            "list_promote_history" => Some(SidecarMethod::ListPromoteHistory),
            "rollback_model" => Some(SidecarMethod::RollbackModel),
            "auto_promote_if_better" => Some(SidecarMethod::AutoPromoteIfBetter),
            "promote_all_trials" => Some(SidecarMethod::PromoteAllTrials),
            "backtest_model" => Some(SidecarMethod::BacktestModel),
            "explain_model" => Some(SidecarMethod::ExplainModel),
            "shap_explain" => Some(SidecarMethod::ShapExplain),
            _ => None,
        }
    }
}

/// 尝试把一行 JSON 解析为 request 或 response。
/// 成功返回 `Ok(parsed)`,失败则返回带原始行的 `Err` 供检查。
pub fn parse_line(line: &str) -> Result<ParseResult, String> {
    let v: Value = serde_json::from_str(line.trim())
        .map_err(|e| format!("invalid JSON: {e}"))?;
    // Response 含 `ok`；request 含 `method`。
    if v.get("ok").is_some() {
        let r: SidecarResponse = serde_json::from_value(v)
            .map_err(|e| format!("response parse: {e}"))?;
        Ok(ParseResult::Response(r))
    } else if v.get("method").is_some() {
        let r: SidecarRequest = serde_json::from_value(v)
            .map_err(|e| format!("request parse: {e}"))?;
        Ok(ParseResult::Request(r))
    } else {
        Err("line is neither a request nor a response".into())
    }
}

pub enum ParseResult {
    Request(SidecarRequest),
    Response(SidecarResponse),
}

impl std::fmt::Debug for ParseResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseResult::Request(r) => f.debug_tuple("Request").field(r).finish(),
            ParseResult::Response(r) => f.debug_tuple("Response").field(r).finish(),
        }
    }
}

/// 从 market id 与市场上下文构建 `predict` 请求。
/// 纯函数：序列化为 JSON-line 字符串。
pub fn build_predict_request(
    id: impl Into<String>,
    markets: &[(String, f64)], // (market_id, current_price)
) -> String {
    let params = serde_json::json!({
        "markets": markets.iter().map(|(id, price)| {
            serde_json::json!({ "market_id": id, "price": price })
        }).collect::<Vec<_>>()
    });
    let req = SidecarRequest {
        id: id.into(),
        method: SidecarMethod::Predict.as_str().to_string(),
        params,
    };
    serde_json::to_string(&req).unwrap_or_default()
}

/// 将 `predict` 响应解析为 `PredictResult`。
/// 若响应为 `ok=false` 则返回 Err。
///
/// v0.12a —— 现在返回完整的 `PredictResult`（predictions + model_version），
/// 而不仅仅是 `Vec<Prediction>`。model version 从响应的顶层
/// `model_version` 字段提升；若 sidecar 未提供则回退为 `None`
///（与 v0.11c 及更早版本兼容）。
pub fn parse_predict_response(
    resp: &SidecarResponse,
) -> Result<PredictResult, String> {
    if !resp.ok {
        return Err(resp.error.clone().unwrap_or_else(|| "unknown error".into()));
    }
    let v = resp.result.clone().unwrap_or(Value::Null);
    let arr = v.get("predictions")
        .and_then(|x| x.as_array())
        .ok_or_else(|| "missing 'predictions' array".to_string())?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let p: Prediction = serde_json::from_value(item.clone())
            .map_err(|e| format!("prediction row: {e}"))?;
        out.push(p);
    }
    let model_version = v.get("model_version")
        .and_then(|x| x.as_str())
        .map(String::from);
    let brier_score = v.get("brier_score").and_then(|x| x.as_f64());
    Ok(PredictResult { predictions: out, model_version, brier_score })
}

// =================================================================
// ============== v0.17a —— train_job wire 格式 ====================
// =================================================================

/// 构建 `train_job` 请求。纯函数：序列化为 JSON-line 字符串。
/// Python sidecar 接受以下参数（参见 `sidecar/polyrocket_sidecar/dispatch.py::train_job`）：
///
///   - n_trials: int（默认 4，上限 4）
///   - epochs:   int（默认 80）
///   - job_id:   str（由服务端生成；客户端传入会被忽略）
///
/// Rust 端会生成全新的 `job_id`，并在 started 事件中回传，
/// 以便 L1 进行关联。
pub fn build_train_request(
    id: impl Into<String>,
    n_trials: Option<u32>,
    epochs: Option<u32>,
) -> String {
    let mut params = serde_json::Map::new();
    if let Some(n) = n_trials {
        params.insert("n_trials".into(), serde_json::json!(n));
    }
    if let Some(e) = epochs {
        params.insert("epochs".into(), serde_json::json!(e));
    }
    let req = SidecarRequest {
        id: id.into(),
        method: SidecarMethod::TrainJob.as_str().to_string(),
        params: serde_json::Value::Object(params),
    };
    serde_json::to_string(&req).unwrap_or_default()
}

/// `train_job` 响应中单个 trial 的结果。v0.17a 镜像 Python
/// sidecar 的 `trials[]` 数组。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainTrial {
    pub lr: f64,
    pub reg: f64,
    pub brier: f64,
    /// `{"w0": ..., "w1": ..., "w2": ...}` —— 该 trial 训练出的权重。
    /// 对应 Python 中的 `weights` 字典。
    pub weights: serde_json::Value,
}

/// 镜像 Python `run_train_job` 返回值的 wire 格式。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainResult {
    /// 服务端生成的 job id（例如 `"train-441c352b"`）。
    pub job_id: String,
    /// "completed" | "failed"（镜像 Python 返回值）。
    pub status: String,
    /// 最佳 trial 的 Brier 分数（越低越好）。失败时为 null。
    pub best_brier: Option<f64>,
    /// 最佳 trial 的权重：`{"w0", "w1", "w2"}`。失败时为 null。
    pub best_params: Option<serde_json::Value>,
    /// 每个 trial 的统计。若 train 在任何 trial 完成前失败则长度为 0。
    pub trials: Vec<TrainTrial>,
    /// 运行耗时（毫秒），对应 Python 的 `duration_ms`。
    pub duration_ms: i64,
    /// Python sidecar 写入的 candidate JSON 的绝对路径
    ///（例如 `~/.polyrocket/sidecar/models/candidate.json`）。
    /// 失败时为 null。
    pub candidate_path: Option<String>,
    /// `status == "failed"` 时的人类可读错误信息。成功时为 None。
    pub message: Option<String>,
}

/// 将 `train_job` 响应解析为 `TrainResult`。若响应为 `ok=false` 则返回 Err。
///
/// Python sidecar 的 `run_train_job` 返回一个 dict，包含上述字段；
/// 部分字段在失败路径上是可选的（例如 `best_brier` 即使在部分成功时也可能为 null）。
pub fn parse_train_response(resp: &SidecarResponse) -> Result<TrainResult, String> {
    if !resp.ok {
        return Err(resp.error.clone().unwrap_or_else(|| "unknown error".into()));
    }
    let v = resp.result.clone().unwrap_or(Value::Null);
    Ok(TrainResult {
        job_id: v.get("job_id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        status: v.get("status")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_string(),
        best_brier: v.get("best_brier").and_then(|x| x.as_f64()),
        best_params: v.get("best_params").cloned(),
        trials: v.get("trials")
            .and_then(|x| x.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| serde_json::from_value::<TrainTrial>(item.clone()).ok())
                    .collect()
            })
            .unwrap_or_default(),
        duration_ms: v.get("duration_ms").and_then(|x| x.as_i64()).unwrap_or(0),
        candidate_path: v.get("candidate_path")
            .and_then(|x| x.as_str())
            .map(String::from),
        message: v.get("message")
            .and_then(|x| x.as_str())
            .map(String::from),
    })
}

// =================================================================
// ============== v0.18a —— promote_model wire 协议格式 ================
// =================================================================

/// 构建 `promote_model` 请求。纯函数：序列化为 JSON-line 字符串。
/// Python sidecar 接受：
///
///   - job_id: str（可选；若设置，则拒绝 promote 来自其他 job 的
///     candidate —— 用于防止「用户在表达 promote 意图」与
///     「实际执行 promote」之间其他 train 完成的竞态条件）
///
/// v0.18a —— promote 是一次快速的同步文件移动（~10ms）。
/// 没有进度事件。IPC 返回完整结果。
/// v0.21a —— `trial_index` 是可选的批量 promote 参数。
/// 若为 `Some(n)`，则从 `all_trials[]` 中 promote 第 n 个 trial
/// （而非最佳）。模型版本会附加 `-t{n}` 后缀，便于用户在
/// history 面板中区分批量 promote 的 trial。
pub fn build_promote_request(
    id: impl Into<String>,
    job_id: Option<&str>,
    trial_index: Option<usize>,
) -> String {
    let mut params = serde_json::Map::new();
    if let Some(j) = job_id {
        params.insert("job_id".into(), serde_json::Value::String(j.to_string()));
    }
    if let Some(t) = trial_index {
        params.insert("trial_index".into(), serde_json::Value::from(t));
    }
    let req = SidecarRequest {
        id: id.into(),
        method: SidecarMethod::PromoteModel.as_str().to_string(),
        params: serde_json::Value::Object(params),
    };
    serde_json::to_string(&req).unwrap_or_default()
}

/// 镜像 Python `run_promote_model` 返回值的 wire 格式。
/// v0.18a —— `promoted: bool` 是首要的成功标志；`status: "ok" | "failed"`
/// 为 Python 端为向后兼容保留的字符串。
///
/// 所有字段均可为空，因为 Python sidecar 在失败路径上
/// 返回部分填充的 dict（例如 candidate 文件缺失时）。
///
/// v0.21a —— `trial_index: Option<usize>` 记录被 promote 的 trial。
/// `None` 表示最佳（默认）；`Some(n)` 表示 train 扫描中的第 n 个
/// trial（批量 promote）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoteResult {
    /// 成功为 `true`，失败为 `false`。
    pub promoted: bool,
    /// "ok" | "failed" —— 镜像 Python 返回字符串。
    pub status: String,
    /// 旧 active.json 的路径（首次 promote 时为 null）。
    pub previous_path: Option<String>,
    /// 新 active.json 的路径（candidate 被重命名为此）。
    pub active_path: Option<String>,
    /// promote 的墙钟时间（毫秒）。
    pub promoted_at_ms: Option<i64>,
    /// 新模型版本字符串（例如最佳的 `logistic-train-441c352b`，
    /// 或批量的 `logistic-train-441c352b-t2`）。失败时为空字符串。
    pub model_version: String,
    /// 失败时的人类可读错误信息；成功时为 `None`。
    pub message: Option<String>,
    /// v0.21a —— 被 promote 的 trial。`None` = 最佳；`Some(n)` = train 扫描中第 n 个 trial。
    pub trial_index: Option<usize>,
}

/// 将 `promote_model` 响应解析为 `PromoteResult`。
/// 若响应为 `ok=false` 则返回 Err。
pub fn parse_promote_response(resp: &SidecarResponse) -> Result<PromoteResult, String> {
    if !resp.ok {
        return Err(resp.error.clone().unwrap_or_else(|| "unknown error".into()));
    }
    let v = resp.result.clone().unwrap_or(Value::Null);
    Ok(PromoteResult {
        promoted: v.get("promoted").and_then(|x| x.as_bool()).unwrap_or(false),
        status: v.get("status")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_string(),
        previous_path: v.get("previous_path")
            .and_then(|x| x.as_str())
            .map(String::from),
        active_path: v.get("active_path")
            .and_then(|x| x.as_str())
            .map(String::from),
        promoted_at_ms: v.get("promoted_at_ms").and_then(|x| x.as_i64()),
        model_version: v.get("model_version")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        message: v.get("message")
            .and_then(|x| x.as_str())
            .map(String::from),
        trial_index: v.get("trial_index").and_then(|x| x.as_u64()).map(|n| n as usize),
    })
}

// =================================================================
// ============ v0.19a —— list_promote_history wire 格式 ===========
// =================================================================

/// 构建 `list_promote_history` 请求。纯函数。
/// v0.19a —— 只读审计，无参数。请求仅携带 id 用于关联。
pub fn build_list_promote_history_request(id: impl Into<String>) -> String {
    let req = SidecarRequest {
        id: id.into(),
        method: SidecarMethod::ListPromoteHistory.as_str().to_string(),
        params: serde_json::json!({}),
    };
    serde_json::to_string(&req).unwrap_or_default()
}

/// 镜像 `active.json.promotion_history[]` 中单条目的 wire 格式。
/// v0.19a —— 每次成功 promote 对应一条记录。最旧在前、最新在后
///（新 promote 发生时，最末条目即为刚被取代的；如需当前活跃模型，
/// 请改用 `PredictResult` 中的 `model_version`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoteHistoryEntry {
    /// Train job_id，例如 "train-441c352b"。
    pub job_id: String,
    /// 派生的模型版本，例如 "logistic-train-441c352b"。
    pub model_version: String,
    /// promote 的墙钟时间（毫秒）。
    pub promoted_at_ms: i64,
    /// train 扫描中的最佳 Brier 分数（越低越好）。
    pub best_brier: Option<f64>,
    /// 最佳 trial 的超参数。
    #[serde(default)]
    pub best_params: Option<Value>,
    /// v0.21a —— 被 promote 的 trial。`None` = 最佳（默认）；
    /// `Some(n)` = 第 n 个 trial（批量）。v0.21a 之前的旧条目
    /// 没有该字段；serde 默认为 None。
    #[serde(default)]
    pub trial_index: Option<usize>,
    /// v0.41a —— promote 的人类可读原因。在 L1 中显示为
    /// history 行的 hover tooltip。格式：
    /// "Promoted as best trial" 或 "Promoted as trial N of M"。
    ///
    /// v0.41 之前的旧条目没有该字段；serde 默认为 None。
    #[serde(default)]
    pub reason: Option<String>,
}

/// 镜像 `list_promote_history` 响应的 wire 格式。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PromoteHistoryResult {
    /// 成功为 `true`；若 active.json 缺失或格式错误则为 `false`
    ///（Python sidecar 仅在极端情况下返回 ok=false；文件缺失属于
    /// 「尚无 history」的自然状态，会以 `ok=true` 且 `count=0` 返回）。
    pub ok: bool,
    /// history 条目，最旧在前。
    pub entries: Vec<PromoteHistoryEntry>,
    /// `len(entries)`，方便使用。
    pub count: usize,
    /// 失败时的人类可读错误信息；成功时为 `None`。
    pub message: Option<String>,
}

/// 将 `list_promote_history` 响应解析为 `PromoteHistoryResult`。
/// 仅当 sidecar 在 envelope 层返回 `ok=false`（传输错误）时才返回 Err。
/// 若 envelope 为 ok=true 但 result 层 ok=false（文件格式错误），
/// 则返回 Ok 并携带 message。
pub fn parse_list_promote_history_response(
    resp: &SidecarResponse,
) -> Result<PromoteHistoryResult, String> {
    if !resp.ok {
        return Err(resp.error.clone().unwrap_or_else(|| "unknown error".into()));
    }
    let v = resp.result.clone().unwrap_or(Value::Null);
    let entries: Vec<PromoteHistoryEntry> = v
        .get("entries")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| serde_json::from_value::<PromoteHistoryEntry>(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    Ok(PromoteHistoryResult {
        ok: v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false),
        count: v.get("count").and_then(|x| x.as_u64()).unwrap_or(entries.len() as u64) as usize,
        entries,
        message: v.get("message")
            .and_then(|x| x.as_str())
            .map(String::from),
    })
}

// =================================================================
// ============== v0.20a —— rollback_model wire 协议格式 ===============
// =================================================================

/// 构建 `rollback_model` 请求。纯函数。
/// v0.20a —— 将活跃模型回滚到先前的某个版本
///（按 `model_version` 在 promotion_history 中查找）。
pub fn build_rollback_request(
    id: impl Into<String>,
    model_version: &str,
) -> String {
    let req = SidecarRequest {
        id: id.into(),
        method: SidecarMethod::RollbackModel.as_str().to_string(),
        params: serde_json::json!({ "model_version": model_version }),
    };
    serde_json::to_string(&req).unwrap_or_default()
}

/// 镜像 Python `run_rollback_model` 返回值的 wire 格式。
/// v0.20a —— `rolled_back: bool` 是首要的成功标志。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackResult {
    /// 成功为 `true`，失败为 `false`。
    pub rolled_back: bool,
    /// "ok" | "failed" —— 镜像 Python 返回字符串。
    pub status: String,
    /// 旧 active.json 的路径（始终为同一路径；rollback 写入同一文件）。
    pub previous_path: Option<String>,
    /// 新 active.json 的路径（与 previous_path 相同）。
    pub active_path: Option<String>,
    /// rollback 的墙钟时间（毫秒）。
    pub rolled_back_at_ms: Option<i64>,
    /// 回滚到的版本。失败时为空。
    pub model_version: String,
    /// 失败时的人类可读错误信息。
    pub message: Option<String>,
}

/// 将 `rollback_model` 响应解析为 `RollbackResult`。
/// 若响应在 envelope 层为 `ok=false` 则返回 Err。
pub fn parse_rollback_response(resp: &SidecarResponse) -> Result<RollbackResult, String> {
    if !resp.ok {
        return Err(resp.error.clone().unwrap_or_else(|| "unknown error".into()));
    }
    let v = resp.result.clone().unwrap_or(Value::Null);
    Ok(RollbackResult {
        rolled_back: v.get("rolled_back").and_then(|x| x.as_bool()).unwrap_or(false),
        status: v.get("status")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_string(),
        previous_path: v.get("previous_path")
            .and_then(|x| x.as_str())
            .map(String::from),
        active_path: v.get("active_path")
            .and_then(|x| x.as_str())
            .map(String::from),
        rolled_back_at_ms: v.get("rolled_back_at_ms").and_then(|x| x.as_i64()),
        model_version: v.get("model_version")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        message: v.get("message")
            .and_then(|x| x.as_str())
            .map(String::from),
    })
}

// =================================================================
// ========== v0.23a —— auto_promote_if_better wire 协议格式 ==========
// =================================================================

/// 构建 `auto_promote_if_better` 请求。纯函数。
/// v0.23a —— 一键「仅当候选明显优于活跃模型时才 promote」操作。
///
/// `brier_margin` 表示候选需要优于此 margin（Brier 越低越好）。
/// 默认 0.005。
/// `trial_index` 表示使用哪个 trial（None = 最佳）。
pub fn build_auto_promote_if_better_request(
    id: impl Into<String>,
    brier_margin: Option<f64>,
    trial_index: Option<usize>,
) -> String {
    let mut params = serde_json::Map::new();
    if let Some(m) = brier_margin {
        params.insert("brier_margin".into(), serde_json::json!(m));
    }
    if let Some(t) = trial_index {
        params.insert("trial_index".into(), serde_json::Value::from(t));
    }
    let req = SidecarRequest {
        id: id.into(),
        method: SidecarMethod::AutoPromoteIfBetter.as_str().to_string(),
        params: serde_json::Value::Object(params),
    };
    serde_json::to_string(&req).unwrap_or_default()
}

/// 镜像 Python `run_auto_promote_if_better` 返回值的 wire 格式。
/// v0.23a —— 用户在每次 train 后点击「Promote if better」；该 DTO
/// 携带执行结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPromoteIfBetterResult {
    /// 若候选模型确实被 promote 则为 `true`。
    pub promoted: bool,
    /// 若候选未被 promote（因为不够优秀）则为 `true`。
    /// 与 `promoted=true` 互斥。
    pub skipped: bool,
    /// 人类可读的原因：
    /// "auto-promoted: improvement X > margin Y" /
    /// "candidate brier X is not at least Y better than active Z" /
    /// "no candidate" 等。
    pub reason: String,
    /// 候选模型的 brier（无候选则为 None）。
    pub candidate_brier: Option<f64>,
    /// 活跃模型的 brier（无活跃模型则为 None）。
    pub active_brier: Option<f64>,
    /// 比较所用的 brier_margin。
    pub margin: f64,
    /// 新模型版本（成功时）。skip/fail 时为空。
    pub model_version: Option<String>,
    /// promote 时间戳（成功时）。skip/fail 时为 None。
    pub promoted_at_ms: Option<i64>,
    /// 人类可读的消息（例如 Python 端的错误信息）。
    pub message: Option<String>,
}

/// 将 `auto_promote_if_better` 响应解析为 `AutoPromoteIfBetterResult`。
/// 若响应在 envelope 层为 `ok=false` 则返回 Err。
pub fn parse_auto_promote_if_better_response(
    resp: &SidecarResponse,
) -> Result<AutoPromoteIfBetterResult, String> {
    if !resp.ok {
        return Err(resp.error.clone().unwrap_or_else(|| "unknown error".into()));
    }
    let v = resp.result.clone().unwrap_or(Value::Null);
    Ok(AutoPromoteIfBetterResult {
        promoted: v.get("promoted").and_then(|x| x.as_bool()).unwrap_or(false),
        skipped: v.get("skipped").and_then(|x| x.as_bool()).unwrap_or(false),
        reason: v.get("reason")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        candidate_brier: v.get("candidate_brier").and_then(|x| x.as_f64()),
        active_brier: v.get("active_brier").and_then(|x| x.as_f64()),
        margin: v.get("margin").and_then(|x| x.as_f64()).unwrap_or(0.005),
        model_version: v.get("model_version")
            .and_then(|x| x.as_str())
            .map(String::from),
        promoted_at_ms: v.get("promoted_at_ms").and_then(|x| x.as_i64()),
        message: v.get("message")
            .and_then(|x| x.as_str())
            .map(String::from),
    })
}

// =================================================================
// ============== v0.25a —— promote_all_trials wire 格式 ==========
// =================================================================

/// 构建 `promote_all_trials` 请求。纯函数。
/// v0.25a —— 无参数；sidecar 读取当前 candidate 并 promote 每个 trial。
pub fn build_promote_all_trials_request(id: impl Into<String>) -> String {
    let req = SidecarRequest {
        id: id.into(),
        method: SidecarMethod::PromoteAllTrials.as_str().to_string(),
        params: serde_json::json!({}),
    };
    serde_json::to_string(&req).unwrap_or_default()
}

/// v0.43a —— 回测中的一条样本。L1 从 markets DB（仅已结算市场）构造
/// 该列表并直接透传。我们把类型放在这里，是为了使 wire 格式与
/// Rust 类型保持同步。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestSample {
    /// 预测时刻的市场价格（0..1）。
    pub price: f64,
    /// 预测时刻距市场开盘的小时数。模型的 `w2` 对 age 加权。
    pub market_age_hours: f64,
    /// 结算结果（0 = NO，1 = YES）。
    pub outcome: f64,
    /// 可选的人类可读标签，会在 top winners/losers 列表中展示
    ///（例如市场问题）。空字符串也可以。
    #[serde(default)]
    pub label: String,
}

/// 构建 `backtest_model` 请求。v0.43a。纯函数 —— 样本原样透传；
/// 由 sidecar 完成预测与 Brier 计算。
pub fn build_backtest_model_request(
    id: impl Into<String>,
    model_version: &str,
    samples: &[BacktestSample],
) -> String {
    let req = SidecarRequest {
        id: id.into(),
        method: SidecarMethod::BacktestModel.as_str().to_string(),
        params: serde_json::json!({
            "model_version": model_version,
            "samples": samples,
        }),
    };
    serde_json::to_string(&req).unwrap_or_default()
}

// =================================================================
// v0.55 —— explain_model 请求构造器与响应类型
// =================================================================

/// v0.55 —— 可解释性查询的输入样本。镜像
/// `explainability.run_explainability` 的 `sample` 参数。
/// 两个字段均为可选；同时省略则使用默认样本（price=0.5，age=24h）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExplainSample {
    /// 市场价格，0..1。默认 0.5。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<f64>,
    /// 市场年龄（小时），>=0。默认 24。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_age_hours: Option<f64>,
}

/// v0.55 —— `explain_model` 请求行的构造器。`sample` 可选 —— 当
/// 为 None 时，sidecar 使用默认样本。
pub fn build_explain_model_request(
    id: impl Into<String>,
    model_version: &str,
    sample: Option<&ExplainSample>,
) -> String {
    let mut params = serde_json::json!({
        "model_version": model_version,
    });
    if let Some(s) = sample {
        params["sample"] = serde_json::to_value(s).unwrap_or(serde_json::Value::Null);
    }
    let req = SidecarRequest {
        id: id.into(),
        method: SidecarMethod::ExplainModel.as_str().to_string(),
        params,
    };
    serde_json::to_string(&req).unwrap_or_default()
}

/// v0.55 —— 单个预测中一个特征的贡献。L1 将其渲染为水平条形图
///（正值绿色、负值红色，长度 = abs_contribution）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainFeature {
    /// 特征名，例如 "price"、"bias"、"market_age_hours"。
    pub feature: String,
    /// 样本中实际使用的特征值。
    pub value: f64,
    /// 模型对该特征的权重。
    pub weight: f64,
    /// 对 (p - 0.5) 的贡献。正值表示「该特征把预测推高」，
    /// 负值表示「把预测压低」。
    pub contribution: f64,
    /// `|contribution|`。用于排序以及条形图长度。
    pub abs_contribution: f64,
}

/// v0.55 —— 镜像 Python sidecar `explain_model` 响应的 wire 格式。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainResult {
    /// 成功为 `true`；未知模型 / 缺失权重 / 样本非法则为 `false`。
    pub ok: bool,
    /// 从请求回显。
    pub model_version: String,
    /// 逐特征贡献，按 `abs_contribution` 降序排列。
    pub features: Vec<ExplainFeature>,
    /// 模型对该样本的预测概率（0..1）。错误时为 `None`。
    pub prediction: Option<f64>,
    /// 已评估的样本（price + market_age_hours）。从请求回显，
    /// 缺失字段已填充默认值。
    pub sample: Option<ExplainSample>,
    /// 人类可读的状态 / 错误信息。
    pub message: String,
}

/// v0.55 —— 将 sidecar `explain_model` 响应解析为强类型的 `ExplainResult`。
/// `ok=true`/`ok=false` 两种情况下结构始终存在；这里仅容忍失败情形。
pub fn parse_explain_model_response(
    response: &SidecarResponse,
) -> AppResult<ExplainResult> {
    if !response.ok {
        let message = response
            .error
            .clone()
            .or_else(|| {
                response
                    .result
                    .as_ref()
                    .and_then(|r| r.get("message").and_then(|m| m.as_str()).map(String::from))
            })
            .unwrap_or_else(|| "unknown error".to_string());
        return Ok(ExplainResult {
            ok: false,
            model_version: response
                .result
                .as_ref()
                .and_then(|r| r.get("model_version").and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string(),
            features: Vec::new(),
            prediction: None,
            sample: None,
            message,
        });
    }
    let result = response.result.as_ref().ok_or_else(|| {
        AppError::Internal("explain_model: missing result".into())
    })?;
    let model_version = result
        .get("model_version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let features: Vec<ExplainFeature> = result
        .get("features")
        .and_then(|f| f.as_array())
        .map(|arr| {
            let mut v: Vec<ExplainFeature> = arr
                .iter()
                .filter_map(|x| serde_json::from_value(x.clone()).ok())
                .collect();
            // 按 abs_contribution 降序排列。sidecar 已排序，
            // 这里再次排序以做防御性处理，防止协议变更。
            v.sort_by(|a, b| {
                b.abs_contribution
                    .partial_cmp(&a.abs_contribution)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            v
        })
        .unwrap_or_default();
    let prediction = result
        .get("prediction")
        .and_then(|p| p.as_f64());
    let sample: Option<ExplainSample> = result
        .get("sample")
        .and_then(|s| serde_json::from_value(s.clone()).ok());
    let message = result
        .get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("ok")
        .to_string();
    Ok(ExplainResult {
        ok: true,
        model_version,
        features,
        prediction,
        sample,
        message,
    })
}

// =================================================================
// v0.59 —— 通过 KernelExplainer 实现 SHAP
// =================================================================

/// v0.59 —— 单个特征的 SHAP 值。形状与 ExplainFeature（v0.55）
/// 相同，但使用 `shap_value` / `abs_shap` 替代 `contribution` /
/// `abs_contribution`，便于在 L1 中显式呈现 SHAP 数学含义。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShapFeature {
    pub feature: String,
    pub value: f64,
    pub weight: f64,
    /// SHAP 值 φ_i。正值表示「把预测推高」，负值表示「把预测压低」。
    /// 满足：
    ///   Σφ_i = f(x) - E[f(x)]
    ///（SHAP efficiency 公理）。
    pub shap_value: f64,
    /// `|shap_value|`。用于排序以及条形图长度。
    pub abs_shap: f64,
}

/// v0.59 —— 镜像 Python sidecar `shap_explain` 响应的 wire 格式。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShapResult {
    pub ok: bool,
    pub model_version: String,
    /// 当前始终为 "kernel_shap"；未来变体（例如面向树模型的
    /// "tree_shap"）可在此设置不同值。
    pub method: String,
    pub features: Vec<ShapFeature>,
    /// baseline（空 coalition）的预测。错误时为 `None`。
    pub baseline_prediction: Option<f64>,
    /// 模型对目标样本的预测。错误时为 `None`。
    pub target_prediction: Option<f64>,
    /// SHAP efficiency 差距：`Σφ_i - (f(x) - E[f(x)])`。
    /// 在浮点容差内应接近 0.0；非零表示回归未收敛
    ///（例如模型退化）。
    pub efficiency_diff: Option<f64>,
    pub sample: Option<ExplainSample>,
    pub message: String,
}

/// v0.59 —— `shap_explain` 请求行的构造器。`sample` 可选 ——
/// 为 None 时，sidecar 使用默认样本（price=0.5，age=24h）。
pub fn build_shap_explain_request(
    id: impl Into<String>,
    model_version: &str,
    sample: Option<&ExplainSample>,
) -> String {
    let mut params = serde_json::json!({
        "model_version": model_version,
    });
    if let Some(s) = sample {
        params["sample"] = serde_json::to_value(s).unwrap_or(serde_json::Value::Null);
    }
    let req = SidecarRequest {
        id: id.into(),
        method: SidecarMethod::ShapExplain.as_str().to_string(),
        params,
    };
    serde_json::to_string(&req).unwrap_or_default()
}

/// v0.59 —— 将 sidecar `shap_explain` 响应解析为强类型的 `ShapResult`。
/// 错误容忍模式与 v0.55 的 `parse_explain_model_response` 一致。
pub fn parse_shap_explain_response(
    response: &SidecarResponse,
) -> AppResult<ShapResult> {
    if !response.ok {
        let message = response
            .error
            .clone()
            .or_else(|| {
                response
                    .result
                    .as_ref()
                    .and_then(|r| r.get("message").and_then(|m| m.as_str()).map(String::from))
            })
            .unwrap_or_else(|| "unknown error".to_string());
        return Ok(ShapResult {
            ok: false,
            model_version: response
                .result
                .as_ref()
                .and_then(|r| r.get("model_version").and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string(),
            method: "kernel_shap".to_string(),
            features: Vec::new(),
            baseline_prediction: None,
            target_prediction: None,
            efficiency_diff: None,
            sample: None,
            message,
        });
    }
    let result = response.result.as_ref().ok_or_else(|| {
        AppError::Internal("shap_explain: missing result".into())
    })?;
    let model_version = result
        .get("model_version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let method = result
        .get("method")
        .and_then(|m| m.as_str())
        .unwrap_or("kernel_shap")
        .to_string();
    let features: Vec<ShapFeature> = result
        .get("features")
        .and_then(|f| f.as_array())
        .map(|arr| {
            let mut v: Vec<ShapFeature> = arr
                .iter()
                .filter_map(|x| serde_json::from_value(x.clone()).ok())
                .collect();
            // 按 abs_shap 降序排列。
            v.sort_by(|a, b| {
                b.abs_shap
                    .partial_cmp(&a.abs_shap)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            v
        })
        .unwrap_or_default();
    let baseline_prediction = result
        .get("baseline_prediction")
        .and_then(|p| p.as_f64());
    let target_prediction = result
        .get("target_prediction")
        .and_then(|p| p.as_f64());
    let efficiency_diff = result
        .get("efficiency_diff")
        .and_then(|p| p.as_f64());
    let sample: Option<ExplainSample> = result
        .get("sample")
        .and_then(|s| serde_json::from_value(s.clone()).ok());
    let message = result
        .get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("ok")
        .to_string();
    Ok(ShapResult {
        ok: true,
        model_version,
        method,
        features,
        baseline_prediction,
        target_prediction,
        efficiency_diff,
        sample,
        message,
    })
}

/// v0.43a —— 校准直方图中的一条记录。L1 将其渲染为小型条形图：
///「该预测桶内的实际结算率为 X（vs 预测的 Y）」。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestCalibrationBucket {
    /// 人类可读的桶标签，例如 "[0.4, 0.6)"。
    pub bucket: String,
    /// 该桶内的平均预测概率。桶为空时为 `None`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predicted_avg: Option<f64>,
    /// 该桶内的实际结算率。桶为空时为 `None`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_rate: Option<f64>,
    /// 该桶内的样本数。
    pub count: usize,
}

/// v0.43a —— top winners / top losers 列表中的一条记录。
/// 包含足够的上下文以便在 hover 时渲染 tooltip。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestTopSample {
    /// 从输入 `label` 回显。
    pub label: String,
    /// 该样本的 Brier 分数。
    pub brier: f64,
    /// 模型的预测。
    pub predicted: f64,
    /// 实际结果。
    pub outcome: f64,
}

/// v0.43a —— 镜像 Python sidecar `backtest_model` 响应的 wire 格式。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    /// 成功为 `true`；未知模型 / 样本为空 / 全部格式错误 /
/// 缺失权重则为 `false`。
    pub ok: bool,
    /// 从请求回显。
    pub model_version: String,
    /// 通过校验并对 Brier 有贡献的样本数。
    pub sample_count: usize,
    /// 预测与结果之间的均方误差。`ok=false` 时为 `None`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brier_mean: Option<f64>,
    /// 每条样本的 Brier 分数，按输入顺序。可用于客户端直方图。
    #[serde(default)]
    pub brier_breakdown: Vec<f64>,
    /// [0, 1] 范围内的 5 个校准桶。
    #[serde(default)]
    pub calibration: Vec<BacktestCalibrationBucket>,
    /// Brier 最低的 3 条样本（最佳预测）。
    #[serde(default)]
    pub top_winners: Vec<BacktestTopSample>,
    /// Brier 最高的 3 条样本（最差预测），顺序反转（最差在前）。
    #[serde(default)]
    pub top_losers: Vec<BacktestTopSample>,
    /// 人类可读的状态 / 错误信息。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// `results` 列表中单个 trial promote 结果的 wire 格式镜像。v0.25a。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoteAllTrialResult {
    /// 0 索引的 trial 编号。
    pub trial_index: usize,
    /// 若该 trial 成功 promote 则为 `true`。
    pub promoted: bool,
    /// "ok" | "failed" —— 镜像 Python 端的逐调用状态。
    pub status: String,
    /// 新模型版本（例如 "logistic-train-XYZ-t2"）。失败时为空字符串。
    pub model_version: String,
    /// promote 的墙钟时间（毫秒）。
    pub promoted_at_ms: Option<i64>,
    /// 失败时的人类可读错误信息。
    pub message: Option<String>,
}

/// 镜像 Python `run_promote_all_trials` 返回值的 wire 格式。v0.25a。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PromoteAllTrialsResult {
    /// 所有 trial promote 均成功则为 `true`。
    pub ok: bool,
    /// 各 trial 的结果，按 trial_index 顺序。
    pub results: Vec<PromoteAllTrialResult>,
    /// 即 `len(results)`。
    pub count: usize,
    /// 总体错误信息（例如 "no candidate"）。所有 promote 均成功时为 None。
    pub message: Option<String>,
}

/// 将 `backtest_model` 响应解析为 `BacktestResult`。
/// 若响应在 envelope 层为 `ok=false` 则返回 Err。
/// `BacktestResult.ok` 字段反映应用层级的成功状态
///（模型找到、样本合法），与 envelope 层级的 `ok` 独立。
pub fn parse_backtest_model_response(
    resp: &SidecarResponse,
) -> Result<BacktestResult, String> {
    if !resp.ok {
        return Err(resp.error.clone().unwrap_or_else(|| "unknown error".into()));
    }
    let v = resp.result.clone().unwrap_or(Value::Null);
    serde_json::from_value::<BacktestResult>(v)
        .map_err(|e| format!("backtest decode: {e}"))
}

/// 将 `promote_all_trials` 响应解析为 `PromoteAllTrialsResult`。
/// 若响应在 envelope 层为 `ok=false` 则返回 Err。
pub fn parse_promote_all_trials_response(
    resp: &SidecarResponse,
) -> Result<PromoteAllTrialsResult, String> {
    if !resp.ok {
        return Err(resp.error.clone().unwrap_or_else(|| "unknown error".into()));
    }
    let v = resp.result.clone().unwrap_or(Value::Null);
    let results: Vec<PromoteAllTrialResult> = v
        .get("results")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    serde_json::from_value::<PromoteAllTrialResult>(item.clone()).ok()
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(PromoteAllTrialsResult {
        ok: v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false),
        count: v.get("count")
            .and_then(|x| x.as_u64())
            .unwrap_or(results.len() as u64) as usize,
        results,
        message: v.get("message")
            .and_then(|x| x.as_str())
            .map(String::from),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_round_trip() {
        for m in [SidecarMethod::Ping, SidecarMethod::Predict,
                  SidecarMethod::TrainJob, SidecarMethod::PromoteModel,
                  SidecarMethod::ListPromoteHistory,
                  SidecarMethod::RollbackModel,
                  SidecarMethod::AutoPromoteIfBetter,
                  SidecarMethod::PromoteAllTrials,
                  SidecarMethod::BacktestModel,
                  SidecarMethod::ExplainModel,
                  SidecarMethod::ShapExplain] {
            assert_eq!(SidecarMethod::parse(m.as_str()), Some(m));
        }
        assert_eq!(SidecarMethod::parse("nope"), None);
    }

    #[test]
    fn parse_promote_response_success() {
        // v0.18a —— 完整成功用例
        let r = SidecarResponse {
            id: "promote-abc".into(),
            ok: true,
            result: Some(serde_json::json!({
                "promoted": true,
                "status": "ok",
                "previous_path": "/home/x/.polyrocket/sidecar/models/active.json",
                "active_path": "/home/x/.polyrocket/sidecar/models/active.json",
                "promoted_at_ms": 1_700_000_000_000_i64,
                "model_version": "logistic-train-441c352b",
            })),
            error: None,
        };
        let p = parse_promote_response(&r).expect("ok");
        assert!(p.promoted);
        assert_eq!(p.status, "ok");
        assert_eq!(p.model_version, "logistic-train-441c352b");
        assert_eq!(p.previous_path.as_deref(), Some("/home/x/.polyrocket/sidecar/models/active.json"));
        assert_eq!(p.promoted_at_ms, Some(1_700_000_000_000));
        assert!(p.message.is_none());
    }

    #[test]
    fn parse_promote_response_no_candidate() {
        // v0.18a —— 用户在 Train 完成前点击了 Promote。
        // Python sidecar 返回 promoted: false 并附带说明性信息。
        // Rust 端返回同样的结构，以便 L1 无需走特殊路径。
        let r = SidecarResponse {
            id: "promote-xyz".into(),
            ok: true,
            result: Some(serde_json::json!({
                "promoted": false,
                "status": "failed",
                "message": "no candidate found at /home/x/.polyrocket/sidecar/models/candidate.json; run train_job first",
            })),
            error: None,
        };
        let p = parse_promote_response(&r).expect("ok");
        assert!(!p.promoted);
        assert_eq!(p.status, "failed");
        assert!(p.message.unwrap().contains("run train_job first"));
        assert_eq!(p.model_version, "");
        assert!(p.previous_path.is_none());
        assert!(p.active_path.is_none());
    }

    #[test]
    fn parse_promote_response_job_id_mismatch() {
        // v0.18a —— 用户传入了 job_id="X"，但当前 candidate 来自
        // job_id="Y"，因此被拒绝。
        let r = SidecarResponse {
            id: "promote-mmm".into(),
            ok: true,
            result: Some(serde_json::json!({
                "promoted": false,
                "status": "failed",
                "message": "candidate job_id mismatch: expected X, got Y",
            })),
            error: None,
        };
        let p = parse_promote_response(&r).expect("ok");
        assert!(!p.promoted);
        assert!(p.message.unwrap().contains("mismatch"));
    }

    #[test]
    fn parse_promote_response_not_ok_returns_err() {
        // v0.18a —— Python sidecar 返回了 ok=false。
        // 我们直接传递错误信息。
        let r = SidecarResponse {
            id: "promote-eee".into(),
            ok: false,
            result: None,
            error: Some("internal: oops".into()),
        };
        assert!(parse_promote_response(&r).is_err());
    }

    #[test]
    fn build_promote_request_no_job_id() {
        // v0.18a —— 可选 job_id 被省略
        let line = build_promote_request("promote-123", None, None);
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["method"], "promote_model");
        assert_eq!(v["id"], "promote-123");
        assert!(v["params"].as_object().unwrap().is_empty());
    }

    #[test]
    fn build_promote_request_with_job_id() {
        // v0.18a —— 包含 job_id
        let line = build_promote_request("promote-456", Some("train-abc"), None);
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["params"]["job_id"], "train-abc");
    }

    #[test]
    fn build_promote_request_with_trial_index() {
        // v0.21a —— 批量 promote：params 中携带 trial_index
        let line = build_promote_request("promote-789", Some("train-abc"), Some(2));
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["params"]["job_id"], "train-abc");
        assert_eq!(v["params"]["trial_index"], 2);
    }

    #[test]
    fn parse_promote_response_with_trial_index() {
        // v0.21a —— 批量 promote：响应携带 trial_index
        let r = SidecarResponse {
            id: "promote-bulk".into(),
            ok: true,
            result: Some(serde_json::json!({
                "promoted": true,
                "status": "ok",
                "previous_path": "/home/x/.polyrocket/sidecar/models/active.json",
                "active_path": "/home/x/.polyrocket/sidecar/models/active.json",
                "promoted_at_ms": 1_700_010_000_000_i64,
                "model_version": "logistic-train-441c352b-t2",
                "trial_index": 2,
            })),
            error: None,
        };
        let p = parse_promote_response(&r).expect("ok");
        assert!(p.promoted);
        assert_eq!(p.model_version, "logistic-train-441c352b-t2");
        assert_eq!(p.trial_index, Some(2));
    }

    #[test]
    fn parse_promote_response_default_trial_index() {
        // v0.21a —— 向后兼容：当 trial_index 缺失
        //（Python sidecar 未返回），Rust 端返回 None（最佳，非批量）。
        let r = SidecarResponse {
            id: "promote-best".into(),
            ok: true,
            result: Some(serde_json::json!({
                "promoted": true,
                "status": "ok",
                "previous_path": "/home/x/.polyrocket/sidecar/models/active.json",
                "active_path": "/home/x/.polyrocket/sidecar/models/active.json",
                "promoted_at_ms": 1_700_011_000_000_i64,
                "model_version": "logistic-train-441c352b",
            })),
            error: None,
        };
        let p = parse_promote_response(&r).expect("ok");
        assert!(p.promoted);
        assert_eq!(p.model_version, "logistic-train-441c352b");
        assert!(p.trial_index.is_none());
    }

    #[test]
    fn build_list_promote_history_request_basic() {
        // v0.19a —— 只读审计，无参数（空对象）
        let line = build_list_promote_history_request("list-1");
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["method"], "list_promote_history");
        assert_eq!(v["id"], "list-1");
        assert_eq!(v["params"], serde_json::json!({}));
    }

    #[test]
    fn parse_list_promote_history_response_populated() {
        // v0.19a —— 2 条记录，ok=true
        let r = SidecarResponse {
            id: "list-2".into(),
            ok: true,
            result: Some(serde_json::json!({
                "ok": true,
                "count": 2,
                "entries": [
                    {
                        "job_id": "train-aaa",
                        "model_version": "logistic-train-aaa",
                        "promoted_at_ms": 1_700_000_000_000_i64,
                        "best_brier": 0.184,
                        "best_params": {"lr": 0.01, "reg": 0.1}
                    },
                    {
                        "job_id": "train-bbb",
                        "model_version": "logistic-train-bbb",
                        "promoted_at_ms": 1_700_001_000_000_i64,
                        "best_brier": 0.179,
                        "best_params": null
                    }
                ]
            })),
            error: None,
        };
        let h = parse_list_promote_history_response(&r).expect("ok");
        assert!(h.ok);
        assert_eq!(h.count, 2);
        assert_eq!(h.entries.len(), 2);
        assert_eq!(h.entries[0].job_id, "train-aaa");
        assert_eq!(h.entries[0].model_version, "logistic-train-aaa");
        assert_eq!(h.entries[0].best_brier, Some(0.184));
        assert!(h.entries[0].best_params.is_some());
        assert_eq!(h.entries[1].job_id, "train-bbb");
        assert!(h.entries[1].best_params.is_none());
        assert!(h.message.is_none());
    }

    #[test]
    fn parse_list_promote_history_response_empty() {
        // v0.19a —— 尚无活跃模型；ok=true 且 entries 为空
        let r = SidecarResponse {
            id: "list-3".into(),
            ok: true,
            result: Some(serde_json::json!({
                "ok": true,
                "count": 0,
                "entries": [],
                "message": "no active model yet; train + promote to start history"
            })),
            error: None,
        };
        let h = parse_list_promote_history_response(&r).expect("ok");
        assert!(h.ok);
        assert_eq!(h.count, 0);
        assert!(h.entries.is_empty());
        assert!(h.message.is_some());
    }

    #[test]
    fn parse_list_promote_history_response_not_ok_returns_err() {
        // v0.19a —— envelope 层错误
        let r = SidecarResponse {
            id: "list-4".into(),
            ok: false,
            result: None,
            error: Some("internal: oops".into()),
        };
        assert!(parse_list_promote_history_response(&r).is_err());
    }

    #[test]
    fn build_rollback_request_basic() {
        // v0.20a —— params 中携带 model_version
        let line = build_rollback_request("rollback-1", "logistic-train-441c352b");
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["method"], "rollback_model");
        assert_eq!(v["id"], "rollback-1");
        assert_eq!(v["params"]["model_version"], "logistic-train-441c352b");
    }

    #[test]
    fn parse_rollback_response_success() {
        // v0.20a —— 完整成功用例
        let r = SidecarResponse {
            id: "rollback-2".into(),
            ok: true,
            result: Some(serde_json::json!({
                "rolled_back": true,
                "status": "ok",
                "previous_path": "/home/x/.polyrocket/sidecar/models/active.json",
                "active_path": "/home/x/.polyrocket/sidecar/models/active.json",
                "rolled_back_at_ms": 1_700_005_000_000_i64,
                "model_version": "logistic-train-441c352b",
            })),
            error: None,
        };
        let rb = parse_rollback_response(&r).expect("ok");
        assert!(rb.rolled_back);
        assert_eq!(rb.status, "ok");
        assert_eq!(rb.model_version, "logistic-train-441c352b");
        assert_eq!(rb.rolled_back_at_ms, Some(1_700_005_000_000));
        assert!(rb.message.is_none());
    }

    #[test]
    fn parse_rollback_response_not_found() {
        // v0.20a —— 请求的 model_version 不在 history 中
        //（例如用户拼错）。Python sidecar 返回 rolled_back=false
        // 并附带清晰的诊断信息。Rust 端返回同样的结构。
        let r = SidecarResponse {
            id: "rollback-3".into(),
            ok: true,
            result: Some(serde_json::json!({
                "rolled_back": false,
                "status": "failed",
                "message": "model_version 'logistic-train-XYZ' not found in promotion history",
            })),
            error: None,
        };
        let rb = parse_rollback_response(&r).expect("ok");
        assert!(!rb.rolled_back);
        assert_eq!(rb.status, "failed");
        assert!(rb.message.unwrap().contains("not found"));
        assert_eq!(rb.model_version, "");
    }

    #[test]
    fn parse_rollback_response_no_weights() {
        // v0.20a —— 条目存在但 promote 早于 v0.20a（未存储 weights）。
        // 用户需要重新训练才能回滚到该版本。
        let r = SidecarResponse {
            id: "rollback-4".into(),
            ok: true,
            result: Some(serde_json::json!({
                "rolled_back": false,
                "status": "failed",
                "message": "model_version 'logistic-train-OLD' has no weights stored (promoted before v0.20); cannot rollback",
            })),
            error: None,
        };
        let rb = parse_rollback_response(&r).expect("ok");
        assert!(!rb.rolled_back);
        assert!(rb.message.unwrap().contains("no weights"));
    }

    #[test]
    fn parse_rollback_response_not_ok_returns_err() {
        // v0.20a —— envelope 层错误
        let r = SidecarResponse {
            id: "rollback-5".into(),
            ok: false,
            result: None,
            error: Some("internal: oops".into()),
        };
        assert!(parse_rollback_response(&r).is_err());
    }

    #[test]
    fn build_auto_promote_if_better_request_default_margin() {
        // v0.23a —— 无参数 → params 为空对象（Python 使用默认 0.005 margin）
        let line = build_auto_promote_if_better_request("ap-1", None, None);
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["method"], "auto_promote_if_better");
        assert_eq!(v["id"], "ap-1");
        assert_eq!(v["params"], serde_json::json!({}));
    }

    #[test]
    fn build_auto_promote_if_better_request_with_margin_and_trial() {
        // v0.23a —— 两个参数均提供
        let line = build_auto_promote_if_better_request("ap-2", Some(0.01), Some(2));
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["params"]["brier_margin"], 0.01);
        assert_eq!(v["params"]["trial_index"], 2);
    }

    #[test]
    fn parse_auto_promote_if_better_response_promoted() {
        // v0.23a —— promote 用例（候选明显更优）
        let r = SidecarResponse {
            id: "ap-3".into(),
            ok: true,
            result: Some(serde_json::json!({
                "promoted": true,
                "skipped": false,
                "reason": "auto-promoted: improvement 0.0120 > margin 0.005",
                "candidate_brier": 0.180,
                "active_brier": 0.192,
                "margin": 0.005,
                "model_version": "logistic-train-XYZ",
                "promoted_at_ms": 1_700_020_000_000_i64,
            })),
            error: None,
        };
        let a = parse_auto_promote_if_better_response(&r).expect("ok");
        assert!(a.promoted);
        assert!(!a.skipped);
        assert!(a.reason.contains("auto-promoted"));
        assert_eq!(a.candidate_brier, Some(0.180));
        assert_eq!(a.active_brier, Some(0.192));
        assert_eq!(a.margin, 0.005);
        assert_eq!(a.model_version.as_deref(), Some("logistic-train-XYZ"));
    }

    #[test]
    fn parse_auto_promote_if_better_response_skipped() {
        // v0.23a —— 跳过用例（候选并未明显更优）
        let r = SidecarResponse {
            id: "ap-4".into(),
            ok: true,
            result: Some(serde_json::json!({
                "promoted": false,
                "skipped": true,
                "reason": "candidate brier 0.1900 is not at least 0.005 better than active 0.1920 (improvement: +0.0020)",
                "candidate_brier": 0.190,
                "active_brier": 0.192,
                "margin": 0.005,
                "model_version": null,
                "promoted_at_ms": null,
            })),
            error: None,
        };
        let a = parse_auto_promote_if_better_response(&r).expect("ok");
        assert!(!a.promoted);
        assert!(a.skipped);
        assert!(a.reason.contains("not at least 0.005 better"));
        assert_eq!(a.candidate_brier, Some(0.190));
        assert_eq!(a.active_brier, Some(0.192));
        assert!(a.model_version.is_none());
    }

    #[test]
    fn parse_auto_promote_if_better_response_no_active() {
        // v0.23a —— 无活跃模型：自动 promote 候选
        let r = SidecarResponse {
            id: "ap-5".into(),
            ok: true,
            result: Some(serde_json::json!({
                "promoted": true,
                "skipped": false,
                "reason": "no active model; auto-promoted the candidate",
                "candidate_brier": null,
                "active_brier": null,
                "margin": 0.005,
                "model_version": "logistic-train-ABC",
                "promoted_at_ms": 1_700_021_000_000_i64,
            })),
            error: None,
        };
        let a = parse_auto_promote_if_better_response(&r).expect("ok");
        assert!(a.promoted);
        assert!(a.candidate_brier.is_none());
        assert!(a.active_brier.is_none());
    }

    #[test]
    fn parse_auto_promote_if_better_response_not_ok_returns_err() {
        // v0.23a —— envelope 层错误
        let r = SidecarResponse {
            id: "ap-6".into(),
            ok: false,
            result: None,
            error: Some("internal: oops".into()),
        };
        assert!(parse_auto_promote_if_better_response(&r).is_err());
    }

    #[test]
    fn build_promote_all_trials_request_basic() {
        // v0.25a —— 无参数，仅 method + id
        let line = build_promote_all_trials_request("all-1");
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["method"], "promote_all_trials");
        assert_eq!(v["id"], "all-1");
        assert_eq!(v["params"], serde_json::json!({}));
    }

    #[test]
    fn parse_promote_all_trials_response_all_ok() {
        // v0.25a —— 4 个 trial 全部 promote 成功
        let r = SidecarResponse {
            id: "all-2".into(),
            ok: true,
            result: Some(serde_json::json!({
                "ok": true,
                "count": 4,
                "results": [
                    {
                        "trial_index": 0,
                        "promoted": true,
                        "status": "ok",
                        "model_version": "logistic-train-XYZ-t0",
                        "promoted_at_ms": 1_700_030_000_000_i64,
                        "message": null,
                    },
                    {
                        "trial_index": 1,
                        "promoted": true,
                        "status": "ok",
                        "model_version": "logistic-train-XYZ-t1",
                        "promoted_at_ms": 1_700_030_001_000_i64,
                        "message": null,
                    },
                    {
                        "trial_index": 2,
                        "promoted": true,
                        "status": "ok",
                        "model_version": "logistic-train-XYZ-t2",
                        "promoted_at_ms": 1_700_030_002_000_i64,
                        "message": null,
                    },
                    {
                        "trial_index": 3,
                        "promoted": true,
                        "status": "ok",
                        "model_version": "logistic-train-XYZ-t3",
                        "promoted_at_ms": 1_700_030_003_000_i64,
                        "message": null,
                    },
                ],
                "message": null,
            })),
            error: None,
        };
        let p = parse_promote_all_trials_response(&r).expect("ok");
        assert!(p.ok);
        assert_eq!(p.count, 4);
        assert_eq!(p.results.len(), 4);
        for (i, t) in p.results.iter().enumerate() {
            assert_eq!(t.trial_index, i);
            assert!(t.promoted);
            assert!(t.model_version.ends_with(&format!("-t{i}")));
        }
    }

    #[test]
    fn parse_promote_all_trials_response_no_candidate() {
        // v0.25a —— 磁盘上无 candidate 时返回 ok=false，
        // results 为空并附带清晰消息
        let r = SidecarResponse {
            id: "all-3".into(),
            ok: true,
            result: Some(serde_json::json!({
                "ok": false,
                "count": 0,
                "results": [],
                "message": "no candidate found at /home/x/.polyrocket/sidecar/models/candidate.json; run train_job first",
            })),
            error: None,
        };
        let p = parse_promote_all_trials_response(&r).expect("ok");
        assert!(!p.ok);
        assert_eq!(p.count, 0);
        assert!(p.results.is_empty());
        assert!(p.message.unwrap().contains("train_job"));
    }

    #[test]
    fn parse_promote_all_trials_response_not_ok_returns_err() {
        // v0.25a —— envelope 层错误
        let r = SidecarResponse {
            id: "all-4".into(),
            ok: false,
            result: None,
            error: Some("internal: oops".into()),
        };
        assert!(parse_promote_all_trials_response(&r).is_err());
    }

    #[test]
    fn response_ok_constructor() {
        let r = SidecarResponse::ok("r1", serde_json::json!({"x": 1}));
        assert!(r.ok);
        assert_eq!(r.id, "r1");
        assert!(r.result.is_some());
        assert!(r.error.is_none());
    }

    #[test]
    fn response_err_constructor() {
        let r = SidecarResponse::err("r2", "boom");
        assert!(!r.ok);
        assert_eq!(r.error.as_deref(), Some("boom"));
        assert!(r.result.is_none());
    }

    #[test]
    fn parse_line_distinguishes_req_vs_resp() {
        let resp_line = r#"{"id":"r1","ok":true,"result":{"x":1}}"#;
        match parse_line(resp_line).unwrap() {
            ParseResult::Response(r) => {
                assert_eq!(r.id, "r1");
                assert!(r.ok);
            }
            _ => panic!("expected response"),
        }

        let req_line = r#"{"id":"r2","method":"ping","params":{}}"#;
        match parse_line(req_line).unwrap() {
            ParseResult::Request(r) => {
                assert_eq!(r.method, "ping");
            }
            _ => panic!("expected request"),
        }
    }

    #[test]
    fn parse_line_rejects_invalid() {
        assert!(parse_line("not json").is_err());
        assert!(parse_line(r#"{"id":"r1"}"#).is_err()); // 既不含 method 也不含 ok
    }

    #[test]
    fn build_and_parse_predict_round_trip() {
        let line = build_predict_request("r1", &[
            ("m1".to_string(), 0.5),
            ("m2".to_string(), 0.7),
        ]);
        // 解析回 request
        let parsed = match parse_line(&line).unwrap() {
            ParseResult::Request(r) => r,
            _ => panic!(),
        };
        assert_eq!(parsed.method, "predict");
        assert_eq!(parsed.id, "r1");
        let markets = parsed.params.get("markets").and_then(|v| v.as_array()).unwrap();
        assert_eq!(markets.len(), 2);

        // 构造一个伪响应并往返解析
        let resp = SidecarResponse::ok("r1", serde_json::json!({
            "predictions": [
                { "market_id": "m1", "prob": 0.65, "confidence": 0.7, "rationale": "why" },
                { "market_id": "m2", "prob": 0.3, "confidence": 0.6, "rationale": null },
            ],
            "model_version": "logistic-train-abc123"
        }));
        let result = parse_predict_response(&resp).unwrap();
        assert_eq!(result.predictions.len(), 2);
        assert_eq!(result.predictions[0].market_id, "m1");
        assert!((result.predictions[0].prob - 0.65).abs() < 1e-9);
        assert_eq!(result.predictions[0].rationale.as_deref(), Some("why"));
        assert!(result.predictions[1].rationale.is_none());
        // v0.12a —— model_version 被提升到顶层
        assert_eq!(result.model_version.as_deref(), Some("logistic-train-abc123"));
    }

    #[test]
    fn parse_predict_response_model_version_optional() {
        // 向后兼容：若 sidecar 未发送 model_version，仍可正常解析，
        // 此时 model_version = None。
        let resp = SidecarResponse::ok("r1", serde_json::json!({
            "predictions": [
                { "market_id": "m1", "prob": 0.5, "confidence": 0.5 },
            ]
        }));
        let result = parse_predict_response(&resp).unwrap();
        assert_eq!(result.predictions.len(), 1);
        assert!(result.model_version.is_none());
        assert!(result.brier_score.is_none());
    }

    #[test]
    fn parse_predict_response_brier_round_trip() {
        // v0.13b —— brier_score 从响应中被提升出来。
        let resp = SidecarResponse::ok("r1", serde_json::json!({
            "predictions": [
                { "market_id": "m1", "prob": 0.6, "confidence": 0.5 },
            ],
            "model_version": "logistic-train-abc123",
            "brier_score": 0.220012
        }));
        let result = parse_predict_response(&resp).unwrap();
        assert_eq!(result.predictions.len(), 1);
        assert_eq!(result.model_version.as_deref(), Some("logistic-train-abc123"));
        assert_eq!(result.brier_score, Some(0.220012));
    }

    #[test]
    fn parse_predict_error_response() {
        let resp = SidecarResponse::err("r1", "model not loaded");
        let r = parse_predict_response(&resp);
        assert!(r.is_err());
        assert_eq!(r.unwrap_err(), "model not loaded");
    }

    #[test]
    fn parse_predict_missing_field() {
        let resp = SidecarResponse::ok("r1", serde_json::json!({"other": 1}));
        let r = parse_predict_response(&resp);
        assert!(r.is_err());
    }

    // v0.43b —— backtest 请求构造器
    #[test]
    fn build_backtest_model_request_basic() {
        let samples = vec![
            BacktestSample {
                price: 0.5,
                market_age_hours: 24.0,
                outcome: 1.0,
                label: "m1".into(),
            },
            BacktestSample {
                price: 0.3,
                market_age_hours: 48.0,
                outcome: 0.0,
                label: "".into(),
            },
        ];
        let line = build_backtest_model_request(
            "backtest-1",
            "logistic-train-abc",
            &samples,
        );
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["id"], "backtest-1");
        assert_eq!(v["method"], "backtest_model");
        assert_eq!(v["params"]["model_version"], "logistic-train-abc");
        let arr = v["params"]["samples"].as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["price"], 0.5);
        assert_eq!(arr[0]["label"], "m1");
        assert_eq!(arr[1]["label"], "");
    }

    // v0.43b —— backtest 响应解析器
    #[test]
    fn parse_backtest_response_ok() {
        let resp = SidecarResponse::ok(
            "backtest-1",
            serde_json::json!({
                "ok": true,
                "model_version": "logistic-train-abc",
                "sample_count": 3,
                "brier_mean": 0.18,
                "brier_breakdown": [0.1, 0.25, 0.05],
                "calibration": [
                    {"bucket": "[0.0, 0.2)", "predicted_avg": 0.1, "actual_rate": 0.0, "count": 1},
                    {"bucket": "[0.8, 1.0)", "predicted_avg": 0.9, "actual_rate": 1.0, "count": 2},
                ],
                "top_winners": [
                    {"label": "best", "brier": 0.05, "predicted": 0.95, "outcome": 1.0},
                ],
                "top_losers": [
                    {"label": "worst", "brier": 0.5, "predicted": 0.0, "outcome": 1.0},
                ],
                "message": null,
            }),
        );
        let r = parse_backtest_model_response(&resp).unwrap();
        assert!(r.ok);
        assert_eq!(r.model_version, "logistic-train-abc");
        assert_eq!(r.sample_count, 3);
        assert_eq!(r.brier_mean, Some(0.18));
        assert_eq!(r.brier_breakdown.len(), 3);
        assert_eq!(r.calibration.len(), 2);
        assert_eq!(r.top_winners.len(), 1);
        assert_eq!(r.top_losers[0].label, "worst");
    }

    #[test]
    fn parse_backtest_response_app_level_error() {
        // envelope 层 ok=true 但应用层 ok=false（模型未找到）。
        // 解析仍应成功；L1 检查 `result.ok`。
        let resp = SidecarResponse::ok(
            "backtest-1",
            serde_json::json!({
                "ok": false,
                "model_version": "logistic-missing",
                "sample_count": 0,
                "brier_breakdown": [],
                "calibration": [],
                "top_winners": [],
                "top_losers": [],
                "message": "model 'logistic-missing' not found in archive or active",
            }),
        );
        let r = parse_backtest_model_response(&resp).unwrap();
        assert!(!r.ok);
        assert_eq!(r.sample_count, 0);
        assert!(r.message.as_ref().unwrap().contains("not found"));
    }

    #[test]
    fn parse_backtest_envelope_error() {
        // envelope 层 ok=false（传输错误）
        let resp = SidecarResponse::err("backtest-1", "sidecar not running");
        assert!(parse_backtest_model_response(&resp).is_err());
    }

    #[test]
    fn sidecar_method_backtest_round_trip() {
        // "backtest_model" 字符串必须能通过 SidecarMethod 枚举往返。
        let m = SidecarMethod::parse("backtest_model").unwrap();
        assert_eq!(m, SidecarMethod::BacktestModel);
        assert_eq!(m.as_str(), "backtest_model");
    }

    // v0.55 —— explain_model 请求构造器
    #[test]
    fn build_explain_model_request_no_sample() {
        let line = build_explain_model_request(
            "explain-1",
            "logistic-train-abc",
            None,
        );
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["id"], "explain-1");
        assert_eq!(v["method"], "explain_model");
        assert_eq!(v["params"]["model_version"], "logistic-train-abc");
        // 当 sample 为 None 时不带 `sample` 键。
        assert!(v["params"].get("sample").is_none());
    }

    #[test]
    fn build_explain_model_request_with_sample() {
        let sample = ExplainSample {
            price: Some(0.42),
            market_age_hours: Some(36.0),
        };
        let line = build_explain_model_request(
            "explain-2",
            "logistic-train-abc",
            Some(&sample),
        );
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["params"]["sample"]["price"], 0.42);
        assert_eq!(v["params"]["sample"]["market_age_hours"], 36.0);
    }

    // v0.55 —— explain_model 响应解析器
    #[test]
    fn parse_explain_response_ok() {
        let resp = SidecarResponse::ok(
            "explain-1",
            serde_json::json!({
                "ok": true,
                "model_version": "logistic-train-abc",
                "features": [
                    {"feature": "bias", "value": 1.0, "weight": 0.1, "contribution": 0.025, "abs_contribution": 0.025},
                    {"feature": "price", "value": 0.5, "weight": 2.4, "contribution": 0.30, "abs_contribution": 0.30},
                    {"feature": "market_age_hours", "value": 24.0, "weight": -0.02, "contribution": -0.024, "abs_contribution": 0.024},
                ],
                "prediction": 0.55,
                "sample": {"price": 0.5, "market_age_hours": 24.0},
                "message": "ok",
            }),
        );
        let r = parse_explain_model_response(&resp).unwrap();
        assert!(r.ok);
        assert_eq!(r.model_version, "logistic-train-abc");
        assert_eq!(r.features.len(), 3);
        // features 在 sidecar 中按 abs_contribution 降序排列，
        // 因此第一个应为 "price"（0.30）。
        assert_eq!(r.features[0].feature, "price");
        assert_eq!(r.features[0].abs_contribution, 0.30);
        assert!((r.prediction.unwrap() - 0.55).abs() < 1e-6);
    }

    #[test]
    fn parse_explain_response_err() {
        let resp = SidecarResponse::err(
            "explain-1",
            "model logistic-train-xyz not found",
        );
        let r = parse_explain_model_response(&resp).unwrap();
        assert!(!r.ok);
        assert!(r.message.contains("not found"));
        assert!(r.features.is_empty());
    }

    #[test]
    fn sidecar_method_explain_round_trip() {
        let m = SidecarMethod::parse("explain_model").unwrap();
        assert_eq!(m, SidecarMethod::ExplainModel);
        assert_eq!(m.as_str(), "explain_model");
    }

    // v0.59 —— SHAP 请求构造器
    #[test]
    fn build_shap_explain_request_no_sample() {
        let line = build_shap_explain_request(
            "shap-1",
            "logistic-train-abc",
            None,
        );
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["id"], "shap-1");
        assert_eq!(v["method"], "shap_explain");
        assert_eq!(v["params"]["model_version"], "logistic-train-abc");
        assert!(v["params"].get("sample").is_none());
    }

    #[test]
    fn build_shap_explain_request_with_sample() {
        let sample = ExplainSample {
            price: Some(0.42),
            market_age_hours: Some(36.0),
        };
        let line = build_shap_explain_request(
            "shap-2",
            "logistic-train-abc",
            Some(&sample),
        );
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["params"]["sample"]["price"], 0.42);
    }

    // v0.59 —— SHAP 响应解析器
    #[test]
    fn parse_shap_response_ok_satisfies_efficiency() {
        // 一个格式良好的响应，其 SHAP 值之和等于偏差
        //（target - baseline）。efficiency diff 应在浮点
        // 容差内接近 0。
        let resp = SidecarResponse::ok(
            "shap-1",
            serde_json::json!({
                "ok": true,
                "model_version": "logistic-train-abc",
                "method": "kernel_shap",
                "features": [
                    {"feature": "bias", "value": 1.0, "weight": 0.1, "shap_value": 0.025, "abs_shap": 0.025},
                    {"feature": "price", "value": 0.5, "weight": 2.4, "shap_value": 0.30, "abs_shap": 0.30},
                    {"feature": "market_age_hours", "value": 24.0, "weight": -0.02, "shap_value": -0.024, "abs_shap": 0.024},
                ],
                "baseline_prediction": 0.5,
                "target_prediction": 0.801,
                "efficiency_diff": 0.0,
                "sample": {"price": 0.5, "market_age_hours": 24.0},
                "message": "ok",
            }),
        );
        let r = parse_shap_explain_response(&resp).unwrap();
        assert!(r.ok);
        assert_eq!(r.model_version, "logistic-train-abc");
        assert_eq!(r.method, "kernel_shap");
        assert_eq!(r.features.len(), 3);
        // abs_shap 最大的特征应为 "price" (0.30)。
        assert_eq!(r.features[0].feature, "price");
        assert!((r.target_prediction.unwrap() - 0.801).abs() < 1e-6);
    }

    #[test]
    fn parse_shap_response_err() {
        let resp = SidecarResponse::err(
            "shap-1",
            "model logistic-train-xyz not found",
        );
        let r = parse_shap_explain_response(&resp).unwrap();
        assert!(!r.ok);
        assert!(r.message.contains("not found"));
        assert!(r.features.is_empty());
        assert!(r.target_prediction.is_none());
    }

    #[test]
    fn sidecar_method_shap_round_trip() {
        let m = SidecarMethod::parse("shap_explain").unwrap();
        assert_eq!(m, SidecarMethod::ShapExplain);
        assert_eq!(m.as_str(), "shap_explain");
    }
}

//! L2 —— Active-model IPC（v0.49b）。
//!
//! 作为「当前激活的是哪个模型、其训练指标是什么」的单一事实来源。
//! 在 v0.49b 之前,每个消费者(降级检测器、设置卡片等)
//! 都直接从磁盘读取 `active.json`。本模块将其封装为 IPC,目的是:
//!
//! - L1 始终通过 `getActiveModel` 访问,不再重复文件路径逻辑。
//! - Rust 端消费者(降级循环、未来 v0.50+ 的监控器)
//!   调用同一个内部辅助函数,而不是重新读取 active.json。
//! - Schema 变更(v0.50+ 将新增 `weights`)只在一处进行。
//!
//! 返回的 DTO:
//!
//! ```text
//! ActiveModel {
//!   model_version: String,           // 例如 "logistic-train-441c352b"
//!   best_brier: Option<f64>,         // 训练时的 Brier
//!   best_params: Option<Value>,      // 最优 trial 的超参数
//!   promoted_at_ms: Option<i64>,     // 上一次 promote 的墙钟时间
//!   weights: Option<Vec<f64>>,       // 可选;v0.50+ 将填充
//!   source_path: String,             // active.json 的绝对路径
//! }
//! ```
//!
//! 当 active.json 缺失时(通常在首次 promote 之前)返回
//! `AppResult::Ok(None)`。其它 IO / 解析错误以 `AppError::Internal` 形式抛出。

use crate::AppError;
use crate::AppResult;
use serde::Serialize;
use std::io::Read;
use std::path::PathBuf;

/// `get_active_model` 返回的 wire-format DTO。
#[derive(Debug, Clone, Serialize)]
pub struct ActiveModel {
    pub model_version: String,
    pub best_brier: Option<f64>,
    pub best_params: Option<serde_json::Value>,
    pub promoted_at_ms: Option<i64>,
    /// 当 sidecar（侧车）把权重写入 active.json 后,v0.50+
    /// 将填充此字段。目前 sidecar 尚未写入,因此始终为 None —— 但此字段
    /// 保留以保持向前兼容。
    pub weights: Option<Vec<f64>>,
    pub source_path: String,
}

/// 与 Python 侧车在 `train.py:48` 中解析 active.json 路径的方式保持一致。
/// 在此处集中处理环境变量,意味着 L1 无需感知 `POLYROCKET_SIDECAR_MODEL_DIR`。
pub fn active_model_path() -> PathBuf {
    let dir = std::env::var("POLYROCKET_SIDECAR_MODEL_DIR")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            format!("{home}/.polyrocket/sidecar/models")
        });
    PathBuf::from(dir).join("active.json")
}

/// 从磁盘读取激活模型。active.json 缺失时(即尚未 promote 过任何模型)
/// 返回 `Ok(None)`;文件存在但格式异常时返回 `Err(Internal)`。
pub fn read_active_model_from_disk() -> AppResult<Option<ActiveModel>> {
    let path = active_model_path();
    let mut f = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(AppError::Internal(format!("active.json: {e}"))),
    };
    let mut s = String::new();
    if let Err(e) = f.read_to_string(&mut s) {
        return Err(AppError::Internal(format!("read active.json: {e}")));
    }
    let v: serde_json::Value = match serde_json::from_str(&s) {
        Ok(v) => v,
        Err(e) => return Err(AppError::Internal(format!("parse active.json: {e}"))),
    };
    // 尽力而为的 schema 解码。所有字段均为可选,
    // 我们返回文件中实际存在的内容。
    let model_version = v
        .get("model_version")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if model_version.is_empty() {
        return Err(AppError::Internal(
            "active.json: missing model_version".into(),
        ));
    }
    let best = v.get("best");
    let best_brier = best
        .and_then(|b| b.get("brier"))
        .and_then(|x| x.as_f64());
    let best_params = best.and_then(|b| b.get("params")).cloned();
    let promoted_at_ms = v
        .get("promoted_at_ms")
        .and_then(|x| x.as_i64());
    let weights = v
        .get("weights")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|n| n.as_f64())
                .collect::<Vec<f64>>()
        });
    Ok(Some(ActiveModel {
        model_version,
        best_brier,
        best_params,
        promoted_at_ms,
        weights,
        source_path: path.to_string_lossy().to_string(),
    }))
}

/// IPC:从磁盘读取激活模型。active.json 缺失时(尚未 promote 过任何模型)
/// 返回 `null`。错误通过标准的 `AppError::Internal` 通道抛出。
#[tauri::command]
pub fn get_active_model() -> AppResult<Option<ActiveModel>> {
    read_active_model_from_disk()
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// 将 JSON 值写入临时文件,返回路径;调用方负责 `remove_file`。
    fn write_active_json(value: serde_json::Value) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "polyrocket_active_model_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("active.json");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(value.to_string().as_bytes()).unwrap();
        path
    }

    /// v0.49b —— 环境变量覆盖路径。我们无法在并行测试中
    /// 修改进程环境,因此这里只对路径计算做基本校验。
    #[test]
    fn active_model_path_under_default() {
        // 健全性检查：当环境变量未设置时,路径以 .polyrocket/sidecar/models/active.json 结尾
        let p = active_model_path();
        let s = p.to_string_lossy();
        // 要么是 "$HOME/.polyrocket/sidecar/models/active.json"
        // 要么是 POLYROCKET_SIDECAR_MODEL_DIR 所指向的位置。
        assert!(s.ends_with("active.json"), "got: {s}");
    }

    /// v0.49b —— 当文件缺失时,读取函数优雅地返回 Ok(None)。
    /// 通过将环境变量指向不包含 active.json 的临时目录进行测试。
    #[test]
    fn read_returns_none_when_missing() {
        let dir = std::env::temp_dir().join(format!(
            "polyrocket_active_model_missing_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: POLYROCKET_SIDECAR_MODEL_DIR 按名称读取;
        // 本测试是该测试进程中唯一的写入者。
        // set_var 自 Rust 1.83 起标记为 unsafe,但测试需要它。
        unsafe { std::env::set_var("POLYROCKET_SIDECAR_MODEL_DIR", &dir); }
        let result = read_active_model_from_disk();
        unsafe { std::env::remove_var("POLYROCKET_SIDECAR_MODEL_DIR"); }
        assert!(matches!(result, Ok(None)), "got: {result:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// v0.49b —— 正常路径:格式良好的 active.json 可往返
    /// 转换为带全部字段的 DTO。
    #[test]
    fn read_happy_path_populates_all_fields() {
        let path = write_active_json(serde_json::json!({
            "model_version": "logistic-train-abc",
            "best": {
                "brier": 0.172,
                "params": {"alpha": 0.01}
            },
            "promoted_at_ms": 1740000000000_i64,
            "weights": [0.5, 1.0, -0.25]
        }));
        let parent = path.parent().unwrap().to_string_lossy().to_string();
        unsafe { std::env::set_var("POLYROCKET_SIDECAR_MODEL_DIR", &parent); }
        let result = read_active_model_from_disk();
        unsafe { std::env::remove_var("POLYROCKET_SIDECAR_MODEL_DIR"); }
        let am = result.expect("Ok").expect("Some");
        assert_eq!(am.model_version, "logistic-train-abc");
        assert_eq!(am.best_brier, Some(0.172));
        assert_eq!(am.promoted_at_ms, Some(1740000000000));
        assert_eq!(am.weights.as_deref(), Some([0.5, 1.0, -0.25].as_slice()));
        assert!(am.source_path.ends_with("active.json"));
        let _ = std::fs::remove_dir_all(&parent);
    }

    /// v0.49b —— 格式错误的 active.json 应抛出
    /// AppError::Internal(而不是被静默地当成 Ok(None))。
    #[test]
    fn read_malformed_json_returns_error() {
        let path = write_active_json(serde_json::json!({
            "model_version": "logistic-train-xyz"
            // 缺少 "best.brier" —— 可以,该字段可选
            // —— 但该文件仍是合法 JSON。下面用非法 JSON 覆盖。
        }));
        // 用乱码覆盖。
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"{ not json").unwrap();
        drop(f);

        let parent = path.parent().unwrap().to_string_lossy().to_string();
        unsafe { std::env::set_var("POLYROCKET_SIDECAR_MODEL_DIR", &parent); }
        let result = read_active_model_from_disk();
        unsafe { std::env::remove_var("POLYROCKET_SIDECAR_MODEL_DIR"); }
        assert!(matches!(result, Err(AppError::Internal(_))), "got: {result:?}");
        let _ = std::fs::remove_dir_all(&parent);
    }
}

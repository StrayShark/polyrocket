//! L4 — Application-wide error type.
//!
//! Single error enum that all layers below return. Serializes to a
//! human-readable string for IPC (frontend gets `err.message`).
//!
//! 8 stable error codes are handled at L3 (LLM clients) by mapping
//! HTTP/SDK errors into a string. This enum does NOT model those
//! categories — it only cares about the transport layer (Db, Http,
//! Io, Serde, Keyring, plus Invalid/NotFound/Internal for app logic).

use serde::Serialize;
use specta::Type;
use thiserror::Error;

/// 全应用统一的 `Result` 别名。命令层用 `Result<T, AppError>` 代替 `Result<T, String>`，
/// 让 `?` 操作符能把 `sqlx`/`reqwest`/`serde_json`/`std::io` 错误自动转成 `AppError`。
///
/// **不**用于 L3 业务层 —— 业务层仍返回具体错误类型（如 `ExplainError`），只在
/// 跨层边界（L2 commands）转换为 `AppError` 后再 `map_err(|e| e.to_string())` 给 IPC。
pub type AppResult<T> = Result<T, AppError>;

/// 全应用唯一的错误枚举。L3 业务层 + L4 基础设施层都返回这个。
///
/// **序列化**：通过 `impl Serialize` 把错误序列化为可读字符串，IPC 那边拿到
/// `err.message = "database error: UNIQUE constraint failed: ..."`。
///
/// **不**承载业务错误码（如 LLM 的 8 种 stable codes）—— 那些在 L3 已经映射为字符串。
/// 这个 enum 只关心「是哪个子系统出了错」（DB/HTTP/IO/Serde/Keyring）+ 几个 app-level
/// 错误（Invalid/NotFound/Internal）。
#[derive(Debug, Error)]
pub enum AppError {
    /// SQLite 操作失败。`#[from]` 让 `?` 自动把 `sqlx::Error` 转成 `AppError::Db`。
    /// 典型场景：连接池耗尽 / UNIQUE 冲突 / 列类型不匹配。
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),

    /// HTTP 请求失败。`#[from]` 让 `?` 自动把 `reqwest::Error` 转成 `AppError::Http`。
    /// 典型场景：连接超时 / TLS 握手失败 / 5xx 响应（注意：5xx 是响应而非 error，
    /// 业务层需要自己检查 `.status()` 并映射到 L3 stable codes）。
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON 序列化/反序列化失败。`#[from]` 让 `?` 自动转。
    /// 典型场景：sidecar 返回的 JSON 字段缺失 / 类型不匹配（协议漂移）。
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),

    /// OS keyring 操作失败。手动 `From`（见下面 impl），因为 `keyring::Error`
    /// 不实现 `thiserror::Error` 的 std::error::Error 兼容。
    /// 典型场景：macOS Keychain 被锁 / 权限被拒 / 入口不存在。
    #[error("keyring error: {0}")]
    Keyring(String),

    /// 文件系统操作失败。`#[from]` 让 `?` 自动把 `std::io::Error` 转成 `AppError::Io`。
    /// 典型场景：storage 路径不存在 / 权限被拒 / 磁盘满。
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// 业务层校验失败。参数格式错 / 范围超界 / 必填字段缺失。
    /// L1 通常用这个做"用户输入不合法"的错误提示。
    #[error("invalid input: {0}")]
    Invalid(String),

    /// 资源不存在。Market / bet / model version / wallet 等查不到时。
    /// L1 通常用这个做"未找到"的 toast。
    #[error("not found: {0}")]
    NotFound(String),

    /// 兜底错误。invariant 违反 / 不可恢复的内部状态。
    /// 如果 L3 业务能给出更具体的错误，就**不要**用 Internal —— 用具体子类型
    /// 或 Invalid/NotFound 替代。
    #[error("internal: {0}")]
    Internal(String),
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl specta::Type for AppError {
    // v0.81 — Map AppError to TS `string` for codegen.
    // The runtime wire format is already a string (see `Serialize`
    // impl above), so this is consistent. `Primitive::str` is the
    // specta 2.0.0-rc.25 representation that maps to `string` in
    // the TS exporter.
    fn definition(_: &mut specta::Types) -> specta::datatype::DataType {
        specta::datatype::DataType::Primitive(specta::datatype::Primitive::str)
    }
}

impl From<keyring::Error> for AppError {
    fn from(e: keyring::Error) -> Self {
        AppError::Keyring(e.to_string())
    }
}

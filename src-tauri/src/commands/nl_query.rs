//! L2 —— 自然语言查询命令（P1-2）。
//!
//! IPC:nl_query —— 将一段自然语言字符串转换为安全的 SQL SELECT，
//! 对 markets 表执行，并附带人类可读的解释一并返回。

use crate::AppResult;
use crate::domain::nl_query::{build_sql_from_keywords, is_safe_sql, NlQueryRow};
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::FromRow;
use tauri::State;

/// 单条 NL 查询结果行 DTO —— 镜像 `NlQueryRow`。
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, Type)]
pub struct NlQueryRowDto {
    pub market_id: String,
    pub question: String,
    pub yes_price: Option<f64>,
    pub model_prob: Option<f64>,
    pub edge: Option<f64>,
    pub category: String,
}

impl From<NlQueryRowDto> for NlQueryRow {
    fn from(dto: NlQueryRowDto) -> Self {
        NlQueryRow {
            market_id: dto.market_id,
            question: dto.question,
            yes_price: dto.yes_price,
            model_prob: dto.model_prob,
            edge: dto.edge,
            category: dto.category,
        }
    }
}

/// 完整 NL 查询结果 DTO —— 镜像 `NlQueryResult`。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NlQueryResultDto {
    pub sql: String,
    pub results: Vec<NlQueryRowDto>,
    pub explanation: String,
}

/// IPC:nl_query —— 针对 markets 表执行自然语言查询。
///
/// 通过 `build_sql_from_keywords` 生成 SQL，使用 `is_safe_sql` 校验其安全性，然后执行。
/// 返回结果行以及生成的 SQL 和说明文字。
#[tauri::command]
pub async fn nl_query(
    state: State<'_, AppState>,
    query: String,
) -> AppResult<NlQueryResultDto> {
    let (sql, explanation) = build_sql_from_keywords(&query);

    if !is_safe_sql(&sql) {
        return Ok(NlQueryResultDto {
            sql: String::new(),
            results: Vec::new(),
            explanation: "Query rejected: generated SQL failed safety validation.".into(),
        });
    }

    let rows: Vec<NlQueryRowDto> = sqlx::query_as(&sql)
        .fetch_all(&state.db)
        .await?;

    Ok(NlQueryResultDto {
        sql,
        results: rows,
        explanation,
    })
}

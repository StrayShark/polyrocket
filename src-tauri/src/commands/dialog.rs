// v0.54a —— tauri-plugin-dialog 封装。
//
// 我们不直接把插件的命令暴露给 L1;而是把它们包装在
// 一个轻量的 Rust 端外观中,目的:
//   1. 设置合理的默认标题。
//   2. 把插件返回的 `Option<FilePath>` 扁平化为
//      `String | null`,L1 不用跨 IPC 边界翻译 `FilePath`。
//   3. `pick_file` 返回 `serde_json::Value`,
//      让同一 IPC 同时处理单文件与多文件模式
//      （L1 根据 `multiple` 决定）。
//
// 所有函数都是 `#[tauri::command]`。它们显式接收
// AppHandle,以便 dialog 插件可以查找父窗口。

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::infra::error::{AppError, AppResult};

/// v0.54a —— 打开原生目录选择器。默认情况下用户
/// 看到的是 home 目录。返回选中路径的 `String`;
/// 用户取消时返回 `None`。
#[tauri::command]
pub async fn pick_directory(app: AppHandle) -> AppResult<Option<String>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("polyrocket — choose storage location")
        .pick_folder(move |result| {
            let _ = tx.send(result);
        });
    let result = rx
        .await
        .map_err(|e| AppError::Internal(format!("dialog channel closed: {e}")))?;
    Ok(result.and_then(|fp| match fp {
        FilePath::Path(p) => Some(p.to_string_lossy().to_string()),
        FilePath::Url(u) => Some(u.to_string()),
    }))
}

#[derive(serde::Deserialize, Debug, Clone, Default)]
pub struct PickFileArgs {
    #[serde(default)]
    pub filters: Vec<DialogFileFilter>,
    #[serde(default)]
    pub multiple: bool,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct DialogFileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

/// v0.54a —— 打开原生文件选择器。返回选中路径的
/// `String`（单选模式）或 `Vec<String>`（多选模式）;
/// 取消时返回 `None`。JS 端的实际形态为
/// `string | null | string[]` —— L1 包装根据 `multiple`
/// 选择对应类型。
///
/// 这里返回 `serde_json::Value`,因为其形态是
/// sum type,而 `tauri::command` 的返回类型必须是具体类型
/// —— `Option<String>` 无法表达多选场景。
#[tauri::command]
pub async fn pick_file(
    app: AppHandle,
    args: PickFileArgs,
) -> AppResult<serde_json::Value> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let mut builder = app.dialog().file();
    builder = builder.set_title("polyrocket — choose a file");
    // OS 对话框中的第一个 filter 是「全部文件」兜底;
    // 之后的 filter 来自 L1 传入。
    builder = builder.add_filter("any", &["*"]);
    for f in &args.filters {
        let ext_refs: Vec<&str> = f.extensions.iter().map(|s| s.as_str()).collect();
        builder = builder.add_filter(f.name.clone(), &ext_refs);
    }
    if args.multiple {
        builder.pick_files(move |result| {
            let _ = tx.send(result);
        });
    } else {
        builder.pick_file(move |result| {
            let _ = tx.send(result.map(|fp| vec![fp]));
        });
    }
    let result = rx
        .await
        .map_err(|e| AppError::Internal(format!("dialog channel closed: {e}")))?;
    let files = match result {
        Some(vec) => vec,
        None => vec![],
    };
    let paths: Vec<String> = files
        .into_iter()
        .map(|fp| match fp {
            FilePath::Path(p) => p.to_string_lossy().to_string(),
            FilePath::Url(u) => u.to_string(),
        })
        .collect();
    Ok(if args.multiple {
        serde_json::to_value(&paths).unwrap_or(serde_json::Value::Null)
    } else {
        match paths.first() {
            Some(p) => serde_json::Value::String(p.clone()),
            None => serde_json::Value::Null,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_file_args_default_is_empty() {
        let a = PickFileArgs::default();
        assert!(a.filters.is_empty());
        assert!(!a.multiple);
    }

    #[test]
    fn dialog_file_filter_deserializes() {
        let json = r#"{"name":"JSON","extensions":["json"]}"#;
        let f: DialogFileFilter = serde_json::from_str(json).unwrap();
        assert_eq!(f.name, "JSON");
        assert_eq!(f.extensions, vec!["json".to_string()]);
    }

    #[test]
    fn pick_file_args_deserializes_with_filters() {
        let json = r#"{"filters":[{"name":"Key files","extensions":["env","key"]}],"multiple":true}"#;
        let a: PickFileArgs = serde_json::from_str(json).unwrap();
        assert!(a.multiple);
        assert_eq!(a.filters.len(), 1);
        assert_eq!(a.filters[0].extensions, vec!["env", "key"]);
    }
}

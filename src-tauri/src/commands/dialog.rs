// v0.54a — tauri-plugin-dialog wrappers.
//
// We don't expose the plugin's commands directly to the
// L1; instead, we wrap them in a tiny Rust-side facade that:
//   1. Sets a sensible default title.
//   2. Maps the plugin's `Option<FilePath>` return into
//      a flat `String | null` so the L1 doesn't have to
//      translate `FilePath` across the IPC boundary.
//   3. Returns `serde_json::Value` from `pick_file` so
//      the same IPC handles both single-file and
//      multi-file modes (L1 picks one based on
//      `multiple`).
//
// All functions are `#[tauri::command]`. They take
// AppHandle explicitly so the dialog plugin can look
// up the parent window.

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::infra::error::{AppError, AppResult};

/// v0.54a — open a native directory picker. The user
/// sees their home directory by default. Returns the
/// picked path as a `String`, or `None` if the user
/// cancelled.
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

/// v0.54a — open a native file picker. Returns the
/// picked path as a `String` (single mode) or
/// `Vec<String>` (multi mode), or `None` if cancelled.
/// The exact JS shape is `string | null | string[]` —
/// the L1 wrapper picks one based on `multiple`.
///
/// We return `serde_json::Value` because the shape
/// is a sum type and `tauri::command` return types
/// must be concrete — `Option<String>` would lose
/// the multi case.
#[tauri::command]
pub async fn pick_file(
    app: AppHandle,
    args: PickFileArgs,
) -> AppResult<serde_json::Value> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let mut builder = app.dialog().file();
    builder = builder.set_title("polyrocket — choose a file");
    // First filter slot in the OS dialog is the
    // "any file" catch-all; subsequent slots are
    // the L1-supplied filters.
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

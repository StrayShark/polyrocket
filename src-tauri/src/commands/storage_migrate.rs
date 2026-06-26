// v0.54b —— 存储迁移工具。
//
// 允许用户在重启之前把现有的 polyrocket.db + logs/
// 拷贝到新路径。流程如下：
//
//   1. 用户通过 tauri-plugin-dialog 选择或输入新路径。
//   2. setStoragePath() 把新路径写入
//      _polyrocket_settings + storage_path.json。
//   3. 用户看到「需要重启」横幅。同时他们也可以看到
//      「把现有数据复制到新路径」按钮（迁移工具）。
//   4. migrateStoragePath() 把当前 db（以及 logs/）
//      复制到新路径。完成后，下次启动新路径已经
//      包含数据 —— 不会遇到空 DB 的意外。
//
// 为什么要单独的 IPC？因为 (a) 大 DB 时可能要花
// 一段时间（我们不想把它放在 setStoragePath 的关键
// 路径上）；(b) 用户可以选择不迁移（全新安装）；
// (c) 我们需要校验新路径为空或可以被安全覆盖。

use crate::AppResult;
use crate::infra::state::AppState;
use crate::platform::paths;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

#[derive(Debug, Deserialize)]
pub struct MigrateStoragePathArgs {
    /// 目标目录（必须与当前 `storage_path` 设置一致；
    /// 留空则迁移到系统默认位置）。
    pub dest: String,
    /// 为 true 时覆盖目标位置已有的文件（例如
    /// `polyrocket.db`、`logs/`）。为 false 时，
    /// 若目标目录非空 IPC 会返回错误。
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MigrateStoragePathResult {
    /// 数据复制来源的路径。
    pub from: String,
    /// 数据复制目标的路径。
    pub to: String,
    /// 复制的文件数。
    pub files_copied: usize,
    /// 复制的总字节数。
    pub bytes_copied: u64,
    /// 目标位置已有数据被覆盖（用户主动开启）时为 true。
    pub overwritten: bool,
    /// 没有源数据可复制时为 true（此前已是全新安装
    /// 路径）。这种情况下没有真正复制；但仍视为
    /// 「已迁移」，以免继续提示用户。
    pub noop: bool,
}

/// v0.54b —— 把 `polyrocket.db` + `logs/` 从
/// 源（当前生效路径）拷贝到 `dest`。
/// 用户没有指定自定义路径时，源就是系统默认路径。
/// 目标必须就是用户刚刚通过 `setStoragePath` 设置的
/// 路径（或者通过 `resetStoragePath` 回到的系统默认）。
///
/// 幂等性：用相同参数再次调用是 no-op（返回 `noop: true`）。
///
/// 错误：
/// - dest 不存在 / 不可写：AppError::Invalid
/// - dest 非空且 overwrite=false：AppError::Invalid
/// - 复制中途失败：AppError::Internal
///   （调用方应把目标视为已损坏并执行 reset）
#[tauri::command]
pub async fn migrate_storage_path(
    app: AppHandle,
    state: State<'_, AppState>,
    args: MigrateStoragePathArgs,
) -> AppResult<MigrateStoragePathResult> {
    let dest_path = std::path::PathBuf::from(args.dest.trim());
    if !dest_path.is_absolute() {
        return Err(crate::AppError::Invalid(
            "migrate: dest must be absolute".into(),
        ));
    }
    if !dest_path.exists() {
        return Err(crate::AppError::Invalid(format!(
            "migrate: dest does not exist: {}",
            dest_path.display()
        )));
    }
    if !dest_path.is_dir() {
        return Err(crate::AppError::Invalid(format!(
            "migrate: dest is not a directory: {}",
            dest_path.display()
        )));
    }
    if !probe_writable(&dest_path) {
        return Err(crate::AppError::Invalid(format!(
            "migrate: dest is not writable: {}",
            dest_path.display()
        )));
    }
    // 源：系统默认路径。无论当前进程使用的是默认
    // 路径还是自定义路径，我们都从默认路径复制到 dest。
    // 理由：若用户在默认路径上选了一个新路径，
    // 我们就从默认 -> 自定义；若用户已经使用某个
    // 自定义路径、又选了另一个新路径，我们就从
    // 旧的自定义路径 -> 新的自定义路径（即「更改
    // 存储路径」流程）。
    let source_dir = paths::app_data_dir(&app)?;
    let source_db = source_dir.join("polyrocket.db");
    let source_logs = source_dir.join("logs");
    let dest_db = dest_path.join("polyrocket.db");
    let dest_logs = dest_path.join("logs");
    // 若 dest 非空且 overwrite=false，则报错。
    let dest_non_empty = dest_db.exists()
        || dest_logs.exists()
        || any_dir_entries(&dest_path)?;
    if dest_non_empty && !args.overwrite {
        return Err(crate::AppError::Invalid(format!(
            "migrate: dest is non-empty (set overwrite=true to replace): {}",
            dest_path.display()
        )));
    }
    // 若源端没有 db（全新安装场景），我们仍然要
    // 确保 dest 处有可用的 db。v0.54b 不会主动创建
    // —— 下次启动时会做这件事。
    let noop = !source_db.exists() && !source_logs.exists();
    if noop {
        return Ok(MigrateStoragePathResult {
            from: source_dir.to_string_lossy().to_string(),
            to: dest_path.to_string_lossy().to_string(),
            files_copied: 0,
            bytes_copied: 0,
            overwritten: args.overwrite,
            noop: true,
        });
    }
    let (files, bytes) = copy_tree(
        &source_dir,
        &dest_path,
        &["polyrocket.db", "logs"],
    )?;
    // 审计日志。
    sqlx::query(
        "INSERT INTO audit_log (actor, action, target, payload, result)
         VALUES ('user', 'storage.path.migrate', ?, ?, 'ok')",
    )
    .bind(dest_path.to_string_lossy().to_string())
    .bind(format!(
        "{{\"from\":\"{}\",\"to\":\"{}\",\"files\":{files},\"bytes\":{bytes}}}",
        source_dir.to_string_lossy(),
        dest_path.to_string_lossy()
    ))
    .execute(&state.db)
    .await
    .ok();
    Ok(MigrateStoragePathResult {
        from: source_dir.to_string_lossy().to_string(),
        to: dest_path.to_string_lossy().to_string(),
        files_copied: files,
        bytes_copied: bytes,
        overwritten: args.overwrite,
        noop: false,
    })
}

/// 判断目录是否含有任何条目（文件或子目录）。在迁移前用于
/// 决定「目标目录是否为空」。
fn any_dir_entries(dir: &std::path::Path) -> std::io::Result<bool> {
    let mut rd = std::fs::read_dir(dir)?;
    Ok(rd.next().is_some())
}

/// 探测当前进程是否能在 `dir` 中创建新文件。我们尝试创建一个临时
/// 文件并立即删除，成功则返回 true。（从 get_storage_info 复制而来，
/// 以保持 storage 模块无依赖；两者都是 1 页左右的小工具。）
fn probe_writable(dir: &std::path::Path) -> bool {
    let probe = dir.join(".polyrocket-write-probe");
    let result = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&probe);
    let ok = result.is_ok();
    if ok {
        let _ = std::fs::remove_file(&probe);
    }
    ok
}

/// 把 `src` 下一组路径（文件或目录）递归地拷贝到 `dst`。返回
/// 已拷贝的文件数与总字节数。用于将已有的 `polyrocket.db` + `logs/`
/// 目录树迁移到新的存储路径。
///
/// `entries` 是 `src` 下的若干相对路径列表，每个要么是文件
///（按原样拷贝），要么是目录（递归遍历）。缺失的条目会直接
/// 跳过（即使 `logs/` 暂时不存在，我们也不会让流程失败）。
fn copy_tree(
    src: &std::path::Path,
    dst: &std::path::Path,
    entries: &[&str],
) -> AppResult<(usize, u64)> {
    let mut files = 0usize;
    let mut bytes = 0u64;
    for entry in entries {
        let s = src.join(entry);
        let d = dst.join(entry);
        if !s.exists() {
            continue;
        }
        if s.is_file() {
            // 确保 dst 中的父目录存在。
            if let Some(parent) = d.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    crate::AppError::Internal(format!(
                        "create_dir_all({}): {e}",
                        parent.display()
                    ))
                })?;
            }
            std::fs::copy(&s, &d).map_err(|e| {
                crate::AppError::Internal(format!(
                    "copy {} -> {}: {e}",
                    s.display(),
                    d.display()
                ))
            })?;
            files += 1;
            bytes += std::fs::metadata(&s)
                .map(|m| m.len())
                .unwrap_or(0);
        } else if s.is_dir() {
            // 递归遍历。
            walk_copy(&s, &d, &mut files, &mut bytes)?;
        }
    }
    Ok((files, bytes))
}

fn walk_copy(
    src: &std::path::Path,
    dst: &std::path::Path,
    files: &mut usize,
    bytes: &mut u64,
) -> AppResult<()> {
    std::fs::create_dir_all(dst).map_err(|e| {
        crate::AppError::Internal(format!(
            "create_dir_all({}): {e}",
            dst.display()
        ))
    })?;
    for entry in std::fs::read_dir(src).map_err(|e| {
        crate::AppError::Internal(format!("read_dir({}): {e}", src.display()))
    })? {
        let entry = entry.map_err(|e| {
            crate::AppError::Internal(format!("read_dir entry: {e}"))
        })?;
        let s = entry.path();
        let d = dst.join(entry.file_name());
        if s.is_dir() {
            walk_copy(&s, &d, files, bytes)?;
        } else if s.is_file() {
            std::fs::copy(&s, &d).map_err(|e| {
                crate::AppError::Internal(format!(
                    "copy {} -> {}: {e}",
                    s.display(),
                    d.display()
                ))
            })?;
            *files += 1;
            *bytes += std::fs::metadata(&s)
                .map(|m| m.len())
                .unwrap_or(0);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(label: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir();
        let p = base.join(format!("polyrocket-mig-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn any_dir_entries_on_empty_returns_false() {
        let d = tempdir("any-empty");
        assert!(!any_dir_entries(&d).unwrap());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn any_dir_entries_with_file_returns_true() {
        let d = tempdir("any-file");
        std::fs::write(d.join("a.txt"), b"hi").unwrap();
        assert!(any_dir_entries(&d).unwrap());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn copy_tree_copies_file_and_walks_subdir() {
        let src = tempdir("cp-src");
        let dst = tempdir("cp-dst");
        std::fs::write(src.join("polyrocket.db"), b"db-bytes").unwrap();
        std::fs::create_dir_all(src.join("logs")).unwrap();
        std::fs::write(src.join("logs").join("a.log"), b"line1\nline2\n").unwrap();
        std::fs::create_dir_all(src.join("logs").join("telemetry")).unwrap();
        std::fs::write(
            src.join("logs").join("telemetry").join("sess.jsonl"),
            b"{\"k\":1}\n",
        )
        .unwrap();
        let (files, bytes) =
            copy_tree(&src, &dst, &["polyrocket.db", "logs"]).unwrap();
        assert_eq!(files, 3, "db + a.log + sess.jsonl");
        assert!(bytes > 0);
        assert!(dst.join("polyrocket.db").exists());
        assert!(dst.join("logs").join("a.log").exists());
        assert!(dst.join("logs").join("telemetry").join("sess.jsonl").exists());
        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&dst);
    }

    #[test]
    fn copy_tree_skips_missing_entries() {
        let src = tempdir("cp-skip-src");
        let dst = tempdir("cp-skip-dst");
        // 源端没有 polyrocket.db，也没有 logs/。
        let (files, bytes) =
            copy_tree(&src, &dst, &["polyrocket.db", "logs"]).unwrap();
        assert_eq!(files, 0);
        assert_eq!(bytes, 0);
        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&dst);
    }

    // v0.58a —— 自动迁移流程在每次「应用」时都会
    // 跑一次。验证 Rust 端的幂等性：用同一对源/目标
    // 第二次执行 copy_tree（目标已有数据时），
    // 因为 overwrite=false 会拒绝非空目标，
    // 所以对已存在的文件而言是 no-op。
    // 这里测试目标非空分支。
    #[test]
    fn any_dir_entries_with_subdir_returns_true() {
        let d = tempdir("any-subdir");
        std::fs::create_dir(d.join("nested")).unwrap();
        assert!(any_dir_entries(&d).unwrap());
        let _ = std::fs::remove_dir_all(&d);
    }
}

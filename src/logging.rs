//! Logging: tracing + tracing-appender, daily rotation, keep 7 days.
//! Logs: Windows %LOCALAPPDATA%/RarZap/logs, macOS ~/Library/Logs/RarZap, Linux ~/.local/share/RarZap/logs
//!
//! 安全要点（审计意见 #8）：
//! - 目录 Unix 下 0700（仅属主可进目录，内层文件即使 0644 外人也不可达）；
//!   启动时把存量日志 chmod 0600（纵深，新文件由 umask 决定，但目录已隔离）。
//! - Windows 依赖每用户配置目录的 ACL（其它非管理员用户默认不可读）；不额外设 ACL，
//!   避免与企业漫游配置冲突。
//! - 日志禁止记敏感信息：激活码原文、完整指纹、机器原始 ID、请求 body 均不得进日志。
//!   允许记：`fp_short`（8 位哈希前缀，难逆推）、是否授权布尔值、文件名（用户自己的路径）。
//! - 保留 7 天：启动时删除 mtime 超过 7 天的 `rarzap.log*`（tracing-appender 自身不删）。

use directories::ProjectDirs;
use std::path::{Path, PathBuf};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// 日志目录（隐私页“打开日志目录”用）。
pub fn log_dir() -> PathBuf {
    if let Some(dirs) = ProjectDirs::from("", "", "RarZap") {
        dirs.data_local_dir().join("logs")
    } else {
        PathBuf::from("logs")
    }
}

/// Initialize global tracing subscriber. Safe to call once at startup.
pub fn init_logging() {
    let dir = log_dir();
    // 日志可能含路径等信息，Unix 下收紧到 0700（仅属主可读）。
    // mode 只对新建生效，老版本遗留的 0755 必须显式 chmod。
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        let _ = builder.create(&dir);
        if let Ok(meta) = std::fs::metadata(&dir) {
            let mut perm = meta.permissions();
            if perm.mode() & 0o777 != 0o700 {
                perm.set_mode(0o700);
                let _ = std::fs::set_permissions(&dir, perm);
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = std::fs::create_dir_all(&dir);
    }
    // 存量维护：7 天以上轮转文件删除 + Unix 下 chmod 0600（best-effort）。
    prune_old_logs(&dir, 7);

    let file_appender = tracing_appender::rolling::daily(&dir, "rarzap.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    // Keep the guard alive for the whole process.
    std::mem::forget(guard);

    let file_layer = fmt::layer().with_writer(non_blocking).with_ansi(false);
    let stdout_layer = fmt::layer().with_writer(std::io::stdout);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stdout_layer)
        .try_init();

    tracing::info!(log_dir = %dir.display(), "logging initialized");
}

/// 启动时维护：删除超过 `keep_days` 天的 `rarzap.log*`，Unix 下把存量日志 chmod 0600。
/// 全部 best-effort（失败忽略，不影响启动）。
fn prune_old_logs(dir: &Path, keep_days: u64) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let now = std::time::SystemTime::now();
    let keep_secs = keep_days.saturating_mul(24 * 3600);
    for entry in rd.flatten() {
        let name = entry.file_name();
        let s = name.to_string_lossy();
        if !s.starts_with("rarzap.log") {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        // Unix 纵深：存量文件收紧到 0600（目录 0700 已隔离，这里是第二层）。
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = meta.permissions();
            if perm.mode() & 0o777 != 0o600 {
                perm.set_mode(0o600);
                let _ = std::fs::set_permissions(entry.path(), perm);
            }
        }
        let old = meta
            .modified()
            .ok()
            .and_then(|t| now.duration_since(t).ok())
            .map(|d| d.as_secs() > keep_secs)
            .unwrap_or(false);
        if old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

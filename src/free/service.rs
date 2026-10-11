//! 免费版单文件转换服务：校验 -> 规划输出路径 -> 调用 CORE 执行。
//! 只有“一次转一个文件”的能力：无队列、无并发、无批量。

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::core::{ConvertProgress, ProgressSink};

/// 单文件转换请求。
#[derive(Debug, Clone)]
pub struct SingleRequest {
    pub rar_path: PathBuf,
    pub output_dir: Option<PathBuf>,
}

impl SingleRequest {
    pub fn new(rar_path: PathBuf, output_dir: Option<PathBuf>) -> Self {
        Self {
            rar_path,
            output_dir,
        }
    }
}

/// 校验输入并算出输出 ZIP 路径（不触碰磁盘）。
pub fn plan_single(req: &SingleRequest) -> Result<PathBuf> {
    if !is_rar_path(&req.rar_path) {
        anyhow::bail!("not a .rar file: {}", req.rar_path.display());
    }
    Ok(crate::core::default_zip_name_for(
        &req.rar_path,
        req.output_dir.as_deref(),
    ))
}

/// 执行单文件转换（无进度），返回产物路径。
pub fn run_single(
    req: &SingleRequest,
    is_cancelled: Option<&dyn Fn() -> bool>,
) -> Result<PathBuf> {
    run_single_with_progress(req, is_cancelled, &mut |_| {})
}

/// 执行单文件转换（带进度），返回产物路径。
/// 不覆盖策略：若规划路径已存在，自动顺延为 `名 (2).zip`（浏览器式改名），
/// 避免静默覆盖用户已有文件。返回的是实际落盘路径（可能与 [`plan_single`] 不同）。
pub fn run_single_with_progress(
    req: &SingleRequest,
    is_cancelled: Option<&dyn Fn() -> bool>,
    on_progress: ProgressSink<'_>,
) -> Result<PathBuf> {
    let wanted = plan_single(req)?;
    // best-effort 不冲突路径；最终落盘前 CORE 还有二次 exists 检查（关住竞态）。
    let zip = crate::core::unique_zip_path(&wanted);
    crate::core::convert_rar_to_zip_with_progress(&req.rar_path, &zip, is_cancelled, on_progress)
        .with_context(|| format!("convert failed: {}", req.rar_path.display()))?;
    Ok(zip)
}

/// picking / 拖放阶段用的轻量 .rar 后缀判断（文件可能尚不存在）。
pub fn is_rar_path(p: &Path) -> bool {
    p.extension()
        .map(|e| e.to_string_lossy().eq_ignore_ascii_case("rar"))
        .unwrap_or(false)
}

/// 从一组拖入路径里挑出第一个 .rar（免费版只转一个）。
pub fn first_rar(paths: &[PathBuf]) -> Option<PathBuf> {
    paths.iter().find(|p| is_rar_path(p)).cloned()
}

/// 让 UI 层复用的进度类型别名。
pub type Progress = ConvertProgress;

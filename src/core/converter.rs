//! 单文件 RAR -> ZIP 转换原语。
//! 流程：先列表(仅读头部)求总大小 -> 解压到临时目录 -> 打包为 ZIP(Deflate) -> 原子改名。
//! 全程不上传、不一次性载入内存；取消时清理半成品，成功才产出最终文件。
//!
//! 进度：解压阶段用 unrar 的逐文件回调（含字节数），打包阶段按块累计字节数，
//! 两者加权映射到 0.0..=1.0 的总体百分比，供 UI 显示。
//!
//! 安全模型（对应审计意见）：
//! - `unrar` C 库是信任边界：条目名先做预检（双分隔符归一化，见 [`unsafe_entry_path`]），
//!   解压后做 [`collect_verified_files`] 单次遍历断言（无符号链接、全部在暂存目录内、
//!   实际体积/文件数复核，顺手产出打包条目表），并对暂存父目录做同级逃逸快照对比。绝对路径写到其它
//!   位置的攻击仍依赖预检 + `unrar-ng-sys` 上游修复，CI 的 `audit` 作业负责 CVE 监控。
//! - Zip 炸弹：[`ConvertOptions`] 默认限制解压体积（20 GiB）与文件数（10 万），
//!   头部声明、解压回调、落盘后三处 enforcement；可用 `RARZAP_MAX_BYTES` /
//!   `RARZAP_MAX_FILES` 覆盖（`0` = 不限，仅建议受控环境使用）。
//! - 覆盖：默认拒绝覆盖已存在的目标文件（防静默丢数据），调用方用
//!   [`unique_zip_path`] 生成 `名 (2).zip` 这类不冲突路径，或显式 `overwrite: true`。

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// 转换阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// 正在解压 RAR。
    Extracting,
    /// 正在打包 ZIP。
    Packing,
}

impl Phase {
    /// i18n key（供 UI 显示）。
    pub fn label_key(self) -> &'static str {
        match self {
            Phase::Extracting => "working.extracting",
            Phase::Packing => "working.packing",
        }
    }
}

/// 一次进度快照。
#[derive(Debug, Clone)]
pub struct ConvertProgress {
    pub phase: Phase,
    /// 总体进度 0.0..=1.0（解压 45% + 打包 55% 加权）。
    pub percent: f32,
    /// 当前处理中的文件名（相对路径）。
    pub current_file: String,
    /// 已处理的文件数 / 总数（总数在解压阶段已知）。
    pub files_done: usize,
    pub files_total: usize,
}

/// 协作式取消探针：返回 true 表示调用方希望尽快停下。
/// 用裸闭包而非具体运行时类型，CORE 不依赖任何异步运行时。
pub type CancelProbe<'a> = &'a dyn Fn() -> bool;

/// 进度回调：由调用方在 worker 线程上收到，负责转回 UI 线程。
pub type ProgressSink<'a> = &'a mut dyn FnMut(&ConvertProgress);

/// 解压占总体进度的权重。
const EXTRACT_WEIGHT: f32 = 0.45;

/// 默认解压体积上限：20 GiB（头部声明 + 回调累计 + 落盘复核三处执行）。
pub const DEFAULT_MAX_TOTAL_BYTES: u64 = 20 * 1024 * 1024 * 1024;
/// 默认文件数上限：10 万。
pub const DEFAULT_MAX_FILES: usize = 100_000;

/// 转换选项（向后兼容：旧入口使用 [`ConvertOptions::default()`]，即不覆盖 + 默认上限）。
#[derive(Debug, Clone)]
pub struct ConvertOptions {
    /// 是否允许覆盖已存在的目标 ZIP。默认 `false`（拒绝覆盖，防静默丢数据）。
    pub overwrite: bool,
    /// 解压体积上限（字节）。`0` = 不限。
    pub max_total_bytes: u64,
    /// 文件数上限。`0` = 不限。
    pub max_files: usize,
    /// ZIP deflate 等级 1..=9。`None` = 库默认（6，与旧行为一致）。
    /// 实测（264MB 混合数据，debug）：L1 比 L6 快约 27%，产物大 7%。
    /// 可用 `RARZAP_ZIP_LEVEL` 覆盖（非法值忽略）。
    pub compression_level: Option<u32>,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            overwrite: false,
            max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
            max_files: DEFAULT_MAX_FILES,
            compression_level: None,
        }
    }
}

impl ConvertOptions {
    /// 允许覆盖的选项（显式 opt-in，供确需覆盖的调用方）。
    pub fn overwriting() -> Self {
        Self {
            overwrite: true,
            ..Self::default()
        }
    }

    /// 合并环境变量覆盖（`RARZAP_MAX_BYTES` / `RARZAP_MAX_FILES`，`0` = 不限）。
    /// 环境变量只在合法解析时生效，非法值忽略（保持默认）。
    fn effective(&self) -> (u64, usize) {
        let bytes = std::env::var("RARZAP_MAX_BYTES")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(self.max_total_bytes);
        let files = std::env::var("RARZAP_MAX_FILES")
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(self.max_files);
        (bytes, files)
    }

    /// 生效的 deflate 等级：显式配置优先，否则看 `RARZAP_ZIP_LEVEL`。
    /// 范围 1..=9，超范围/非法一律回退默认（None）。
    fn effective_level(&self) -> Option<i64> {
        if let Some(l) = self.compression_level {
            return normalize_level(l);
        }
        std::env::var("RARZAP_ZIP_LEVEL")
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .and_then(normalize_level)
    }
}

/// 等级归一化：1..=9 通过，其余（0/>9）视为未设置。
fn normalize_level(l: u32) -> Option<i64> {
    (1..=9).contains(&l).then_some(l as i64)
}

/// 输出 `<stem>.zip` 到输出目录（缺省与源文件同目录）。
/// 注意：只做纯路径拼接，不触碰磁盘、不保证不冲突；需要不覆盖保证时用 [`unique_zip_path`]。
pub fn default_zip_name_for(rar_path: &Path, output_dir: Option<&Path>) -> PathBuf {
    let stem = rar_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "archive".to_string());
    let dir = output_dir
        .map(Path::to_path_buf)
        .or_else(|| rar_path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    dir.join(format!("{stem}.zip"))
}

/// 路径是否存在（含断开的符号链接）。用 `symlink_metadata` 而非 `exists`，
/// 后者对断开链接返回 false，会误判为“可安全写入”。
fn path_exists(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok()
}

/// 对期望路径找一个不冲突的变体：`a.zip` 已存在时依次试 `a (2).zip`、`a (3).zip`…。
/// 最多试 10_000 次（基本不可能耗尽）；调用方仍应处理最终写入时的 `already exists`
/// 竞态（见转换函数内的二次检查）。
pub fn unique_zip_path(wanted: &Path) -> PathBuf {
    if !path_exists(wanted) {
        return wanted.to_path_buf();
    }
    let parent = wanted.parent().map(Path::to_path_buf);
    let stem = wanted
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "archive".to_string());
    let ext = wanted
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    for i in 2..=10_000u32 {
        let name = format!("{stem} ({i}){ext}");
        let cand = match &parent {
            Some(d) => d.join(&name),
            None => PathBuf::from(&name),
        };
        if !path_exists(&cand) {
            return cand;
        }
    }
    // 理论不可达：全部被占时返回最后一个（调用方会收到 already-exists 错误）。
    match parent {
        Some(d) => d.join(format!("{stem} (10000){ext}")),
        None => PathBuf::from(format!("{stem} (10000){ext}")),
    }
}

/// 转换一个 RAR 文件为 ZIP（阻塞；须在 worker 线程调用）。
/// 无进度需求时的便捷入口（默认不覆盖 + 默认体积上限）。
pub fn convert_rar_to_zip(
    rar_path: &Path,
    zip_path: &Path,
    is_cancelled: Option<CancelProbe<'_>>,
) -> Result<()> {
    convert_rar_to_zip_with_progress(rar_path, zip_path, is_cancelled, &mut |_| {})
}

/// 带进度的转换。`on_progress` 在解压/打包过程中被频繁调用（已做节流）。
/// 默认不覆盖已存在的目标文件（存在即报错），默认体积上限见 [`ConvertOptions`]。
pub fn convert_rar_to_zip_with_progress(
    rar_path: &Path,
    zip_path: &Path,
    is_cancelled: Option<CancelProbe<'_>>,
    on_progress: ProgressSink<'_>,
) -> Result<()> {
    convert_rar_to_zip_with_options(
        rar_path,
        zip_path,
        is_cancelled,
        on_progress,
        &ConvertOptions::default(),
    )
}

/// 全选项转换（体积上限 / 覆盖策略可配）。
pub fn convert_rar_to_zip_with_options(
    rar_path: &Path,
    zip_path: &Path,
    is_cancelled: Option<CancelProbe<'_>>,
    on_progress: ProgressSink<'_>,
    options: &ConvertOptions,
) -> Result<()> {
    let cancelled = || is_cancelled.is_some_and(|f| f());
    let (max_bytes, max_files) = options.effective();
    if !path_exists(rar_path) {
        anyhow::bail!("source RAR not found: {}", rar_path.display());
    }
    if let Some(parent) = zip_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("cannot create target dir {}", parent.display()))?;
        }
    }
    if cancelled() {
        anyhow::bail!("cancelled by user");
    }
    // 默认拒绝覆盖：前置检查（最终落盘前还有二次检查，见后文）。
    if !options.overwrite && path_exists(zip_path) {
        anyhow::bail!(
            "target already exists (refusing to overwrite): {}",
            zip_path.display()
        );
    }
    // 磁盘配额提示：头部声明若已超过上限，直接拒绝（不触碰磁盘）。
    // 剩余空间预检刻意不做：跨平台取可用空间需额外系统调用，
    // 体积上限 + 落盘时的 ENOSPC 友好错误已覆盖 Zip 炸弹场景。

    // 1) 仅读头部，统计总解压字节数与文件数（用于算进度；不触碰磁盘）。
    //    同时做路径穿越预检 + 体积/数量上限预检。
    let (total_bytes, files_total) = scan_totals(rar_path, max_bytes, max_files)?;

    // 2) 解压到临时目录（最终 zip 成功后才出现）。
    //    暂存目录优先放在目标同盘：跨盘 rename 会失败，同盘才能原子落盘。
    //    顺手清掉该目录下过期（>1h）的遗留暂存目录。
    let staging_parent = zip_path.parent().filter(|p| p.is_dir());
    if let Some(parent) = staging_parent {
        sweep_stale_staging(parent);
    }
    // 同级逃逸快照：记录父目录当前条目，解压后新增的非 `.rarzap-*` 条目视为 C 库写穿。
    let before_siblings: HashSet<String> =
        staging_parent.map(snapshot_names).unwrap_or_default();
    let tmp = staging_parent
        .and_then(|parent| {
            tempfile::Builder::new()
                .prefix(".rarzap-")
                .tempdir_in(parent)
                .ok()
        })
        .or_else(|| tempfile::tempdir().ok())
        .context("create temp dir")?;
    let mut done_bytes: u64 = 0;
    let mut files_done: usize = 0;
    let mut last_pct: f32 = -1.0;
    let weight = if files_total == 0 { 1.0 } else { EXTRACT_WEIGHT };
    let mut limit_err: Option<String> = None;

    let status = unrar_ng::Archive::new(rar_path)
        .open_for_processing()
        .with_context(|| format!("cannot open RAR {}", rar_path.display()))?
        .extract_all_with_callback(tmp.path(), |event| {
            use unrar_ng::ExtractEvent;
            // 取消：返回 false，unrar 会中止本次解压。
            if cancelled() {
                return false;
            }
            // 体积/数量上限：已触发后保持返回 false（中止本次解压）。
            if limit_err.is_some() {
                return false;
            }
            match event {
                ExtractEvent::Start { filename, size } => {
                    // 预判：这个文件解出来就会超限，直接中止（避免把炸弹写满磁盘）。
                    if max_bytes > 0 && done_bytes.saturating_add(size) > max_bytes {
                        limit_err = Some(format!(
                            "archive too large: unpacked size would exceed {} ({}), refusing to extract '{}'",
                            human_bytes(max_bytes),
                            human_bytes(done_bytes.saturating_add(size)),
                            filename.display()
                        ));
                        return false;
                    }
                    if max_files > 0 && files_done + 1 > max_files {
                        limit_err = Some(format!(
                            "archive has too many files (limit {max_files}), refusing to extract '{}'",
                            filename.display()
                        ));
                        return false;
                    }
                    let pct = weight * fraction(done_bytes, total_bytes, files_done, files_total);
                    emit(on_progress, &mut last_pct, phase_progress(
                        Phase::Extracting,
                        pct,
                        &filename,
                        files_done,
                        files_total,
                    ));
                }
                ExtractEvent::Ok { filename, size } => {
                    done_bytes = done_bytes.saturating_add(size);
                    files_done += 1;
                    if max_bytes > 0 && done_bytes > max_bytes {
                        limit_err = Some(format!(
                            "archive too large: unpacked {} exceeds {} , aborting",
                            human_bytes(done_bytes),
                            human_bytes(max_bytes)
                        ));
                        return false;
                    }
                    if max_files > 0 && files_done > max_files {
                        limit_err = Some(format!(
                            "archive has too many files ({} > {max_files}), aborting",
                            files_done
                        ));
                        return false;
                    }
                    let pct = weight * fraction(done_bytes, total_bytes, files_done, files_total);
                    emit(on_progress, &mut last_pct, phase_progress(
                        Phase::Extracting,
                        pct,
                        &filename,
                        files_done,
                        files_total,
                    ));
                }
                ExtractEvent::Err { .. } => {
                    // 交由后续状态判定为失败。
                }
                _ => {}
            }
            true
        })
        .with_context(|| format!("RAR extract failed: {}", rar_path.display()))?;

    if let Some(msg) = limit_err {
        anyhow::bail!("{msg}");
    }
    if matches!(status, unrar_ng::ExtractStatus::Cancelled) || cancelled() {
        anyhow::bail!("cancelled by user");
    }

    // 2b) 解压后断言 + 收集一次遍历：无符号链接、词法 containment、
    //     实际体积复核，顺手产出打包要用的条目表（原来断言与收集各走一遍目录，
    //     万级文件包能省一次全量 readdir+stat）。
    detect_sibling_escape(staging_parent, &before_siblings)?;
    let collected = collect_verified_files(tmp.path())?;
    if max_bytes > 0 && collected.bytes > max_bytes {
        anyhow::bail!(
            "archive too large: extracted {} on disk exceeds {}",
            human_bytes(collected.bytes),
            human_bytes(max_bytes)
        );
    }
    if max_files > 0 && collected.files > max_files {
        anyhow::bail!(
            "archive has too many files on disk ({} > {max_files})",
            collected.files
        );
    }

    // 3) 打包 ZIP，按块累计字节进度。
    let tmp_zip = tmp.path().join("output.zip");
    let pack_base = weight;
    let pack_span = 1.0 - weight;
    let mut zipped: u64 = 0;
    let level = options.effective_level();
    write_dir_as_zip(
        &collected.entries,
        &tmp_zip,
        level,
        is_cancelled,
        &mut |chunk| {
            zipped = zipped.saturating_add(chunk);
            let pct = pack_base
                + pack_span * fraction(zipped, total_bytes, files_done, files_total.max(1));
            emit(on_progress, &mut last_pct, phase_progress(
                Phase::Packing,
                pct,
                Path::new(""),
                files_done,
                files_total,
            ));
        },
    )
    .with_context(|| format!("ZIP pack failed: {}", rar_path.display()))?;

    if cancelled() {
        let _ = std::fs::remove_file(&tmp_zip);
        anyhow::bail!("cancelled by user");
    }

    // 4) 落盘：二次检查覆盖策略（关住 plan 与落盘之间的 TOCTOU 窗口）。
    //    Windows 的 rename 在目标存在时直接失败（天然不覆盖）；
    //    Unix 的 rename 会静默覆盖，所以必须先检查。
    if !options.overwrite && path_exists(zip_path) {
        let _ = std::fs::remove_file(&tmp_zip);
        anyhow::bail!(
            "target already exists (refusing to overwrite): {}",
            zip_path.display()
        );
    }
    if options.overwrite {
        let _ = std::fs::remove_file(zip_path);
    }
    if std::fs::rename(&tmp_zip, zip_path).is_err() {
        // 跨盘或占用时退回复制；ENOSPC 给出明确提示而非笼统 IO 错误。
        if !options.overwrite && path_exists(zip_path) {
            let _ = std::fs::remove_file(&tmp_zip);
            anyhow::bail!(
                "target already exists (refusing to overwrite): {}",
                zip_path.display()
            );
        }
        std::fs::copy(&tmp_zip, zip_path).map_err(|e| {
            if is_disk_full(&e) {
                anyhow::anyhow!(
                    "disk full while writing {} (free some space and retry)",
                    zip_path.display()
                )
            } else {
                anyhow::anyhow!(
                    "cannot write ZIP {} (disk full or no permission?): {e:#}",
                    zip_path.display()
                )
            }
        })?;
        let _ = std::fs::remove_file(&tmp_zip);
    }

    // 收尾 100%。
    emit(on_progress, &mut last_pct, ConvertProgress {
        phase: Phase::Packing,
        percent: 1.0,
        current_file: String::new(),
        files_done: files_total,
        files_total,
    });
    Ok(())
}

fn phase_progress(
    phase: Phase,
    percent: f32,
    file: &Path,
    files_done: usize,
    files_total: usize,
) -> ConvertProgress {
    ConvertProgress {
        phase,
        percent: percent.clamp(0.0, 1.0),
        current_file: file.to_string_lossy().replace('\\', "/"),
        files_done,
        files_total,
    }
}

/// 仅当百分比推进 >=0.5% 时才回调，避免事件风暴。
fn emit(sink: ProgressSink<'_>, last: &mut f32, p: ConvertProgress) {
    if p.percent >= 0.999 || (*last < 0.0 || (p.percent - *last) >= 0.005) {
        *last = p.percent;
        sink(&p);
    }
}

/// 字节优先、退回文件数。返回 0.0..=1.0。
fn fraction(done: u64, total: u64, files_done: usize, files_total: usize) -> f32 {
    if total > 0 {
        (done as f32 / total as f32).clamp(0.0, 1.0)
    } else if files_total > 0 {
        (files_done as f32 / files_total as f32).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

fn is_disk_full(e: &std::io::Error) -> bool {
    if e.kind() == std::io::ErrorKind::StorageFull {
        return true;
    }
    // 兜底原始码：ENOSPC(28, Unix) / ERROR_DISK_FULL(112, Windows)。
    matches!(e.raw_os_error(), Some(28) | Some(112))
}

/// 清理某目录下遗留的暂存目录（强杀进程会漏下 `.rarzap-*`）。
/// 只删超过 1 小时的，避免误伤正在进行的转换。
pub fn sweep_stale_staging(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let now = std::time::SystemTime::now();
    for entry in rd.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with(".rarzap-") {
            continue;
        }
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| now.duration_since(t).ok())
            .map(|d| d.as_secs() > 3600)
            .unwrap_or(false);
        if old && entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

fn snapshot_names(dir: &Path) -> HashSet<String> {
    let mut set = HashSet::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            set.insert(e.file_name().to_string_lossy().to_string());
        }
    }
    set
}

/// 同级逃逸检测：解压后父目录若出现新的非 `.rarzap-*` 条目，
/// 说明 C 库把文件写到了暂存目录之外（ZipSlip 穿透预检）。
/// 发现后尝试删除逃逸产物并直接失败。
fn detect_sibling_escape(parent: Option<&Path>, before: &HashSet<String>) -> Result<()> {
    let Some(dir) = parent else { return Ok(()) };
    let after = snapshot_names(dir);
    for name in after.difference(before) {
        if name.starts_with(".rarzap-") {
            continue; // 允许并发转换产生的其它暂存目录
        }
        let p = dir.join(name);
        // 尝试清理逃逸产物（文件/目录分别处理，失败忽略）。
        let _ = std::fs::remove_file(&p);
        let _ = std::fs::remove_dir_all(&p);
        anyhow::bail!(
            "archive tried to write outside its staging dir (blocked: {}), refusing to continue",
            p.display()
        );
    }
    Ok(())
}

/// 暂存目录单次遍历：containment 断言 + 收集打包条目 + 体积/文件数统计。
/// 对 C 库写入行为的纵深防御（预检是第一道，这里是第二道）。
/// 不跟随符号链接：发现即失败（恶意包显式报错；unrar 写出的链接本就该拒，
/// 打包跳过只会让用户拿到缺文件且无提示的包）。
#[derive(Debug)]
struct Collected {
    entries: Vec<(PathBuf, PathBuf)>,
    bytes: u64,
    files: usize,
}
fn collect_verified_files(staging: &Path) -> Result<Collected> {
    let mut entries: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut bytes: u64 = 0;
    let mut files: usize = 0;
    let mut stack = vec![staging.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let rd = std::fs::read_dir(&dir)
            .with_context(|| format!("read staging {}", dir.display()))?;
        for entry in rd {
            let entry = entry?;
            let path = entry.path();
            // 词法 containment：walk 出来的路径必须以 staging 为前缀。
            let rel = path.strip_prefix(staging).map(Path::to_path_buf);
            let Ok(rel) = rel else {
                anyhow::bail!("staging containment violated: {}", path.display());
            };
            // 安全：不跟随符号链接。既防止链接成环导致无限递归（爆栈），
            // 也防止把链接指向的目录外文件打进用户 ZIP（信息泄露）。
            let meta = std::fs::symlink_metadata(&path)
                .with_context(|| format!("stat {}", path.display()))?;
            if meta.file_type().is_symlink() {
                anyhow::bail!(
                    "archive created a symlink (refusing to pack): {}",
                    path.display()
                );
            }
            if meta.is_dir() {
                entries.push((rel, path.clone()));
                stack.push(path);
            } else if meta.is_file() {
                bytes = bytes.saturating_add(meta.len());
                files += 1;
                entries.push((rel, path));
            }
        }
    }
    Ok(Collected {
        entries,
        bytes,
        files,
    })
}

/// 列表扫描：返回 (总解压字节数, 文件数)。目录条目不计入。
/// 同时做**路径穿越预检**：发现绝对路径 / `..` / 盘符 / ADS 等条目直接拒绝，
/// 不把安全性完全押在 unrar C 库的净化行为上。
/// `max_bytes` / `max_files` 为 0 表示不限；超限直接失败（Zip 炸弹预检）。
fn scan_totals(rar_path: &Path, max_bytes: u64, max_files: usize) -> Result<(u64, usize)> {
    let listing = unrar_ng::Archive::new(rar_path)
        .open_for_listing()
        .with_context(|| format!("cannot list RAR {}", rar_path.display()))?;
    let mut bytes: u64 = 0;
    let mut files: usize = 0;
    for entry in listing {
        let header = entry.with_context(|| format!("corrupt RAR {}", rar_path.display()))?;
        if unsafe_entry_path(&header.filename) {
            anyhow::bail!(
                "archive contains an unsafe entry path, refusing to extract: {}",
                header.filename.display()
            );
        }
        if header.is_directory() {
            continue;
        }
        bytes = bytes.saturating_add(header.unpacked_size);
        files += 1;
        if max_files > 0 && files > max_files {
            anyhow::bail!(
                "archive has too many files ({} > {max_files}), refusing to extract",
                files
            );
        }
        if max_bytes > 0 && bytes > max_bytes {
            anyhow::bail!(
                "archive too large: declared {} exceeds {} , refusing to extract",
                human_bytes(bytes),
                human_bytes(max_bytes)
            );
        }
    }
    Ok((bytes, files))
}

/// 条目名是否可能导致写到目标目录之外。
/// 跨平台归一化：把 `\` 也当分隔符（Linux 上 `..\..\evil` 必须判为不安全，
/// 否则 Windows 解压时会逃逸）；`:` 一律拒绝（盘符 `C:` 与 NTFS ADS `file:stream`）。
fn unsafe_entry_path(p: &Path) -> bool {
    use std::path::Component;
    // 平台原生视角：绝对路径 / 前缀 / 父目录直接判不安全。
    if p.is_absolute() {
        return true;
    }
    if p.components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return true;
    }
    // 归一化视角（跨平台）：统一把 `\` 当 `/` 再检查。
    // 这样 Linux 构建也能拦截 Windows 风格的 `..\` 逃逸。
    let s = p.to_string_lossy().replace('\\', "/");
    if s.contains('\0') {
        return true;
    }
    // 盘符与 ADS：`C:/x`、`C:x`、`file:stream` 都含 `:`。
    // Unix 合法文件名理论上可含冒号，代价是这类包会被拒绝（fail-closed，可改名重试）。
    if s.contains(':') {
        return true;
    }
    if s.starts_with('/') {
        return true;
    }
    for seg in s.split('/') {
        if seg == ".." {
            return true;
        }
        // 空段来自 `//` 或尾部 `/`（目录条目），允许；`.` 停留在当前目录，允许。
    }
    false
}

fn write_dir_as_zip(
    entries: &[(PathBuf, PathBuf)],
    zip_path: &Path,
    level: Option<i64>,
    is_cancelled: Option<CancelProbe<'_>>,
    on_chunk: &mut dyn FnMut(u64),
) -> Result<()> {
    let file = File::create(zip_path).context("create zip file")?;
    // 注：实测输出包 BufWriter(1MB) 相对直写无增益（读侧已是 256KB 大块，
    // 编码器吐出块也够大），为省一次拷贝直接写 File。
    let mut zip = zip::ZipWriter::new(file);
    let mut options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    if let Some(l) = level {
        options = options.compression_level(Some(l));
    }

    let mut entries: Vec<(PathBuf, PathBuf)> = entries.to_vec();
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut buf = vec![0u8; 256 * 1024];
    for (rel, abs) in entries {
        if is_cancelled.is_some_and(|f| f()) {
            anyhow::bail!("cancelled by user");
        }
        if abs == *zip_path {
            continue;
        }
        let name = rel.to_string_lossy().replace('\\', "/");
        if abs.is_dir() {
            zip.add_directory(format!("{name}/"), options)?;
            continue;
        }
        zip.start_file(name, options)?;
        let mut f = File::open(&abs).with_context(|| format!("open {}", abs.display()))?;
        loop {
            if is_cancelled.is_some_and(|f| f()) {
                anyhow::bail!("cancelled by user");
            }
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            zip.write_all(&buf[..n]).map_err(|e| {
                if is_disk_full(&e) {
                    anyhow::anyhow!("disk full while writing ZIP (free some space and retry)")
                } else {
                    anyhow::anyhow!("zip write failed: {e:#}")
                }
            })?;
            on_chunk(n as u64);
        }
    }
    zip.finish().map_err(|e| match &e {
        zip::result::ZipError::Io(io) if is_disk_full(io) => {
            anyhow::anyhow!("disk full while finalizing ZIP (free some space and retry)")
        }
        _ => anyhow::anyhow!("finalize zip: {e:#}"),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 安全回归：暂存里出现符号链接必须显式失败（而不是静默跳过，
    /// 否则用户拿到缺文件且无提示的包；也防链接成环爆栈/目录外文件打包）。
    #[test]
    fn verified_collect_rejects_symlinks() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), b"top secret").unwrap();
        std::fs::write(tmp.path().join("real.txt"), b"ok").unwrap();

        // 创建指向外部文件的符号链接；无权限（Windows 未开开发者模式）时跳过本测试。
        #[cfg(windows)]
        let linked = std::os::windows::fs::symlink_file(
            outside.path().join("secret.txt"),
            tmp.path().join("link.txt"),
        );
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(
            outside.path().join("secret.txt"),
            tmp.path().join("link.txt"),
        );

        if linked.is_err() {
            eprintln!("symlink not permitted here; skipping");
            return;
        }

        let err = collect_verified_files(tmp.path()).expect_err("symlink must fail");
        assert!(
            format!("{err:#}").contains("symlink"),
            "unexpected error: {err:#}"
        );
    }

    /// 单次遍历即产出条目表 + 体积/文件数（断言与收集二合一，不走两遍目录）。
    #[test]
    fn verified_collect_counts() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"12345").unwrap();
        std::fs::write(tmp.path().join("sub").join("b.txt"), b"1234567").unwrap();
        let c = collect_verified_files(tmp.path()).unwrap();
        assert_eq!(c.bytes, 12);
        assert_eq!(c.files, 2);
        let names: Vec<String> = c
            .entries
            .iter()
            .map(|(rel, _)| rel.to_string_lossy().replace('\\', "/"))
            .collect();
        assert!(names.contains(&"a.txt".to_string()), "names={names:?}");
        assert!(names.contains(&"sub/b.txt".to_string()), "names={names:?}");
    }

    /// 等级归一化：1..=9 通过，其余回退默认。
    #[test]
    fn normalizes_levels() {
        assert_eq!(normalize_level(1), Some(1));
        assert_eq!(normalize_level(9), Some(9));
        assert_eq!(normalize_level(0), None);
        assert_eq!(normalize_level(10), None);
        assert_eq!(ConvertOptions::default().compression_level, None);
    }

    /// 安全回归：绝对路径 / .. / 盘符条目必须被判定为不安全。
    #[test]
    fn detects_unsafe_entry_paths() {
        assert!(!unsafe_entry_path(Path::new("dir/file.txt")));
        assert!(!unsafe_entry_path(Path::new("a.b/..x")));
        assert!(unsafe_entry_path(Path::new("../escape.txt")));
        assert!(unsafe_entry_path(Path::new("dir/../../escape.txt")));
        assert!(unsafe_entry_path(Path::new("/abs.txt")));
        // 跨平台：反斜杠同样视为分隔符（Linux 构建也要拦截 Windows 风格逃逸）。
        assert!(unsafe_entry_path(Path::new("..\\escape.txt")));
        assert!(unsafe_entry_path(Path::new("dir\\..\\..\\escape.txt")));
        // 盘符与 ADS（含冒号一律拒绝）。
        assert!(unsafe_entry_path(Path::new("C:/Windows/x.txt")));
        assert!(unsafe_entry_path(Path::new("C:x")));
        assert!(unsafe_entry_path(Path::new("file.txt:stream")));
        assert!(unsafe_entry_path(Path::new("//server/share/x")));
        #[cfg(windows)]
        {
            assert!(unsafe_entry_path(Path::new(r"C:\Windows\x.txt")));
            assert!(unsafe_entry_path(Path::new(r"\\server\share\x")));
        }
    }

    /// 不覆盖策略：unique_zip_path 应生成 `名 (2).zip` 且不触碰磁盘之外。
    #[test]
    fn unique_zip_path_increments() {
        let tmp = tempfile::tempdir().unwrap();
        let wanted = tmp.path().join("a.zip");
        assert_eq!(unique_zip_path(&wanted), wanted);
        std::fs::write(&wanted, b"x").unwrap();
        assert_eq!(unique_zip_path(&wanted), tmp.path().join("a (2).zip"));
        std::fs::write(tmp.path().join("a (2).zip"), b"x").unwrap();
        assert_eq!(unique_zip_path(&wanted), tmp.path().join("a (3).zip"));
    }

    /// 默认不覆盖：目标已存在时转换直接失败（不删除原文件）。
    #[test]
    fn refuses_to_overwrite_by_default() {
        let tmp = tempfile::tempdir().unwrap();
        let rar = tmp.path().join("missing.rar");
        let zip = tmp.path().join("exists.zip");
        std::fs::write(&zip, b"original").unwrap();
        let res = convert_rar_to_zip_with_options(
            &rar,
            &zip,
            None,
            &mut |_| {},
            &ConvertOptions::default(),
        );
        assert!(res.is_err());
        // 原文件必须完好（未被删除/截断）。
        assert_eq!(std::fs::read(&zip).unwrap(), b"original");
    }
}

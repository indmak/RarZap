//! CORE 插件接口（开源侧唯一扩展点）。
//!
//! 约定：
//! - CORE 只做单文件 `RAR -> ZIP` 原语 + 本接口定义，不含任何批量调度与付费判断。
//! - 商业功能（如批量）单独成 crate，通过本 trait 向 CORE 注册扩展，
//!   代码放在仓库外的本地目录 `plugins/`，不进 git。
//! - 开源构建（`cargo build` / CI）零插件依赖即可完整编译运行。

use std::path::PathBuf;

/// 单个转换任务描述（插件与宿主之间传递的最小契约）。
#[derive(Debug, Clone)]
pub struct JobSpec {
    pub rar_path: PathBuf,
    pub zip_path: PathBuf,
}

impl JobSpec {
    pub fn new(rar_path: PathBuf, zip_path: PathBuf) -> Self {
        Self { rar_path, zip_path }
    }
}

/// 单个任务的执行结果。
#[derive(Debug, Clone)]
pub enum JobOutcome {
    Completed { output: PathBuf },
    Failed { error: String },
    Cancelled,
}

/// 进度事件：插件向 UI 回传的最小事件集（UI 层自行决定如何展示）。
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    JobStarted { name: String },
    JobProgress { percent: f32 },
    JobFinished(JobOutcome),
}

/// 插件能力描述（用于“已安装插件”列表展示，无任何付费语义）。
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
}

/// RarZap 插件接口。商业批量插件实现本 trait。
///
/// 注意：本 trait 刻意保持极小——CORE 不知道“批量”“并发”“取消令牌”等概念，
/// 这些全部是插件内部实现细节。
pub trait RarzapPlugin: Send + Sync {
    fn info(&self) -> PluginInfo;

    /// 执行单个任务。`is_cancelled` 为协作式取消探针（返回 true 即尽快停下）。
    /// 默认实现直接调用 CORE 单文件转换，原子写入、取消清理语义由 CORE 保证。
    fn convert_single(
        &self,
        spec: &JobSpec,
        is_cancelled: Option<&dyn Fn() -> bool>,
    ) -> anyhow::Result<()> {
        crate::core::converter::convert_rar_to_zip(&spec.rar_path, &spec.zip_path, is_cancelled)
    }

    /// 把一次转换拆成“可展示进度”的步骤时调用；默认单步完成。
    /// 批量插件可覆盖为多任务事件流。
    fn on_progress(&self, _event: &ProgressEvent) {}
}

//! FREE（开源基础功能）：单文件 RAR -> ZIP。
//! 只有“一次转一个文件”的能力：无队列、无并发、无批量。
//! 批量能力由 `plugins/` 下的商业插件通过 [`crate::core::RarzapPlugin`] 提供。

pub mod service;

pub use service::{
    Progress, SingleRequest, first_rar, is_rar_path, plan_single, run_single, run_single_with_progress,
};

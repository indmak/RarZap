//! CORE：单文件 RAR -> ZIP 原语 + 插件接口声明。
//! 无批量、无并发、无付费判断——扩展点见 [`plugin::RarzapPlugin`]。

pub mod converter;
pub mod error;
pub mod plugin;

pub use converter::{
    CancelProbe, ConvertOptions, ConvertProgress, Phase, ProgressSink,
    convert_rar_to_zip, convert_rar_to_zip_with_options, convert_rar_to_zip_with_progress,
    default_zip_name_for, sweep_stale_staging, unique_zip_path,
    DEFAULT_MAX_FILES, DEFAULT_MAX_TOTAL_BYTES,
};
pub use error::ConvertError;
pub use plugin::{JobOutcome, JobSpec, PluginInfo, ProgressEvent, RarzapPlugin};

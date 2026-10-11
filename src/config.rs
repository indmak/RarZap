//! App config: theme mode + language + output dir. Persisted as JSON.
//! 注意：无并发/批量相关配置——那是商业插件的内部事项，不进开源配置。

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    /// Follow OS appearance.
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub theme_mode: ThemeMode,
    /// None = 跟随系统；Some(code) = 强制语言（如 "zh-CN" / "en"）。
    pub language: Option<String>,
    /// None = 与源文件同目录。
    pub output_dir: Option<PathBuf>,
}

fn config_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "RarZap").map(|d| d.config_dir().join("config.json"))
}

/// 配置目录（日志等也放这棵树下）。
pub fn data_dir() -> Option<PathBuf> {
    ProjectDirs::from("", "", "RarZap").map(|d| d.data_local_dir().to_path_buf())
}

impl AppConfig {
    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        std::fs::read_to_string(&path)
            .ok()
            .map(|s| s.trim_start_matches('\u{feff}').to_string())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = config_path() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, s);
        }
    }
}

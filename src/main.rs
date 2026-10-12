//! RarZap 开源版入口（单文件 RAR -> ZIP）。
//! 全部逻辑在库里（`rarzap::run`），本文件只做转发，避免 bin/lib 两套模块树。
//!
//! `windows_subsystem = "windows"`：release 版按 GUI 子系统链接，启动不再弹黑框控制台
//! （debug 保留控制台，方便看日志）。仅 Windows 有意义，用 `windows` 门控避免其它平台告警。
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() {
    rarzap::run();
}

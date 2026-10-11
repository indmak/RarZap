//! 通用 UI 扩展点。
//!
//! 开源版本身不含任何商业功能：它只提供“外部二进制可注入额外屏幕”的机制。
//! 免费构建（`rarzap::run`）不注入任何扩展；商业二进制通过 [`crate::run_with`]
//! 注入自己的屏幕实现（如批量转换）。
//!
//! 这里刻意保持通用——没有“批量”“付费”“pro”等概念，只有“扩展屏幕”。

use std::path::PathBuf;

use gpui_kit::component::IconName;
use gpui_kit::{AnyElement, App, SharedString, Window};

/// 一个可注入的整屏扩展。
pub trait UiExtension: 'static {
    /// 稳定标识（用于元素 id）。
    fn id(&self) -> &'static str;
    /// 导航按钮上的本地化标题。
    fn title(&self) -> SharedString;
    /// 导航按钮图标。
    fn icon(&self) -> IconName;
    /// 可选的徽标文本（例如扩展自己的版本/品牌标记）。默认无。
    fn badge(&self) -> Option<SharedString> {
        None
    }
    /// 渲染该扩展的整屏内容。
    fn render(&self, window: &mut Window, cx: &mut App) -> AnyElement;
    /// 宿主收到系统文件拖放时转发；返回 `true` 表示已消费该拖放。
    fn on_files_dropped(&self, _paths: &[PathBuf], _cx: &mut App) -> bool {
        false
    }
    /// 可选：导航行右侧的状态区（状态徽标 / 引导按钮等）。默认无。
    fn nav_status(&self, _window: &mut Window, _cx: &mut App) -> Option<AnyElement> {
        None
    }
    /// 可选：向设置页贡献一个分组（标题, 内容）。默认无。
    fn settings_section(
        &self,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<(String, AnyElement)> {
        None
    }
    /// 可选：覆盖整个窗口的浮层（如模态对话框），由宿主最后绘制。默认无。
    fn overlay(&self, _window: &mut Window, _cx: &mut App) -> Option<AnyElement> {
        None
    }
    /// 可选：替换隐私页正文（例如需要说明联网激活的版本）。默认无。
    fn privacy_content(&self, _window: &mut Window, _cx: &mut App) -> Option<AnyElement> {
        None
    }
}

/// 扩展工厂：在窗口创建时（有 Window/App 上下文）构造扩展实例。
pub type ExtensionFactory =
    Box<dyn FnOnce(&mut Window, &mut App) -> Vec<Box<dyn UiExtension>>>;

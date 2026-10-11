//! RarZap 开源库：CORE（单文件原语 + 插件接口）+ FREE（单文件功能）+ 桌面 UI。
//! 商业功能通过 [`ui::UiExtension`] 注入额外屏幕，由外部二进制提供；
//! 本仓库不含任何批量与付费代码。

pub mod config;
pub mod core;
pub mod free;
pub mod i18n;
pub mod logging;
pub mod theme;
pub mod ui;

// i18n 后端必须挂在 crate root（见 i18n.rs 注释）。
rust_i18n::i18n!("locales", fallback = "en");

use gpui_kit::component::TitleBar;
use gpui_kit::{App, Bounds, WindowBounds, WindowOptions, prelude::*, px, size};

pub use ui::{ExtensionFactory, UiExtension};

/// 启动免费版桌面应用（不注入任何扩展）。
pub fn run() {
    run_with(None);
}

/// 启动应用，可选注入扩展屏幕（商业二进制用）。
/// 免费构建传 `None`，行为与 [`run`] 完全一致。
pub fn run_with(extension_factory: Option<ExtensionFactory>) {
    logging::init_logging();
    i18n::init_locale();

    tracing::info!("RarZap {} starting", env!("CARGO_PKG_VERSION"));

    // 组件库的图标/控件依赖资产源（图标 SVG 内嵌在 gpui-kit-assets）。
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx: &mut App| {
            gpui_kit::init(cx);

            // 启动清理：过期暂存目录顺手删掉。read_dir 在大目录可达百毫秒级，
            // 放独立线程，首帧不等它。
            if let Some(dir) = config::AppConfig::load().output_dir {
                std::thread::spawn(move || {
                    core::sweep_stale_staging(&dir);
                });
            }

            let bounds = Bounds::centered(None, size(px(760.0), px(560.0)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                // 最小尺寸，避免拖到不可用（设计文档 §4.1）。
                window_min_size: Some(size(px(560.0), px(420.0))),
                // Wayland app_id：与 rarzap.desktop 的 StartupWMClass 对齐，
                // 否则 GNOME Dock 认不出窗口，只能显示通用图标。
                app_id: Some("rarzap".to_string()),
                // TitleBar 需要：透明标题栏 + 由应用自己负责拖移。
                ..TitleBar::window_options()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                let extensions = extension_factory
                    .map(|factory| factory(window, cx))
                    .unwrap_or_default();
                cx.new(|cx| ui::app::RootView::new(window, cx, extensions))
            })
            .expect("open RarZap window");
            cx.activate(true);
        });
}

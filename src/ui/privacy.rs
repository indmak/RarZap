//! 隐私页：说明本地处理、无上传；提供打开日志目录；展示版本与许可。
//! 正文可被扩展替换（如商业版需说明联网激活）——见 [`crate::ui::UiExtension::privacy_content`]。

use gpui_kit::component::{
    ActiveTheme, IconName, Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit::{AnyElement, Context, Window, div, prelude::*, px};

use super::app::{RootView, Screen};
use crate::i18n::t;

impl RootView {
    pub(crate) fn render_privacy(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let back_view = cx.entity();
        // 扩展可替换隐私正文（免费版不注入扩展，用下面的默认文案）。
        let custom: Option<AnyElement> = self
            .extensions
            .iter()
            .find_map(|ext| ext.privacy_content(window, cx));
        let theme = cx.theme();
        let version = env!("CARGO_PKG_VERSION");

        let body = custom.unwrap_or_else(|| default_body(theme));

        div()
            .id("privacy-scroll")
            .flex()
            .flex_1()
            .flex_col()
            .gap(px(16.0))
            .p(px(24.0))
            .overflow_scroll()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        Button::new("back")
                            .ghost()
                            .xsmall()
                            .icon(IconName::ArrowLeft)
                            .label(t!("nav.back").to_string())
                            .on_click(move |_, _, cx| {
                                back_view.update(cx, |v, cx| v.goto(Screen::Main, cx));
                            }),
                    ),
            )
            .child(body)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        Button::new("open-logs")
                            .label(t!("privacy.open_logs").to_string())
                            .xsmall()
                            .on_click(move |_, _, cx| {
                                let dir = crate::logging::log_dir();
                                cx.reveal_path(&dir);
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("{} {}", t!("privacy.version").to_string(), version)),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(t!("privacy.license").to_string()),
            )
            .into_any_element()
    }
}

/// 免费版默认隐私正文（纯本地、无网络）。
fn default_body(theme: &gpui_kit::component::Theme) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .p(px(16.0))
        .bg(theme.popover)
        .border_1()
        .border_color(theme.border)
        .rounded(px(12.0))
        .child(paragraph(t!("privacy.p1").to_string()))
        .child(paragraph(t!("privacy.p2").to_string()))
        .child(paragraph(t!("privacy.p3").to_string()))
        .into_any_element()
}

fn paragraph(text: String) -> AnyElement {
    div().text_sm().child(text).into_any_element()
}

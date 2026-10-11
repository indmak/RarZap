//! 设置页：语言、主题、输出目录。全部即时生效并持久化到 config.json。

use gpui_kit::component::{
    ActiveTheme, Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit::{AnyElement, Context, Window, div, prelude::*, px};

use super::app::{RootView, Screen};
use crate::config::ThemeMode;
use crate::i18n::{LANGUAGES, t};

impl RootView {
    pub(crate) fn render_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let current_lang = self.config.language.clone();
        let current_theme = self.config.theme_mode;
        let out_label = self.output_dir_label();
        let has_custom_out = self.config.output_dir.is_some();

        // ---- 语言：跟随系统 + 8 种 ----
        let mut lang_row = div().flex().flex_row().flex_wrap().gap(px(6.0));
        {
            let view = cx.entity();
            lang_row = lang_row.child(
                Button::new("lang-system")
                    .label(t!("settings.follow_system").to_string())
                    .xsmall()
                    .when(current_lang.is_none(), |b| b.primary())
                    .when(current_lang.is_some(), |b| b.ghost())
                    .on_click(move |_, _, cx| {
                        view.update(cx, |v, cx| v.set_language(None, cx));
                    }),
            );
        }
        for (code, name) in LANGUAGES {
            let code_owned = (*code).to_string();
            let active = current_lang.as_deref() == Some(*code);
            let view = cx.entity();
            lang_row = lang_row.child(
                Button::new(format!("lang-{code}"))
                    .label((*name).to_string())
                    .xsmall()
                    .when(active, |b| b.primary())
                    .when(!active, |b| b.ghost())
                    .on_click(move |_, _, cx| {
                        let code = Some(code_owned.clone());
                        view.update(cx, |v, cx| v.set_language(code, cx));
                    }),
            );
        }

        // ---- 主题：跟随系统 / 浅色 / 深色 ----
        let theme_modes = [
            (ThemeMode::System, "theme-system", t!("settings.theme_system").to_string()),
            (ThemeMode::Light, "theme-light", t!("settings.theme_light").to_string()),
            (ThemeMode::Dark, "theme-dark", t!("settings.theme_dark").to_string()),
        ];
        let mut theme_row = div().flex().flex_row().gap(px(6.0));
        for (mode, id, label) in theme_modes {
            let view = cx.entity();
            let active = current_theme == mode;
            theme_row = theme_row.child(
                Button::new(id)
                    .label(label)
                    .xsmall()
                    .when(active, |b| b.primary())
                    .when(!active, |b| b.ghost())
                    .on_click(move |_, window, cx| {
                        view.update(cx, |v, cx| v.set_theme(mode, window, cx));
                    }),
            );
        }

        // ---- 输出目录 ----
        let change_view = cx.entity();
        let reset_view = cx.entity();
        let output_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(div().flex_1().text_xs().child(out_label))
            .child(
                Button::new("out-change")
                    .label(t!("settings.change").to_string())
                    .xsmall()
                    .on_click(move |_, window, cx| {
                        RootView::pick_output_dir(change_view.clone(), window, cx);
                    }),
            )
            .child(
                Button::new("out-reset")
                    .ghost()
                    .label(t!("settings.reset_default").to_string())
                    .xsmall()
                    .disabled(!has_custom_out)
                    .on_click(move |_, _, cx| {
                        reset_view.update(cx, |v, cx| v.set_output_dir(None, cx));
                    }),
            );

        // 扩展贡献的设置分组（先取原始数据，再取 theme，避免借用冲突）。
        let ext_raw: Vec<(String, AnyElement)> = self
            .extensions
            .iter()
            .filter_map(|ext| ext.settings_section(window, cx))
            .collect();
        let back_view = cx.entity();
        let theme = cx.theme();
        let ext_sections: Vec<AnyElement> = ext_raw
            .into_iter()
            .map(|(title, body)| section(theme, title, body))
            .collect();

        div()
            .id("settings-scroll")
            .flex()
            .flex_1()
            .flex_col()
            .gap(px(18.0))
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
            .child(section(
                theme,
                t!("settings.language").to_string(),
                lang_row.into_any_element(),
            ))
            .child(section(
                theme,
                t!("settings.theme").to_string(),
                theme_row.into_any_element(),
            ))
            .child(section(
                theme,
                t!("settings.output_dir").to_string(),
                output_row.into_any_element(),
            ))
            .children(ext_sections)
            .into_any_element()
    }
}

/// 分组：小标题 + 内容。
fn section(
    theme: &gpui_kit::component::Theme,
    title: String,
    body: AnyElement,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(title),
        )
        .child(body)
        .into_any_element()
}

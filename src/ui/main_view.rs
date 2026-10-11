//! 主页三态渲染：空（拖放/选择）、转换中（真实进度+取消）、完成（结果+后续操作）。

use gpui_kit::component::{
    ActiveTheme, IconName, Sizable,
    button::{Button, ButtonVariants},
    progress::Progress,
};
use gpui_kit::{AnyElement, Context, ExternalPaths, IntoElement, div, prelude::*, px};

use super::app::{Outcome, RootView, Stage};
use crate::i18n::t;

impl RootView {
    pub(crate) fn render_main(
        &mut self,
        _window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match self.stage {
            Stage::Empty => self.render_empty(cx),
            Stage::Working => self.render_working(cx),
            Stage::Done => self.render_done(cx),
        }
    }

    // ---------- 空状态 ----------

    fn render_empty(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let view = cx.entity();
        let error = self.error.clone();

        div()
            .flex()
            .flex_1()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(16.0))
            .p(px(32.0))
            .child(
                // 拖放区：拖入 .rar 时边框高亮（GPUI 将 OS 拖放转成 ExternalPaths 内部拖放）。
                div()
                    .id("drop-zone")
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(12.0))
                    .w(px(420.0))
                    .py(px(40.0))
                    .px(px(24.0))
                    .bg(theme.popover)
                    .border_2()
                    .border_color(theme.border)
                    .rounded(px(14.0))
                    .drag_over::<ExternalPaths>(move |style, _paths, _window, cx| {
                        style.border_color(cx.theme().accent)
                    })
                    .child(div().text_xl().child(t!("empty.title").to_string()))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(t!("empty.hint").to_string()),
                    )
                    .child(
                        Button::new("pick-file")
                            .primary()
                            .label(t!("empty.select_file").to_string())
                            .on_click(move |_, window, cx| {
                                RootView::pick_file(view.clone(), window, cx);
                            }),
                    ),
            )
            .when_some(error, |this, msg| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(theme.danger)
                        .child(msg),
                )
            })
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(t!("empty.footer").to_string()),
            )
            .into_any_element()
    }

    // ---------- 转换中 ----------

    fn render_working(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let view = cx.entity();
        let name = self.file_label();

        div()
            .flex()
            .flex_1()
            .flex_col()
            .items_center()
            .justify_center()
            .p(px(32.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .w(px(460.0))
                    .p(px(20.0))
                    .bg(theme.popover)
                    .border_1()
                    .border_color(theme.border)
                    .rounded(px(12.0))
                    .child(div().text_lg().child(name))
                    .child(
                        Progress::new("convert-progress")
                            .value(self.progress * 100.0)
                            // 刚开始（还没有字节进度）时用不定态动画，避免看着像卡住。
                            .loading(self.progress <= 0.01),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(self.progress_text.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("{:.0}%", self.progress * 100.0)),
                            ),
                    )
                    .child(
                        div().flex().flex_row().justify_end().child(
                            Button::new("cancel")
                                .ghost()
                                .label(t!("working.cancel").to_string())
                                .on_click(move |_, _, cx| {
                                    view.update(cx, |v, cx| v.cancel_convert(cx));
                                }),
                        ),
                    ),
            )
            .into_any_element()
    }

    // ---------- 完成 ----------

    fn render_done(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let view = cx.entity();
        let name = self.file_label();
        let zip = self
            .zip_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let (icon, title, color) = match self.outcome {
            Outcome::Completed => (
                IconName::CircleCheck,
                t!("done.title_ok").to_string(),
                theme.success,
            ),
            Outcome::Failed => (
                IconName::CircleX,
                t!("done.title_fail").to_string(),
                theme.danger,
            ),
            Outcome::Cancelled | Outcome::Working => (
                IconName::CircleAlert,
                t!("done.title_cancelled").to_string(),
                theme.warning,
            ),
        };

        let mut actions = div().flex().flex_row().gap(px(8.0)).justify_end();
        if self.outcome == Outcome::Completed {
            let reveal = view.clone();
            actions = actions.child(
                Button::new("reveal")
                    .label(t!("done.reveal").to_string())
                    .on_click(move |_, _, cx| {
                        let zip = reveal.update(cx, |v, _| v.zip_path.clone());
                        if let Some(z) = zip {
                            cx.reveal_path(&z);
                        }
                    }),
            );
        }
        if self.outcome == Outcome::Failed {
            let retry = view.clone();
            let rar = self.rar_path.clone();
            actions = actions.child(
                Button::new("retry")
                    .label(t!("done.retry").to_string())
                    .on_click(move |_, _, cx| {
                        if let Some(p) = rar.clone() {
                            retry.update(cx, |v, cx| v.set_file(p, cx));
                        }
                    }),
            );
        }
        let again = view.clone();
        actions = actions.child(
            Button::new("again")
                .primary()
                .label(t!("done.again").to_string())
                .on_click(move |_, _, cx| {
                    again.update(cx, |v, cx| v.reset(cx));
                }),
        );

        div()
            .flex()
            .flex_1()
            .flex_col()
            .items_center()
            .justify_center()
            .p(px(32.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .w(px(460.0))
                    .p(px(20.0))
                    .bg(theme.popover)
                    .border_1()
                    .border_color(theme.border)
                    .rounded(px(12.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                gpui_kit::component::Icon::new(icon)
                                    .text_color(color)
                                    .small(),
                            )
                            .child(div().text_lg().text_color(color).child(title)),
                    )
                    .child(div().text_sm().child(name))
                    .when(self.outcome == Outcome::Completed, |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(format!("{zip} · {}s", self.elapsed_secs)),
                        )
                    })
                    .when(self.outcome == Outcome::Failed, |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(theme.danger)
                                .child(self.error.clone().unwrap_or_default()),
                        )
                    })
                    .child(actions),
            )
            .into_any_element()
    }
}

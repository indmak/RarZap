//! 根视图：屏幕路由（主页/设置/隐私）+ 主页三态（空/转换中/完成）。
//! 关键修复点：
//! - 用 gpui-component `TitleBar` 接管窗口拖移、双击最大化、最小化/关闭按钮。
//! - 系统文件拖放经 `on_drop::<ExternalPaths>` 接入（GPUI 会把 OS 拖放转成内部拖放）。
//! - 转换进度来自 CORE 的真实回调（解压逐文件 + 打包逐块），经 mpsc + 定时器转回 UI 线程。

use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, TryRecvError},
};
use std::time::{Duration, Instant};

use gpui_kit::component::{
    ActiveTheme, IconName, Selectable, Sizable, TitleBar,
    button::{Button, ButtonVariants},
};
use gpui_kit::{
    AnyElement, App, Context, Entity, ExternalPaths, Subscription, Window, WindowAppearance, div,
    prelude::*, px,
};

use super::ext::UiExtension;
use crate::config::{AppConfig, ThemeMode};
use crate::core::{ConvertProgress, Phase};
use crate::free::{SingleRequest, first_rar, plan_single};
use crate::i18n::t;

/// 顶层页面。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    Main,
    Settings,
    Privacy,
}

/// 主页阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stage {
    #[default]
    Empty,
    Working,
    Done,
}

/// 转换结果状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Outcome {
    #[default]
    Working,
    Completed,
    Failed,
    Cancelled,
}

pub struct RootView {
    pub screen: Screen,
    pub stage: Stage,
    pub outcome: Outcome,
    pub rar_path: Option<PathBuf>,
    pub zip_path: Option<PathBuf>,
    pub error: Option<String>,
    /// 0.0..=1.0 真实进度。
    pub progress: f32,
    /// 已本地化的进度描述（阶段 · 计数 · 文件名）。
    pub progress_text: String,
    pub config: AppConfig,
    cancel_flag: Arc<AtomicBool>,
    started_at: Option<Instant>,
    pub elapsed_secs: u64,
    /// 上次因进度刷新 UI 的时间（进度事件节流）。
    last_notify: Option<Instant>,
    /// 冒烟/截图用：启动即转换该文件（`RARZAP_OPEN`），首帧后消费。
    pending_open: Option<PathBuf>,
    /// 注入的扩展屏幕（免费构建为空）。见 [`super::ext::UiExtension`]。
    pub extensions: Vec<Box<dyn UiExtension>>,
    /// 当前激活的扩展索引；None 表示开源主页/设置/隐私。
    pub active_ext: Option<usize>,
    _appearance_sub: Option<Subscription>,
}

impl RootView {
    pub fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        extensions: Vec<Box<dyn UiExtension>>,
    ) -> Self {
        let config = AppConfig::load();
        // 启动即应用配置里的主题与语言。
        apply_theme(config.theme_mode, window, cx);
        crate::i18n::apply_locale(config.language.as_deref());

        // 跟随系统时，系统外观变化要即时重绘。
        let sub = cx.observe_window_appearance(window, |this, window, cx| {
            if this.config.theme_mode == ThemeMode::System {
                apply_theme(ThemeMode::System, window, cx);
            }
        });

        Self {
            screen: initial_screen(),
            stage: Stage::Empty,
            outcome: Outcome::Working,
            rar_path: None,
            zip_path: None,
            error: None,
            progress: 0.0,
            progress_text: String::new(),
            config,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            started_at: None,
            elapsed_secs: 0,
            last_notify: None,
            pending_open: debug_env("RARZAP_OPEN").map(PathBuf::from),
            extensions,
            // 冒烟/截图：启动即激活第 N 个扩展（RARZAP_START_EXT=0）。
            active_ext: debug_env("RARZAP_START_EXT")
                .and_then(|s| s.parse::<usize>().ok()),
            _appearance_sub: Some(sub),
        }
    }

    // ---------- 导航 ----------

    pub(crate) fn goto(&mut self, screen: Screen, cx: &mut Context<Self>) {
        tracing::info!(?screen, "navigate");
        self.screen = screen;
        self.active_ext = None;
        cx.notify();
    }

    /// 切换扩展屏幕（再点一次则回到开源主页）。
    pub(crate) fn toggle_ext(&mut self, index: usize, cx: &mut Context<Self>) {
        self.active_ext = if self.active_ext == Some(index) {
            None
        } else {
            Some(index)
        };
        cx.notify();
    }

    // ---------- 拖放 / 选择文件 ----------

    /// 系统拖入文件：先交给扩展（如批量）消费；未消费则按免费版单文件处理。
    pub(crate) fn on_files_dropped(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if let Some(index) = self
            .extensions
            .iter()
            .position(|ext| ext.on_files_dropped(&paths, cx))
        {
            self.active_ext = Some(index);
            cx.notify();
            return;
        }
        self.on_files_dropped_free(paths, cx);
    }

    /// 免费版：只取第一个 .rar。
    fn on_files_dropped_free(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if self.stage == Stage::Working {
            return; // 转换中忽略新的拖入，避免覆盖进行中的任务
        }
        match first_rar(&paths) {
            Some(rar) => {
                self.error = None;
                self.set_file(rar, cx);
            }
            None => {
                self.error = Some(t!("error.not_rar").to_string());
                cx.notify();
            }
        }
    }

    /// 选中文件（只取第一个 .rar；批量不是免费版能力）。
    pub(crate) fn set_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        tracing::info!(path = %path.display(), "set_file");
        let req = SingleRequest::new(path.clone(), self.config.output_dir.clone());
        match plan_single(&req) {
            Ok(zip) => {
                self.screen = Screen::Main;
                self.rar_path = Some(path);
                self.zip_path = Some(zip);
                self.error = None;
                self.start_convert(cx);
            }
            Err(e) => {
                self.error = Some(format!("{e:#}"));
                cx.notify();
            }
        }
    }

    pub(crate) fn pick_file(
        view: Entity<Self>,
        window: &mut Window,
        cx: &mut App,
    ) {
        // macOS checklist: 原生单文件选择；取消（None）直接无操作。
        let rx = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(t!("empty.select_file").to_string().into()),
        });
        cx.spawn(async move |cx| {
            let Ok(Ok(Some(mut paths))) = rx.await else {
                return;
            };
            if let Some(first) = paths.drain(..).next() {
                cx.update(|cx| {
                    view.update(cx, |v, cx| v.set_file(first, cx));
                });
            }
        })
        .detach();
        let _ = window;
    }

    // ---------- 转换 ----------

    fn start_convert(&mut self, cx: &mut Context<Self>) {
        let Some(rar) = self.rar_path.clone() else {
            return;
        };
        self.started_at = Some(Instant::now());
        self.cancel_flag.store(false, Ordering::SeqCst);
        self.outcome = Outcome::Working;
        self.stage = Stage::Working;
        self.progress = 0.0;
        self.progress_text = t!("working.extracting").to_string();
        cx.notify();

        let flag = self.cancel_flag.clone();
        let output_dir = self.config.output_dir.clone();
        let (tx, rx) = mpsc::channel::<Msg>();

        cx.spawn(async move |view, cx| {
            // 重活（unrar 解压 + zip 打包，可跑数分钟）放独立线程，
            // 不占 GPUI 后台池——池留给定时器/弹窗等，进度与取消才不迟钝。
            let tx_work = tx.clone();
            let _worker = std::thread::spawn(move || {
                let req = SingleRequest::new(rar, output_dir);
                let res = crate::free::run_single_with_progress(
                    &req,
                    Some(&|| flag.load(Ordering::SeqCst)),
                    &mut |p: &ConvertProgress| {
                        let _ = tx_work.send(Msg::Progress(p.clone()));
                    },
                );
                let out = res.map_err(|e| format!("{e:#}"));
                let _ = tx_work.send(Msg::Done(out));
            });

            // 前台轮询：把后台事件转成 UI 更新（50ms 一拍，够顺滑）。
            loop {
                match rx.try_recv() {
                    Ok(Msg::Progress(p)) => {
                        cx.update(|cx| {
                            view.update(cx, |v, cx| v.apply_progress(&p, cx)).ok();
                        });
                    }
                    Ok(Msg::Done(res)) => {
                        cx.update(|cx| {
                            view.update(cx, |v, cx| v.apply_done(res, cx)).ok();
                        });
                        break;
                    }
                    Err(TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(Duration::from_millis(50))
                            .await;
                    }
                    Err(TryRecvError::Disconnected) => break,
                }
            }
        })
        .detach();
    }

    fn apply_progress(&mut self, p: &ConvertProgress, cx: &mut Context<Self>) {
        // 进度回调可达每秒数十次，notify 节流到 100ms；完成/失败走 apply_done（必通知）。
        // 节流判据放最前：被吞的事件连字符串都不拼，省掉 format!/t!() 开销。
        let now = Instant::now();
        let due = self
            .last_notify
            .map(|t| now.duration_since(t).as_millis() >= 100)
            .unwrap_or(true);
        if !due {
            return;
        }
        self.last_notify = Some(now);
        self.progress = p.percent.clamp(0.0, 1.0);
        let phase = match p.phase {
            Phase::Extracting => t!("working.extracting").to_string(),
            Phase::Packing => t!("working.packing").to_string(),
        };
        self.progress_text = match p.phase {
            Phase::Extracting => {
                let name = short_name(&p.current_file);
                if name.is_empty() {
                    format!("{phase} · {}/{}", p.files_done, p.files_total)
                } else {
                    format!(
                        "{phase} · {}/{} · {name}",
                        p.files_done, p.files_total
                    )
                }
            }
            Phase::Packing => format!("{phase} · {:.0}%", p.percent * 100.0),
        };
        cx.notify();
    }

    fn apply_done(&mut self, res: Result<PathBuf, String>, cx: &mut Context<Self>) {
        match res {
            Ok(zip) => {
                self.zip_path = Some(zip);
                self.outcome = Outcome::Completed;
                self.progress = 1.0;
            }
            Err(e) if e.contains("cancelled") => {
                self.outcome = Outcome::Cancelled;
                self.error = None;
            }
            Err(e) => {
                self.outcome = Outcome::Failed;
                self.error = Some(e);
            }
        }
        self.stage = Stage::Done;
        self.elapsed_secs = self.started_at.map(|t| t.elapsed().as_secs()).unwrap_or(0);
        cx.notify();
    }

    pub(crate) fn cancel_convert(&mut self, cx: &mut Context<Self>) {
        self.cancel_flag.store(true, Ordering::SeqCst);
        self.progress_text = t!("status.cancelling").to_string();
        cx.notify();
    }

    pub(crate) fn reset(&mut self, cx: &mut Context<Self>) {
        self.stage = Stage::Empty;
        self.outcome = Outcome::Working;
        self.rar_path = None;
        self.zip_path = None;
        self.error = None;
        self.progress = 0.0;
        self.progress_text.clear();
        cx.notify();
    }

    // ---------- 设置动作 ----------

    pub(crate) fn set_language(&mut self, code: Option<String>, cx: &mut Context<Self>) {
        crate::i18n::apply_locale(code.as_deref());
        self.config.language = code;
        self.config.save();
        cx.notify();
    }

    pub(crate) fn set_theme(&mut self, mode: ThemeMode, window: &mut Window, cx: &mut Context<Self>) {
        self.config.theme_mode = mode;
        apply_theme(mode, window, cx);
        self.config.save();
        cx.notify();
    }

    pub(crate) fn set_output_dir(&mut self, dir: Option<PathBuf>, cx: &mut Context<Self>) {
        self.config.output_dir = dir;
        self.config.save();
        cx.notify();
    }

    pub(crate) fn pick_output_dir(
        view: Entity<Self>,
        _window: &mut Window,
        cx: &mut App,
    ) {
        let rx = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t!("settings.output_dir").to_string().into()),
        });
        cx.spawn(async move |cx| {
            let Ok(Ok(Some(mut paths))) = rx.await else {
                return;
            };
            if let Some(dir) = paths.drain(..).next() {
                cx.update(|cx| {
                    view.update(cx, |v, cx| v.set_output_dir(Some(dir), cx));
                });
            }
        })
        .detach();
    }

    // ---------- 渲染辅助 ----------

    pub(crate) fn file_label(&self) -> String {
        self.rar_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    pub(crate) fn output_dir_label(&self) -> String {
        match &self.config.output_dir {
            Some(d) => d.to_string_lossy().to_string(),
            None => t!("settings.output_same_as_source").to_string(),
        }
    }

    fn render_title_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let _ = cx;
        // 标题栏只用组件 TitleBar：它负责拖移/双击最大化/最小化/关闭。
        // 注意：Windows 上标题栏内部是 OS 拖拽区（HTCAPTION），放按钮会被吞掉点击，
        // 所以设置/隐私按钮放在下方内容区的导航行里（见 render_nav_bar）。
        TitleBar::new()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .size_full()
                    .px_2()
                    .child(
                        div()
                            .text_sm()
                            .child(t!("app.title").to_string()),
                    ),
            )
            .into_any_element()
    }

    /// 内容区导航行：扩展按钮（如有）+ 设置 / 隐私（带文字标签，点击可靠）。
    fn render_nav_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let view = cx.entity();
        let nav = |id: &'static str, icon: IconName, label: String, active: bool, target: Screen| {
            let view = view.clone();
            Button::new(id)
                .ghost()
                .xsmall()
                .icon(icon)
                .label(label)
                .when(active, |b| b.selected(true))
                .on_click(move |_, _, cx| {
                    view.update(cx, |v, cx| {
                        let screen = if v.screen == target { Screen::Main } else { target };
                        v.goto(screen, cx);
                    });
                })
        };

        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_end()
            .gap(px(6.0))
            .h(px(34.0))
            .px(px(8.0));

        // 扩展屏幕按钮（开源构建时为空）。徽标由扩展自行提供，显示在按钮左侧。
        for (i, ext) in self.extensions.iter().enumerate() {
            if let Some(badge) = ext.badge() {
                row = row.child(
                    div()
                        .text_xs()
                        .px(px(5.0))
                        .py(px(1.0))
                        .rounded(px(4.0))
                        .bg(theme.accent)
                        .text_color(theme.accent_foreground)
                        .child(badge),
                );
            }
            let view = view.clone();
            let active = self.active_ext == Some(i);
            row = row.child(
                Button::new(format!("ext-{}", ext.id()))
                    .ghost()
                    .xsmall()
                    .icon(ext.icon())
                    .label(ext.title().to_string())
                    .when(active, |b| b.selected(true))
                    .on_click(move |_, _, cx| {
                        view.update(cx, |v, cx| v.toggle_ext(i, cx));
                    }),
            );
        }

        row.child(nav(
            "nav-settings",
            IconName::Settings,
            t!("nav.settings").to_string(),
            self.active_ext.is_none() && self.screen == Screen::Settings,
            Screen::Settings,
        ))
        .child(nav(
            "nav-privacy",
            IconName::Info,
            t!("nav.privacy").to_string(),
            self.active_ext.is_none() && self.screen == Screen::Privacy,
            Screen::Privacy,
        ))
        // 扩展提供的状态区（如激活状态 + 引导按钮）。
        .children(
            self.extensions
                .iter()
                .filter_map(|ext| ext.nav_status(window, cx)),
        )
        .into_any_element()
    }
}

impl Drop for RootView {
    fn drop(&mut self) {
        // 视图释放（关窗）时通知后台转换停下，避免关窗后仍全速跑完大包。
        // 后台线程的取消探针与 C 库回调会感知并清理半成品。
        self.cancel_flag.store(true, Ordering::SeqCst);
    }
}

/// 后台 -> 前台的消息。
enum Msg {
    Progress(ConvertProgress),
    Done(Result<PathBuf, String>),
}

/// 冒烟/截图用环境变量：**只在 debug 构建生效**。
/// release 里忽略，避免把“启动即转换任意文件”这类调试后门带进正式包。
fn debug_env(name: &str) -> Option<String> {
    if cfg!(debug_assertions) {
        std::env::var(name).ok()
    } else {
        None
    }
}

/// 启动页（默认主页）。`RARZAP_START_SCREEN=settings|privacy` 便于截图与冒烟测试。
fn initial_screen() -> Screen {
    match debug_env("RARZAP_START_SCREEN").as_deref() {
        Some("settings") => Screen::Settings,
        Some("privacy") => Screen::Privacy,
        _ => Screen::Main,
    }
}

fn short_name(p: &str) -> String {
    p.rsplit(['/', '\\']).next().unwrap_or(p).to_string()
}

/// 应用主题（System 时按窗口外观取亮/暗）。
pub fn apply_theme(mode: ThemeMode, window: &mut Window, cx: &mut App) {
    let effective = match mode {
        ThemeMode::Light => gpui_kit::component::ThemeMode::Light,
        ThemeMode::Dark => gpui_kit::component::ThemeMode::Dark,
        ThemeMode::System => {
            let dark = matches!(
                window.appearance(),
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            );
            if dark {
                gpui_kit::component::ThemeMode::Dark
            } else {
                gpui_kit::component::ThemeMode::Light
            }
        }
    };
    gpui_kit::component::Theme::change(effective, Some(window), cx);
}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 冒烟/截图：首帧后自动开转（延后一拍，避免在构造期 re-enter）。
        if let Some(path) = self.pending_open.take() {
            tracing::info!(path = %path.display(), "smoke: auto-open scheduled");
            cx.spawn(async move |view, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(400))
                    .await;
                tracing::info!("smoke: timer elapsed, starting conversion");
                cx.update(|cx| {
                    view.update(cx, |v, cx| v.set_file(path, cx)).ok();
                });
            })
            .detach();
        }

        // 先产出 body（借用 &mut self），再取 theme，避免重叠借用 cx。
        // 扩展屏幕优先（商业二进制注入），否则走开源主页/设置/隐私。
        let body: AnyElement = match self.active_ext {
            Some(i) if i < self.extensions.len() => self.extensions[i].render(window, cx),
            _ => match self.screen {
                Screen::Main => self.render_main(window, cx),
                Screen::Settings => self.render_settings(window, cx),
                Screen::Privacy => self.render_privacy(window, cx),
            },
        };
        let title_bar = self.render_title_bar(cx);
        let nav_bar = self.render_nav_bar(window, cx);
        let overlay: Option<AnyElement> = self
            .extensions
            .iter()
            .find_map(|ext| ext.overlay(window, cx));
        let theme = cx.theme();
        let view = cx.entity();

        div()
            .id("root")
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            // 整窗接受系统文件拖放（GPUI 将 OS 拖放转成内部拖放事件）。
            .on_drop::<ExternalPaths>(move |paths, _window, cx| {
                let dropped: Vec<PathBuf> = paths.paths().to_vec();
                view.update(cx, |v, cx| v.on_files_dropped(dropped, cx));
            })
            .child(title_bar)
            .child(nav_bar)
            .child(body)
            // 扩展浮层（模态对话框等）最后绘制，覆盖整窗。
            .children(overlay)
    }
}

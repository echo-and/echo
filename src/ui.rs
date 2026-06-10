mod charts;
pub(crate) mod containers;
mod docker_icons;
mod header;
mod images;
mod networks;
mod settings;
mod sidebar;
mod snapshot;
mod terminal;
mod theme;
mod volumes;

use gpui::*;
#[cfg(target_os = "linux")]
use gpui_component::TitleBar;
use gpui_component::{
    Icon, Root, Sizable as _, StyledExt as _,
    button::Button,
    h_flex,
    resizable::{h_resizable, resizable_panel},
};
use rust_i18n::t;
use std::rc::Rc;

#[cfg(target_os = "linux")]
use crate::app::hide_echo_window_from_window;
#[cfg(target_os = "linux")]
use crate::ui::theme::theme_border;
use crate::{
    app::{AppFontFamily, EchoApp, MAX_CONTAINER_LIST_WIDTH, MIN_CONTAINER_LIST_WIDTH, NavSection},
    ui::{
        containers::{container_list_row_sizes, content_panel, list_panel},
        images::images_page,
        networks::networks_page,
        settings::settings_page,
        sidebar::sidebar,
        snapshot::WorkspaceSnapshot,
        theme::{theme_bg, theme_secondary, theme_text},
        volumes::volumes_page,
    },
};

pub(super) const ICON_SLIDERS_HORIZONTAL: &str = "assets/icons/sliders-horizontal.svg";
const LOGO_GRAY_PLACEHOLDER: &str = "assets/images/logo-gray-placeholder.svg";
const ICON_REFRESH_CW: &str = "assets/icons/refresh-cw.svg";
pub(super) const TOP_BAR_HEIGHT: Pixels = px(46.);

pub(crate) fn apply_echo_theme_overrides(cx: &mut App) {
    theme::apply_echo_theme_overrides(cx);
}

pub(crate) fn apply_echo_font_preference(font_family: AppFontFamily, cx: &mut App) {
    theme::apply_echo_font_preference(font_family, cx);
}

impl Render for EchoApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snapshot = self
            .model
            .read_with(cx, |model, _| WorkspaceSnapshot::from(model));
        self.sync_input_placeholders(snapshot.locale, window, cx);
        let model = self.model.clone();
        let row_sizes = container_list_row_sizes(&snapshot);

        div()
            .id("echo-root")
            .size_full()
            .track_focus(&self.focus_handle)
            .bg(theme_bg(snapshot.theme_mode))
            .text_color(theme_text(snapshot.theme_mode))
            .flex()
            .flex_col()
            .overflow_hidden()
            .children(linux_window_chrome(window, &snapshot))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(sidebar(model.clone(), &snapshot, cx))
                    .child(main_content(self, model, &snapshot, row_sizes, cx)),
            )
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}

#[cfg(target_os = "linux")]
fn linux_window_chrome(window: &mut Window, snapshot: &WorkspaceSnapshot) -> Option<AnyElement> {
    if matches!(window.window_decorations(), Decorations::Client { .. }) {
        Some(
            TitleBar::new()
                .on_close_window(|_, window, cx| hide_echo_window_from_window(window, cx))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .text_sm()
                        .font_medium()
                        .child(t!("app.title").to_string()),
                )
                .into_any_element(),
        )
    } else {
        Some(
            div()
                .h(px(1.))
                .flex_shrink_0()
                .bg(theme_border(snapshot.theme_mode))
                .into_any_element(),
        )
    }
}

#[cfg(not(target_os = "linux"))]
fn linux_window_chrome(_: &mut Window, _: &WorkspaceSnapshot) -> Option<AnyElement> {
    None
}

fn main_content(
    app: &mut EchoApp,
    model: Entity<crate::app::WorkspaceModel>,
    snapshot: &WorkspaceSnapshot,
    row_sizes: Rc<Vec<Size<Pixels>>>,
    cx: &mut Context<EchoApp>,
) -> AnyElement {
    if snapshot.docker_unavailable && snapshot.active_nav != NavSection::Settings {
        return connection_error_page(snapshot, model, cx).into_any_element();
    }

    match snapshot.active_nav {
        NavSection::Containers => {
            let model = model.clone();
            h_resizable("container-layout")
                .with_state(&app.container_layout)
                .child(
                    resizable_panel()
                        .size(px(snapshot.container_list_width as f32))
                        .size_range(
                            px(MIN_CONTAINER_LIST_WIDTH as f32)
                                ..px(MAX_CONTAINER_LIST_WIDTH as f32),
                        )
                        .flex_none()
                        .child(list_panel(app, snapshot, row_sizes.clone(), cx)),
                )
                .child(
                    resizable_panel()
                        .size_range(px(360.)..Pixels::MAX)
                        .min_w_0()
                        .child(content_panel(
                            snapshot,
                            &app.log_filter_input,
                            app.logs_panel.clone(),
                            app.shell_panel.clone(),
                            cx,
                        )),
                )
                .on_resize(move |state, _, cx| {
                    let Some(width) = state.read(cx).sizes().first().copied() else {
                        return;
                    };
                    let width = width.as_f32().round() as u16;
                    model.update(cx, |model, cx| model.set_container_list_width(width, cx));
                })
                .into_any_element()
        }
        NavSection::Settings => settings_page(model, snapshot, cx).into_any_element(),
        NavSection::Images => images_page(app, snapshot, cx).into_any_element(),
        NavSection::Volumes => volumes_page(app, snapshot, cx).into_any_element(),
        NavSection::Networks => networks_page(app, snapshot, cx).into_any_element(),
    }
}

fn connection_error_page(
    snapshot: &WorkspaceSnapshot,
    model: Entity<crate::app::WorkspaceModel>,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let status_text = snapshot
        .reconnect_seconds_remaining
        .map(|seconds| t!("connection.retry_summary", seconds = seconds))
        .unwrap_or_else(|| t!("connection.retrying"));

    div()
        .flex_1()
        .h_full()
        .min_w_0()
        .min_h_0()
        .overflow_hidden()
        .bg(theme_bg(snapshot.theme_mode))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .px(px(24.))
        .child(
            img(LOGO_GRAY_PLACEHOLDER)
                .size(px(96.))
                .object_fit(ObjectFit::Contain),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.))
                .child(
                    div()
                        .text_sm()
                        .font_medium()
                        .text_center()
                        .text_color(theme_text(snapshot.theme_mode))
                        .child(status_text),
                )
                .child(
                    div()
                        .max_w(px(520.))
                        .text_xs()
                        .text_center()
                        .text_color(theme_secondary(snapshot.theme_mode))
                        .child(t!("connection.daemon_hint")),
                ),
        )
        .child(
            Button::new("connection-reconnect")
                .small()
                .mt(px(14.))
                .icon(Icon::new(Icon::empty()).path(ICON_REFRESH_CW))
                .label(t!("connection.retry_now"))
                .on_click(cx.listener(move |_, _, _, cx| {
                    model.update(cx, |model, cx| model.reconnect_active_connection(cx));
                })),
        )
}

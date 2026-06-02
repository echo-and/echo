use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{
    IconName, Sizable, StyledExt as _, ThemeMode,
    button::{Button, ButtonVariants},
    h_flex,
    hover_card::HoverCard,
    input::InputState,
    scroll::ScrollableElement as _,
    tab::{Tab, TabBar},
    v_flex,
};
use rust_i18n::t;

use crate::{
    app::{ContainerDetailTab, EchoApp},
    bridge::ContainerDetailSnapshot,
    domain::ContainerSummary,
    ui::{
        containers::{
            format::{
                format_bytes, format_environment, format_full_environment, format_full_labels,
                format_full_mounts, format_labels, format_mounts, format_ports, format_string_list,
                format_timestamp, format_uptime_value, short_id, unavailable,
            },
            header::container_page_header,
            logs::{ContainerLogsPanel, logs_panel},
            metrics::metrics_overview,
            shell::{ContainerShellPanel, shell_panel},
            style::{metric_card_bg, tab_bar_bg},
        },
        snapshot::WorkspaceSnapshot,
        theme::{theme_border, theme_content_bg, theme_muted, theme_secondary, theme_text},
    },
};

pub(in crate::ui) fn content_panel(
    snapshot: &WorkspaceSnapshot,
    log_filter_input: &Entity<InputState>,
    logs_panel: Entity<ContainerLogsPanel>,
    shell_panel: Entity<ContainerShellPanel>,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    v_flex()
        .id("content-panel")
        .flex_1()
        .h_full()
        .overflow_hidden()
        .bg(theme_content_bg(snapshot.theme_mode))
        .child(container_page_header(snapshot, cx))
        .child(div().flex_1().overflow_hidden().child(content_body(
            snapshot,
            log_filter_input,
            logs_panel,
            shell_panel,
            cx,
        )))
}

fn content_body(
    snapshot: &WorkspaceSnapshot,
    log_filter_input: &Entity<InputState>,
    logs_panel: Entity<ContainerLogsPanel>,
    shell_panel: Entity<ContainerShellPanel>,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    if snapshot.is_loading {
        return detail_placeholder(t!("detail.loading"), snapshot.theme_mode).into_any_element();
    }

    if let Some(error) = &snapshot.error {
        return detail_placeholder(error.clone(), snapshot.theme_mode).into_any_element();
    }

    if let Some(error) = &snapshot.refresh_error {
        return detail_placeholder(error.clone(), snapshot.theme_mode).into_any_element();
    }

    let Some(container) = &snapshot.selected_container else {
        return detail_placeholder(t!("detail.empty_body"), snapshot.theme_mode).into_any_element();
    };

    container_detail_main_middle(
        container,
        snapshot,
        log_filter_input,
        logs_panel,
        shell_panel,
        cx,
    )
    .into_any_element()
}

pub(in crate::ui) fn detail_placeholder(
    text: impl Into<SharedString>,
    theme_mode: ThemeMode,
) -> impl IntoElement {
    div()
        .h_full()
        .items_center()
        .justify_center()
        .px_6()
        .text_center()
        .text_sm()
        .text_color(theme_secondary(theme_mode))
        .child(text.into())
}

fn container_detail_main_middle(
    container: &ContainerSummary,
    snapshot: &WorkspaceSnapshot,
    log_filter_input: &Entity<InputState>,
    logs_panel: Entity<ContainerLogsPanel>,
    shell_panel: Entity<ContainerShellPanel>,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let detail = snapshot
        .container_detail
        .as_ref()
        .filter(|detail| detail.container_id == container.id);
    let vm = ContainerDetailVm::new(container, detail);

    v_flex()
        .id("container-main-middle")
        .size_full()
        .overflow_hidden()
        .when(!snapshot.container_bottom_maximized, |this| {
            this.child(
                v_flex()
                    .flex_none()
                    .w_full()
                    .min_w_0()
                    .overflow_hidden()
                    .p(px(16.))
                    .gap(px(12.))
                    .child(metrics_overview(&vm, detail, snapshot.theme_mode))
                    .child(config_summary(&vm, snapshot.theme_mode)),
            )
        })
        .child(main_bottom(
            snapshot,
            log_filter_input,
            logs_panel,
            shell_panel,
            cx,
        ))
}

fn main_bottom(
    snapshot: &WorkspaceSnapshot,
    log_filter_input: &Entity<InputState>,
    logs_panel_view: Entity<ContainerLogsPanel>,
    shell_panel_view: Entity<ContainerShellPanel>,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let tabs = main_bottom_tabs(snapshot, cx).into_any_element();
    let content = match snapshot.container_detail_tab {
        ContainerDetailTab::Logs => logs_panel(snapshot, log_filter_input, logs_panel_view, cx),
        ContainerDetailTab::Shell => shell_panel(snapshot, shell_panel_view, cx),
    };

    v_flex()
        .id("container-main-bottom")
        .flex_1()
        .min_w_0()
        .min_h(px(220.))
        .border_t_1()
        .border_color(theme_border(snapshot.theme_mode))
        .bg(metric_card_bg(snapshot.theme_mode))
        .overflow_hidden()
        .child(tabs)
        .child(content)
}

fn main_bottom_tabs(snapshot: &WorkspaceSnapshot, cx: &mut Context<EchoApp>) -> impl IntoElement {
    let active_tab = tab_index(snapshot.container_detail_tab);
    let maximized = snapshot.container_bottom_maximized;
    let maximize_button = Button::new("container-bottom-maximize")
        .ghost()
        .icon(if maximized {
            IconName::Minimize
        } else {
            IconName::Maximize
        })
        .tooltip(if maximized {
            t!("detail.restore")
        } else {
            t!("detail.maximize")
        })
        .xsmall()
        .on_click(cx.listener(|app, _, _, cx| {
            app.model
                .update(cx, |model, cx| model.toggle_container_bottom_maximized(cx));
        }));

    h_flex()
        .h(px(28.))
        .max_h(px(28.))
        .flex_none()
        .w_full()
        .items_center()
        .border_color(theme_border(snapshot.theme_mode))
        .bg(tab_bar_bg(snapshot.theme_mode))
        .child(
            div()
                .h(px(28.))
                .max_h(px(28.))
                .w_full()
                .overflow_hidden()
                .child(
                    TabBar::new("container-detail-tabs")
                        .w_full()
                        .with_size(gpui_component::Size::Small)
                        .selected_index(active_tab as usize)
                        .on_click(cx.listener(|app, ix: &usize, _, cx| {
                            let tab = match *ix {
                                0 => ContainerDetailTab::Logs,
                                _ => ContainerDetailTab::Shell,
                            };
                            app.model
                                .update(cx, |model, cx| model.set_container_detail_tab(tab, cx));
                        }))
                        .bg(transparent_black())
                        .child(Tab::new().label(t!("detail.logs").to_string()))
                        .child(Tab::new().label(t!("detail.shell").to_string()))
                        .prefix(div().w(px(16.)))
                        .suffix(
                            div()
                                .h_full()
                                .px(px(8.))
                                .flex()
                                .items_center()
                                .child(maximize_button),
                        ),
                ),
        )
}

fn tab_index(tab: ContainerDetailTab) -> u64 {
    match tab {
        ContainerDetailTab::Logs => 0,
        ContainerDetailTab::Shell => 1,
    }
}

pub(super) struct ContainerDetailVm {
    image: String,
    short_id: String,
    ports: String,
    size: String,
    volumes: String,
    full_volumes: Option<String>,
    environment: String,
    full_environment: Option<String>,
    user: String,
    working_dir: String,
    entrypoint: String,
    command: String,
    created: String,
    labels: String,
    full_labels: Option<String>,
    pub(super) uptime_value: String,
    pub(super) uptime_unit: String,
    pub(super) started_footer: String,
    pub(super) restart_count: String,
    pub(super) restart_policy: String,
}

impl ContainerDetailVm {
    fn new(container: &ContainerSummary, snapshot: Option<&ContainerDetailSnapshot>) -> Self {
        let detail = snapshot.and_then(|snapshot| snapshot.detail.as_ref());

        Self {
            image: detail
                .map(|detail| detail.image.clone())
                .unwrap_or_else(|| container.image.clone()),
            short_id: short_id(
                detail
                    .map(|detail| detail.id.as_str())
                    .unwrap_or(container.id.as_str()),
            ),
            ports: detail
                .map(|detail| format_ports(&detail.ports))
                .unwrap_or_else(|| format_ports(&container.ports)),
            size: detail
                .map(format_container_size)
                .unwrap_or_else(|| unavailable(snapshot)),
            volumes: detail
                .map(format_mounts)
                .unwrap_or_else(|| unavailable(snapshot)),
            full_volumes: detail.and_then(format_full_mounts),
            environment: detail
                .map(format_environment)
                .unwrap_or_else(|| unavailable(snapshot)),
            full_environment: detail.and_then(format_full_environment),
            user: detail
                .and_then(|detail| detail.user.clone())
                .unwrap_or_else(|| "-".to_string()),
            working_dir: detail
                .and_then(|detail| detail.working_dir.clone())
                .unwrap_or_else(|| "-".to_string()),
            entrypoint: detail
                .map(|detail| format_string_list(&detail.entrypoint))
                .unwrap_or_else(|| unavailable(snapshot)),
            command: detail
                .map(|detail| format_string_list(&detail.command))
                .unwrap_or_else(|| unavailable(snapshot)),
            created: detail
                .and_then(|detail| detail.created_at.clone())
                .map(|created| format_timestamp(&created))
                .unwrap_or_else(|| unavailable(snapshot)),
            labels: detail
                .map(format_labels)
                .unwrap_or_else(|| unavailable(snapshot)),
            full_labels: detail.and_then(format_full_labels),
            uptime_value: detail
                .and_then(|detail| detail.started_at.as_ref())
                .map(|started_at| format_uptime_value(started_at).0)
                .unwrap_or_else(|| "--".to_string()),
            uptime_unit: detail
                .and_then(|detail| detail.started_at.as_ref())
                .map(|started_at| format_uptime_value(started_at).1)
                .unwrap_or_default(),
            started_footer: detail
                .and_then(|detail| detail.started_at.as_ref())
                .map(|started_at| {
                    format!("{}\n{}", t!("detail.started"), format_timestamp(started_at))
                })
                .unwrap_or_else(|| unavailable(snapshot)),
            restart_count: detail
                .map(|detail| detail.restart_count.to_string())
                .unwrap_or_else(|| "--".to_string()),
            restart_policy: detail
                .and_then(|detail| detail.restart_policy.clone())
                .map(|policy| format!("{}\n{}", t!("detail.policy"), policy))
                .unwrap_or_else(|| unavailable(snapshot)),
        }
    }
}

fn config_summary(vm: &ContainerDetailVm, theme_mode: ThemeMode) -> impl IntoElement {
    h_flex()
        .w_full()
        .min_w_0()
        .items_start()
        .p(px(12.))
        .rounded(px(4.))
        .border_1()
        .border_color(theme_border(theme_mode))
        .bg(metric_card_bg(theme_mode))
        .overflow_hidden()
        .child(config_column(
            vec![
                ConfigRow::new("image", t!("detail.image").to_string(), vm.image.clone()),
                ConfigRow::new(
                    "container-id",
                    t!("detail.container_id").to_string(),
                    vm.short_id.clone(),
                ),
                ConfigRow::new("ports", t!("detail.ports").to_string(), vm.ports.clone()),
                ConfigRow::new("size", t!("detail.size").to_string(), vm.size.clone()),
                ConfigRow::new(
                    "volumes",
                    t!("detail.volumes").to_string(),
                    vm.volumes.clone(),
                )
                .with_full_text(vm.full_volumes.clone()),
                ConfigRow::new(
                    "environment",
                    t!("detail.environment").to_string(),
                    vm.environment.clone(),
                )
                .with_full_text(vm.full_environment.clone()),
            ],
            theme_mode,
        ))
        .child(
            div()
                .w(px(1.))
                .self_stretch()
                .mx(px(12.))
                .bg(theme_border(theme_mode)),
        )
        .child(config_column(
            vec![
                ConfigRow::new("user", t!("detail.user").to_string(), vm.user.clone()),
                ConfigRow::new(
                    "working-dir",
                    t!("detail.working_dir").to_string(),
                    vm.working_dir.clone(),
                ),
                ConfigRow::new(
                    "entrypoint",
                    t!("detail.entrypoint").to_string(),
                    vm.entrypoint.clone(),
                ),
                ConfigRow::new(
                    "command",
                    t!("detail.command").to_string(),
                    vm.command.clone(),
                ),
                ConfigRow::new(
                    "created",
                    t!("detail.created").to_string(),
                    vm.created.clone(),
                ),
                ConfigRow::new("labels", t!("detail.labels").to_string(), vm.labels.clone())
                    .with_full_text(vm.full_labels.clone()),
            ],
            theme_mode,
        ))
}

#[derive(Clone)]
struct ConfigRow {
    id: &'static str,
    label: String,
    summary: String,
    full_text: Option<String>,
}

impl ConfigRow {
    fn new(id: &'static str, label: String, summary: String) -> Self {
        Self {
            id,
            label,
            summary,
            full_text: None,
        }
    }

    fn with_full_text(mut self, full_text: Option<String>) -> Self {
        self.full_text = full_text;
        self
    }
}

fn config_column(rows: Vec<ConfigRow>, theme_mode: ThemeMode) -> impl IntoElement {
    v_flex()
        .flex_1()
        .min_w_0()
        .gap(px(6.))
        .children(rows.into_iter().map(move |row| {
            let value = config_value(row.clone(), theme_mode).into_any_element();
            h_flex()
                .items_start()
                .gap(px(20.))
                .child(
                    div()
                        .w(px(100.))
                        .flex_shrink_0()
                        .text_xs()
                        .line_height(relative(1.35))
                        .text_color(theme_muted(theme_mode))
                        .child(row.label),
                )
                .child(value)
        }))
}

fn config_value(row: ConfigRow, theme_mode: ThemeMode) -> impl IntoElement {
    let value = config_value_text(row.summary.clone(), theme_mode);
    div()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .child(match row.full_text.clone() {
            Some(full_text) => HoverCard::new(format!("container-config-{}-hover", row.id))
                .anchor(Anchor::TopLeft)
                .trigger(value)
                .open_delay(std::time::Duration::from_millis(300))
                .child(config_hover_card_content(
                    row.label.clone(),
                    full_text,
                    theme_mode,
                ))
                .into_any_element(),
            None => value.into_any_element(),
        })
}

fn config_value_text(value: String, theme_mode: ThemeMode) -> impl IntoElement {
    div()
        .w_full()
        .min_w_0()
        .text_xs()
        .line_height(relative(1.35))
        .text_color(theme_text(theme_mode))
        .truncate()
        .child(value)
}

fn config_hover_card_content(
    label: String,
    full_text: String,
    theme_mode: ThemeMode,
) -> impl IntoElement {
    let copy_value = full_text.clone();

    v_flex()
        .w(px(460.))
        .max_w(px(460.))
        .max_h(px(300.))
        .gap(px(8.))
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .child(
                    div()
                        .min_w_0()
                        .text_xs()
                        .font_semibold()
                        .line_height(relative(1.2))
                        .text_color(theme_text(theme_mode))
                        .truncate()
                        .child(label),
                )
                .child(
                    Button::new("container-config-copy")
                        .ghost()
                        .icon(IconName::Copy)
                        .tooltip(t!("detail.copy_config_value"))
                        .xsmall()
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy_value.clone()));
                        }),
                ),
        )
        .child(
            div().w_full().max_h(px(250.)).overflow_y_scrollbar().child(
                div()
                    .w_full()
                    .pr(px(8.))
                    .font_family("Menlo")
                    .text_xs()
                    .line_height(relative(1.45))
                    .whitespace_normal()
                    .text_color(theme_text(theme_mode))
                    .child(full_text),
            ),
        )
}

fn format_container_size(detail: &crate::domain::ContainerDetail) -> String {
    match (detail.size_rw_bytes, detail.size_root_fs_bytes) {
        (Some(writable), Some(rootfs)) => {
            format!(
                "{} (virtual {})",
                format_bytes(writable),
                format_bytes(rootfs)
            )
        }
        (Some(writable), None) => format_bytes(writable),
        (None, Some(rootfs)) => format!("virtual {}", format_bytes(rootfs)),
        (None, None) => "-".to_string(),
    }
}

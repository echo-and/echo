use std::{ops::Range, rc::Rc};

use gpui::*;
use gpui_component::{
    Icon, IconName, Sizable, ThemeMode, VirtualListScrollHandle,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    scroll::ScrollableElement as _,
    v_flex, v_virtual_list,
};
use rust_i18n::t;

use crate::{
    app::EchoApp,
    bridge::ContainerLogsStatus,
    domain::ContainerLogLine,
    ui::{
        containers::style::{log_toolbar_bg, theme_log_text},
        snapshot::WorkspaceSnapshot,
        theme::{theme_border, theme_content_bg, theme_muted, theme_secondary, theme_text},
    },
};

const ICON_TRASH_2: &str = "assets/icons/trash-2.svg";

pub(super) fn logs_panel(
    snapshot: &WorkspaceSnapshot,
    log_filter_input: &Entity<InputState>,
    logs_panel: Entity<ContainerLogsPanel>,
    cx: &mut Context<EchoApp>,
) -> AnyElement {
    logs_panel.update(cx, |panel, cx| {
        if panel.sync_from_snapshot(snapshot) {
            cx.notify();
        }
    });

    v_flex()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .p(px(12.))
        .gap(px(10.))
        .bg(theme_content_bg(snapshot.theme_mode))
        .child(logs_panel.clone())
        .child(logs_toolbar(snapshot, log_filter_input, logs_panel, cx))
        .into_any_element()
}

pub struct ContainerLogsPanel {
    container_id: Option<String>,
    lines: Rc<Vec<LogLineVm>>,
    visible_lines: Rc<Vec<usize>>,
    line_sizes: Rc<Vec<Size<Pixels>>>,
    filter: String,
    logs_revision: u64,
    last_applied_revision: u64,
    copy_cache_revision: u64,
    copy_cache: String,
    status: Option<ContainerLogsStatus>,
    error: Option<String>,
    theme_mode: ThemeMode,
    scroll_handle: VirtualListScrollHandle,
}

#[derive(Clone)]
struct LogLineVm {
    line: ContainerLogLine,
    searchable_lower: String,
}

impl ContainerLogsPanel {
    pub fn new() -> Self {
        Self {
            container_id: None,
            lines: Rc::new(Vec::new()),
            visible_lines: Rc::new(Vec::new()),
            line_sizes: Rc::new(Vec::new()),
            filter: String::new(),
            logs_revision: 0,
            last_applied_revision: u64::MAX,
            copy_cache_revision: u64::MAX,
            copy_cache: String::new(),
            status: None,
            error: None,
            theme_mode: ThemeMode::default(),
            scroll_handle: VirtualListScrollHandle::new(),
        }
    }

    fn sync_from_snapshot(&mut self, snapshot: &WorkspaceSnapshot) -> bool {
        let previous_theme_mode = self.theme_mode;
        self.theme_mode = snapshot.theme_mode;
        self.filter = snapshot.container_log_filter.trim().to_lowercase();
        self.logs_revision = snapshot.container_logs_revision;

        let logs = snapshot
            .container_logs
            .as_ref()
            .filter(|logs| snapshot.selected_container_id.as_deref() == Some(&logs.container_id));
        let Some(logs) = logs else {
            let changed = self.container_id.is_some()
                || !self.lines.is_empty()
                || self.status.is_some()
                || previous_theme_mode != self.theme_mode;
            if self.container_id.is_some() || !self.lines.is_empty() {
                self.reset();
            }
            self.status = None;
            return changed;
        };

        let status_changed = self.status != Some(logs.status) || self.error != logs.error;
        let needs_rebuild = self.container_id.as_deref() != Some(&logs.container_id)
            || self.last_applied_revision != snapshot.container_logs_revision;

        self.status = Some(logs.status);
        self.error = logs.error.clone();

        if !needs_rebuild {
            return status_changed || previous_theme_mode != self.theme_mode;
        }

        self.container_id = Some(logs.container_id.clone());
        self.lines = Rc::new(
            logs.lines
                .iter()
                .cloned()
                .map(LogLineVm::new)
                .collect::<Vec<_>>(),
        );
        self.rebuild_visible_lines();
        self.last_applied_revision = snapshot.container_logs_revision;
        self.copy_cache_revision = u64::MAX;

        if status_changed || !self.visible_lines.is_empty() {
            self.scroll_to_latest_visible_line();
        }
        true
    }

    fn reset(&mut self) {
        self.container_id = None;
        self.lines = Rc::new(Vec::new());
        self.visible_lines = Rc::new(Vec::new());
        self.line_sizes = Rc::new(Vec::new());
        self.copy_cache.clear();
        self.copy_cache_revision = u64::MAX;
        self.last_applied_revision = u64::MAX;
    }

    fn rebuild_visible_lines(&mut self) {
        let visible_lines = filtered_log_line_indices(&self.lines, &self.filter);
        self.line_sizes = Rc::new(
            visible_lines
                .iter()
                .map(|_| size(px(1.), px(17.)))
                .collect(),
        );
        self.visible_lines = Rc::new(visible_lines);
    }

    fn scroll_to_latest_visible_line(&self) {
        if self.visible_lines.is_empty() {
            return;
        }

        self.scroll_handle.scroll_to_item(
            self.visible_lines.len().saturating_sub(1),
            ScrollStrategy::Top,
        );
    }

    fn copy_text(&mut self) -> String {
        if self.copy_cache_revision != self.logs_revision {
            self.copy_cache = self
                .visible_lines
                .iter()
                .map(|index| self.lines[*index].line.message.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            self.copy_cache_revision = self.logs_revision;
        }
        self.copy_cache.clone()
    }
}

impl Render for ContainerLogsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.status.is_none() || self.status == Some(ContainerLogsStatus::Loading) {
            return tab_placeholder(t!("detail.logs_loading"), self.theme_mode).into_any_element();
        }

        if self.status == Some(ContainerLogsStatus::Error) {
            return tab_placeholder(
                self.error
                    .clone()
                    .unwrap_or_else(|| t!("detail.logs_unavailable").to_string()),
                self.theme_mode,
            )
            .into_any_element();
        }

        if self.lines.is_empty() {
            let message = if self.status == Some(ContainerLogsStatus::Stopped) {
                t!("detail.logs_no_lines")
            } else {
                t!("detail.logs_loading")
            };
            return tab_placeholder(message, self.theme_mode).into_any_element();
        }

        if self.visible_lines.is_empty() {
            return tab_placeholder(t!("detail.logs_empty"), self.theme_mode).into_any_element();
        }

        let lines = self.lines.clone();
        let visible_lines = self.visible_lines.clone();
        let theme_mode = self.theme_mode;

        div()
            .id("container-logs-scroll")
            .w_full()
            .min_w_0()
            .flex_1()
            .min_h_0()
            .relative()
            .child(
                v_virtual_list(
                    cx.entity().clone(),
                    "container-log-lines",
                    self.line_sizes.clone(),
                    move |_, range: Range<usize>, _, _| {
                        range
                            .map(|index| {
                                let line_index = visible_lines[index];
                                log_line(&lines[line_index].line, theme_mode)
                            })
                            .collect::<Vec<_>>()
                    },
                )
                .track_scroll(&self.scroll_handle)
                .gap(px(4.)),
            )
            .scrollbar(&self.scroll_handle, Axis::Vertical)
            .into_any_element()
    }
}

impl LogLineVm {
    fn new(line: ContainerLogLine) -> Self {
        let searchable_lower = line.searchable_text().to_lowercase();
        Self {
            line,
            searchable_lower,
        }
    }
}

fn filtered_log_line_indices(lines: &[LogLineVm], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..lines.len()).collect();
    }

    lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| line.searchable_lower.contains(query).then_some(index))
        .collect()
}

fn log_line(line: &ContainerLogLine, theme_mode: ThemeMode) -> AnyElement {
    h_flex()
        .h(px(17.))
        .overflow_hidden()
        .items_start()
        .text_size(px(12.))
        .line_height(px(17.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(theme_log_text(theme_mode))
                .child(line.message.clone()),
        )
        .into_any_element()
}

fn logs_toolbar(
    snapshot: &WorkspaceSnapshot,
    log_filter_input: &Entity<InputState>,
    logs_panel: Entity<ContainerLogsPanel>,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let filter = log_filter_input_element(snapshot, log_filter_input).into_any_element();
    let clear_button = Button::new("container-logs-clear")
        .ghost()
        .icon(Icon::new(Icon::empty()).path(ICON_TRASH_2))
        .tooltip(t!("detail.clear_logs"))
        .xsmall()
        .on_click(cx.listener(|app, _, _, cx| {
            app.model
                .update(cx, |model, cx| model.clear_container_logs(cx));
        }))
        .into_any_element();
    let log_text = logs_panel.update(cx, |panel, _| panel.copy_text());
    let copy_button = copy_logs_button(log_text).into_any_element();

    h_flex()
        .h(px(40.))
        .w_full()
        .min_w_0()
        .items_center()
        .justify_between()
        .p(px(8.))
        .rounded(px(4.))
        .border_1()
        .border_color(theme_border(snapshot.theme_mode))
        .bg(log_toolbar_bg(snapshot.theme_mode))
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap(px(8.))
                .child(filter),
        )
        .child(
            h_flex()
                .flex_shrink_0()
                .items_center()
                .gap(px(6.))
                .child(clear_button)
                .child(copy_button),
        )
}

fn log_filter_input_element(
    snapshot: &WorkspaceSnapshot,
    log_filter_input: &Entity<InputState>,
) -> impl IntoElement {
    Input::new(log_filter_input)
        .tab_index(-1)
        .prefix(
            Icon::new(IconName::Search)
                .xsmall()
                .text_color(theme_muted(snapshot.theme_mode)),
        )
        .small()
        .w(px(180.))
        .max_w_full()
        .text_color(theme_text(snapshot.theme_mode))
}

pub(super) fn tab_placeholder(text: impl Into<SharedString>, theme_mode: ThemeMode) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .items_center()
        .justify_center()
        .px_4()
        .text_sm()
        .line_height(relative(1.35))
        .bg(theme_content_bg(theme_mode))
        .text_color(theme_secondary(theme_mode))
        .child(text.into())
        .into_any_element()
}

fn copy_logs_button(value: String) -> impl IntoElement {
    Button::new("container-logs-copy")
        .ghost()
        .icon(IconName::Copy)
        .tooltip(t!("detail.copy_logs"))
        .xsmall()
        .on_click(move |_, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(value.clone()));
        })
}

#[cfg(test)]
mod tests {
    use crate::domain::{ContainerLogLine, ContainerLogStreamKind};

    use super::{LogLineVm, filtered_log_line_indices};

    #[test]
    fn filters_log_lines_with_cached_lowercase_text() {
        let lines = vec![
            LogLineVm::new(ContainerLogLine::new(
                None,
                ContainerLogStreamKind::Stdout,
                "Server Ready".to_string(),
            )),
            LogLineVm::new(ContainerLogLine::new(
                None,
                ContainerLogStreamKind::Stderr,
                "background worker".to_string(),
            )),
        ];

        assert_eq!(filtered_log_line_indices(&lines, ""), vec![0, 1]);
        assert_eq!(filtered_log_line_indices(&lines, "ready"), vec![0]);
        assert_eq!(
            filtered_log_line_indices(&lines, "WORKER"),
            Vec::<usize>::new()
        );
    }
}

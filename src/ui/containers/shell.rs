use std::collections::HashMap;

use gpui::*;
use gpui_component::{ThemeMode, v_flex};
use rust_i18n::t;
use tokio::sync::watch;

use crate::{
    app::{EchoApp, WorkspaceModel},
    bridge::{ContainerShellSnapshot, ContainerShellStatus},
    ui::{
        containers::{detail::detail_placeholder, style::theme_log_text},
        snapshot::WorkspaceSnapshot,
        terminal::{ColorPalette, TerminalConfig, TerminalView},
        theme::theme_content_bg,
    },
};

const TERMINAL_FONT_SIZE: f32 = 12.;
const DEFAULT_TERMINAL_COLS: usize = 100;
const DEFAULT_TERMINAL_ROWS: usize = 28;

pub(super) fn shell_panel(
    snapshot: &WorkspaceSnapshot,
    shell_panel: Entity<ContainerShellPanel>,
    cx: &mut Context<EchoApp>,
) -> AnyElement {
    shell_panel.update(cx, |panel, cx| {
        if panel.sync_from_snapshot(snapshot, cx) {
            cx.notify();
        }
    });

    v_flex()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .p(px(12.))
        .bg(theme_content_bg(snapshot.theme_mode))
        .child(shell_panel)
        .into_any_element()
}

pub struct ContainerShellPanel {
    model: WeakEntity<WorkspaceModel>,
    container_id: Option<String>,
    terminals: HashMap<String, ShellTerminalEntry>,
    status: Option<ContainerShellStatus>,
    error: Option<String>,
    theme_mode: ThemeMode,
    auto_focus_container_id: Option<String>,
}

struct ShellTerminalEntry {
    terminal: Entity<TerminalView>,
    _status_rx: watch::Receiver<ContainerShellSnapshot>,
}

impl ContainerShellPanel {
    pub fn new(model: Entity<WorkspaceModel>, _cx: &mut Context<Self>) -> Self {
        Self {
            model: model.downgrade(),
            container_id: None,
            terminals: HashMap::new(),
            status: None,
            error: None,
            theme_mode: ThemeMode::default(),
            auto_focus_container_id: None,
        }
    }

    fn sync_from_snapshot(&mut self, snapshot: &WorkspaceSnapshot, cx: &mut Context<Self>) -> bool {
        let previous_container_id = self.container_id.clone();
        let previous_status = self.status;
        let previous_error = self.error.clone();
        let previous_theme = self.theme_mode;

        self.theme_mode = snapshot.theme_mode;
        if previous_theme != self.theme_mode {
            let colors = terminal_palette(self.theme_mode);
            for terminal in self.terminals.values() {
                terminal.terminal.update(cx, |terminal, cx| {
                    terminal.update_colors(colors.clone(), cx)
                });
            }
        }

        let shell = snapshot
            .container_shell
            .as_ref()
            .filter(|shell| snapshot.selected_container_id.as_deref() == Some(&shell.container_id));

        if let Some(shell) = shell {
            self.container_id = Some(shell.container_id.clone());
            self.status = Some(shell.status);
            self.error = shell.error.clone();
        } else {
            self.container_id = snapshot.selected_container_id.clone();
            self.status = None;
            self.error = None;
        }

        if self.status == Some(ContainerShellStatus::Live) {
            self.ensure_terminal(cx);
        }

        previous_container_id != self.container_id
            || previous_status != self.status
            || previous_error != self.error
            || previous_theme != self.theme_mode
    }

    fn ensure_terminal(&mut self, cx: &mut Context<Self>) {
        let Some(container_id) = self.container_id.clone() else {
            return;
        };

        if self.terminals.contains_key(&container_id) {
            return;
        }

        let Some(model) = self.model.upgrade() else {
            self.status = Some(ContainerShellStatus::Error);
            self.error = Some(t!("detail.shell_unavailable").to_string());
            return;
        };

        let session = match model.update(cx, |model, _| model.open_selected_container_shell()) {
            Ok(session) => session,
            Err(error) => {
                self.status = Some(ContainerShellStatus::Error);
                self.error = Some(error.to_string());
                return;
            }
        };

        let status_rx = session.status_rx.clone();
        let resizer = session.resizer();
        let config = terminal_config(self.theme_mode);
        let terminal = cx.new(|cx| {
            TerminalView::new(session.writer, session.reader, config, cx).with_resize_callback(
                move |cols, rows| {
                    resizer.resize(cols as u16, rows as u16);
                },
            )
        });

        self.terminals.insert(
            container_id,
            ShellTerminalEntry {
                terminal,
                _status_rx: status_rx,
            },
        );
    }
}

impl Render for ContainerShellPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.status.is_none() || self.status == Some(ContainerShellStatus::Loading) {
            return detail_placeholder(t!("detail.shell_connecting"), self.theme_mode)
                .into_any_element();
        }

        if self.status == Some(ContainerShellStatus::Stopped) {
            return detail_placeholder(t!("detail.shell_stopped"), self.theme_mode)
                .into_any_element();
        }

        if self.status == Some(ContainerShellStatus::Error) {
            return detail_placeholder(
                self.error
                    .clone()
                    .unwrap_or_else(|| t!("detail.shell_unavailable").to_string()),
                self.theme_mode,
            )
            .into_any_element();
        }

        let Some(container_id) = &self.container_id else {
            return detail_placeholder(t!("detail.shell_connecting"), self.theme_mode)
                .into_any_element();
        };
        let Some(terminal) = self
            .terminals
            .get(container_id)
            .map(|entry| entry.terminal.clone())
        else {
            return detail_placeholder(t!("detail.shell_connecting"), self.theme_mode)
                .into_any_element();
        };

        if let Some(container_id) = &self.container_id
            && self.auto_focus_container_id.as_ref() != Some(container_id)
        {
            self.auto_focus_container_id = Some(container_id.clone());
            terminal.update(cx, |terminal, cx| terminal.focus(window, cx));
        }

        let theme_mode = self.theme_mode;

        div()
            .id("container-shell-terminal")
            .w_full()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .bg(terminal_bg(theme_mode))
            .child(terminal)
            .into_any_element()
    }
}

fn terminal_config(theme_mode: ThemeMode) -> TerminalConfig {
    TerminalConfig {
        cols: DEFAULT_TERMINAL_COLS,
        rows: DEFAULT_TERMINAL_ROWS,
        font_family: "Menlo".into(),
        font_size: px(TERMINAL_FONT_SIZE),
        scrollback: 10_000,
        line_height_multiplier: 1.16,
        padding: Edges::all(px(0.)),
        colors: terminal_palette(theme_mode),
    }
}

fn terminal_palette(theme_mode: ThemeMode) -> ColorPalette {
    ColorPalette::default().with_base(
        terminal_text(theme_mode),
        terminal_bg(theme_mode),
        terminal_text(theme_mode),
    )
}

fn terminal_bg(theme_mode: ThemeMode) -> Hsla {
    theme_content_bg(theme_mode)
}

fn terminal_text(theme_mode: ThemeMode) -> Hsla {
    theme_log_text(theme_mode)
}

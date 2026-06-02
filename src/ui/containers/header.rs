use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{
    Disableable, Icon, Sizable, ThemeMode,
    button::{Button, ButtonVariants},
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
};
use rust_i18n::t;

use crate::{
    app::EchoApp,
    bridge::ContainerAction,
    domain::{ContainerPortSummary, ContainerSummary},
    ui::{header::page_header, snapshot::WorkspaceSnapshot},
};

const ICON_CHEVRONS_LEFT_RIGHT: &str = "assets/icons/chevrons-left-right.svg";
const ICON_COPY: &str = "assets/icons/copy.svg";
const ICON_ELLIPSIS: &str = "assets/icons/ellipsis.svg";
const ICON_HEART: &str = "assets/icons/heart.svg";
const ICON_PAUSE: &str = "assets/icons/pause.svg";
const ICON_PLAY: &str = "assets/icons/play.svg";
const ICON_REFRESH_CW: &str = "assets/icons/refresh-cw.svg";
const ICON_ROTATE_CW: &str = "assets/icons/rotate-cw.svg";
const ICON_SQUARE: &str = "assets/icons/square.svg";
const ICON_TRASH_2: &str = "assets/icons/trash-2.svg";

pub(in crate::ui) fn container_page_header(
    snapshot: &WorkspaceSnapshot,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    page_header(
        container_page_title(snapshot),
        Some(container_header_actions(snapshot, cx).into_any_element()),
        snapshot.theme_mode,
    )
}

fn container_page_title(snapshot: &WorkspaceSnapshot) -> SharedString {
    snapshot
        .selected_container
        .as_ref()
        .map(|container| container.name.clone())
        .unwrap_or_else(|| t!("detail.empty_title").to_string())
        .into()
}

fn container_header_actions(
    snapshot: &WorkspaceSnapshot,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    h_flex().items_center().gap(px(10.)).when_some(
        snapshot.selected_container.clone(),
        |this, container| {
            this.child(container_status_bar(&container, snapshot.theme_mode))
                .child(container_actions(container, snapshot, cx))
        },
    )
}

fn container_status_bar(
    container: &ContainerSummary,
    theme_mode: ThemeMode,
) -> impl IntoElement + use<> {
    let status = ContainerStatusVm::from(container);
    let port_targets = openable_port_targets(&container.ports);

    h_flex()
        .items_center()
        .child(status_chip(
            StatusIcon::Dot(status.running_color(theme_mode)),
            status.state_label,
            theme_mode,
        ))
        .when_some(status.health_label, |this, health| {
            this.child(status_chip(
                StatusIcon::Path(ICON_HEART),
                health,
                theme_mode,
            ))
        })
        .when_some(status.port_label, |this, port| {
            this.child(port_status_chip(port, port_targets, theme_mode))
        })
}

fn status_chip(
    icon: StatusIcon,
    label: impl Into<SharedString>,
    theme_mode: ThemeMode,
) -> impl IntoElement {
    h_flex()
        .items_center()
        .gap(px(6.))
        .px(px(12.))
        .py(px(6.))
        .rounded(px(4.))
        .overflow_hidden()
        .child(match icon {
            StatusIcon::Dot(color) => div()
                .size(px(6.))
                .rounded_full()
                .bg(color)
                .into_any_element(),
            StatusIcon::Path(path) => Icon::new(Icon::empty())
                .path(path)
                .with_size(gpui_component::Size::XSmall)
                .text_color(theme_text_color(theme_mode))
                .into_any_element(),
        })
        .child(
            div()
                .text_xs()
                .line_height(relative(1.2))
                .text_color(theme_text_color(theme_mode))
                .child(label.into()),
        )
}

fn port_status_chip(
    label: SharedString,
    targets: Vec<PortOpenTarget>,
    theme_mode: ThemeMode,
) -> impl IntoElement {
    let chip = status_chip(
        StatusIcon::Path(ICON_CHEVRONS_LEFT_RIGHT),
        label,
        theme_mode,
    );

    match targets.as_slice() {
        [] => chip.into_any_element(),
        [target] => Button::new("container-port-open")
            .ghost()
            .tooltip(format!("Open {}", target.url))
            .p(px(4.))
            .child(chip)
            .on_click({
                let url = target.url.clone();
                move |_, _, cx| cx.open_url(&url)
            })
            .into_any_element(),
        _ => Button::new("container-port-open")
            .ghost()
            .tooltip("Open port")
            .p(px(4.))
            .child(chip)
            .dropdown_menu_with_anchor(Anchor::BottomRight, move |menu, _, _| {
                targets.iter().fold(menu.min_w(220.), |menu, target| {
                    menu.item(PopupMenuItem::link(target.menu_label(), target.url.clone()))
                })
            })
            .into_any_element(),
    }
}

fn container_actions(
    container: ContainerSummary,
    snapshot: &WorkspaceSnapshot,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let state = container.state.as_deref().unwrap_or_default();
    let is_running = state.eq_ignore_ascii_case("running");
    let is_paused = state.eq_ignore_ascii_case("paused");
    let has_selection = !container.id.is_empty();
    let pending = snapshot
        .pending_container_action
        .as_ref()
        .is_some_and(|pending| pending.container_id == container.id);

    let primary_action = if is_running {
        ContainerAction::Stop
    } else {
        ContainerAction::Start
    };
    let primary_icon = if is_running { ICON_SQUARE } else { ICON_PLAY };
    let primary_label = if is_running {
        t!("actions.stop_container")
    } else {
        t!("actions.start_container")
    };

    let pause_action = if is_paused {
        ContainerAction::Unpause
    } else {
        ContainerAction::Pause
    };
    let pause_label = if is_paused {
        t!("actions.resume_container")
    } else {
        t!("actions.pause_container")
    };
    let pause_icon = if is_paused { ICON_PLAY } else { ICON_PAUSE };

    h_flex()
        .items_center()
        .gap(px(4.))
        .when(!is_paused, |this| {
            this.child(action_button(
                ContainerActionButton {
                    id: "container-primary-action".into(),
                    icon_path: primary_icon,
                    tooltip: primary_label.into(),
                    container_id: container.id.clone(),
                    action: primary_action,
                    enabled: has_selection && !pending,
                    loading: pending_action_matches(snapshot, &container.id, primary_action),
                },
                cx,
            ))
        })
        .child(action_button(
            ContainerActionButton {
                id: "container-restart-action".into(),
                icon_path: ICON_ROTATE_CW,
                tooltip: t!("actions.restart_container").into(),
                container_id: container.id.clone(),
                action: ContainerAction::Restart,
                enabled: has_selection && (is_running || is_paused) && !pending,
                loading: pending_action_matches(snapshot, &container.id, ContainerAction::Restart),
            },
            cx,
        ))
        .child(action_button(
            ContainerActionButton {
                id: "container-pause-action".into(),
                icon_path: pause_icon,
                tooltip: pause_label.into(),
                container_id: container.id.clone(),
                action: pause_action,
                enabled: has_selection && (is_running || is_paused) && !pending,
                loading: pending_action_matches(snapshot, &container.id, pause_action),
            },
            cx,
        ))
        .child(more_menu_button(
            container,
            pending,
            is_running || is_paused,
            cx,
        ))
}

struct ContainerActionButton {
    id: ElementId,
    icon_path: &'static str,
    tooltip: SharedString,
    container_id: String,
    action: ContainerAction,
    enabled: bool,
    loading: bool,
}

fn action_button(config: ContainerActionButton, cx: &mut Context<EchoApp>) -> impl IntoElement {
    let icon = Icon::new(Icon::empty()).path(config.icon_path);

    Button::new(config.id)
        .ghost()
        .icon(icon)
        .tooltip(config.tooltip)
        .disabled(!config.enabled)
        .loading(config.loading)
        .on_click(cx.listener(move |app, _, _, cx| {
            app.control_container(config.container_id.clone(), config.action, cx);
        }))
}

fn more_menu_button(
    container: ContainerSummary,
    pending: bool,
    protected: bool,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let app = cx.entity().downgrade();
    let refresh_id = container.id.clone();
    let copy_id = container.id.clone();
    let copy_name = container.name.clone();
    let remove_id = container.id.clone();
    let button = Button::new("container-more-action")
        .ghost()
        .icon(Icon::new(Icon::empty()).path(ICON_ELLIPSIS))
        .tooltip(t!("actions.more"))
        .disabled(pending);

    button.dropdown_menu_with_anchor(Anchor::BottomRight, move |menu, _, _| {
        let id = copy_id.clone();
        let name = copy_name.clone();
        let refresh_id = refresh_id.clone();
        let remove_id = remove_id.clone();
        let remove_app = app.clone();

        menu.min_w(180.)
            .item(
                PopupMenuItem::new(t!("actions.copy_container_id"))
                    .icon(Icon::new(Icon::empty()).path(ICON_COPY))
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(id.clone()));
                    }),
            )
            .item(
                PopupMenuItem::new(t!("actions.copy_container_name"))
                    .icon(Icon::new(Icon::empty()).path(ICON_COPY))
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(name.clone()));
                    }),
            )
            .separator()
            .item(
                PopupMenuItem::new(t!("actions.refresh_status"))
                    .icon(Icon::new(Icon::empty()).path(ICON_REFRESH_CW))
                    .on_click({
                        let app = app.clone();
                        move |_, _, cx| {
                            let _ = app.update(cx, |app, cx| {
                                app.model.update(cx, |model, cx| {
                                    if model
                                        .containers
                                        .iter()
                                        .any(|container| container.id == refresh_id)
                                    {
                                        model.refresh_containers(true, cx);
                                    }
                                });
                            });
                        }
                    }),
            )
            .separator()
            .item(
                PopupMenuItem::new(t!("actions.remove_container"))
                    .icon(Icon::new(Icon::empty()).path(ICON_TRASH_2))
                    .disabled(protected)
                    .on_click(move |_, _, cx| {
                        let _ = remove_app.update(cx, |app, cx| {
                            app.control_container(remove_id.clone(), ContainerAction::Remove, cx);
                        });
                    }),
            )
    })
}

fn pending_action_matches(
    snapshot: &WorkspaceSnapshot,
    container_id: &str,
    action: ContainerAction,
) -> bool {
    snapshot
        .pending_container_action
        .as_ref()
        .is_some_and(|pending| pending.container_id == container_id && pending.action == action)
}

enum StatusIcon {
    Dot(Hsla),
    Path(&'static str),
}

struct ContainerStatusVm {
    state_label: SharedString,
    health_label: Option<SharedString>,
    port_label: Option<SharedString>,
    state: String,
}

impl From<&ContainerSummary> for ContainerStatusVm {
    fn from(container: &ContainerSummary) -> Self {
        let state = container
            .state
            .as_deref()
            .filter(|state| !state.is_empty())
            .unwrap_or("unknown");

        Self {
            state_label: state_label(state).into(),
            health_label: container
                .health
                .as_deref()
                .map(health_label)
                .map(SharedString::from),
            port_label: port_label(container).map(SharedString::from),
            state: state.to_string(),
        }
    }
}

impl ContainerStatusVm {
    fn running_color(&self, theme_mode: ThemeMode) -> Hsla {
        if self.state.eq_ignore_ascii_case("running") {
            hsla(0.441, 0.760, 0.520, 1.0)
        } else if self.state.eq_ignore_ascii_case("paused") {
            hsla(0.128, 0.850, 0.560, 1.0)
        } else if self.state.eq_ignore_ascii_case("unknown") {
            theme_muted_color(theme_mode)
        } else {
            hsla(0.0, 0.820, 0.620, 1.0)
        }
    }
}

fn state_label(state: &str) -> String {
    let mut chars = state.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => t!("status.unknown").to_string(),
    }
}

fn health_label(health: &str) -> String {
    state_label(health)
}

fn port_label(container: &ContainerSummary) -> Option<String> {
    let mut ports = container
        .ports
        .iter()
        .map(|port| port.public_port.unwrap_or(port.private_port))
        .collect::<Vec<_>>();
    ports.sort_unstable();
    ports.dedup();

    match ports.as_slice() {
        [] => None,
        [port] => Some(format!("Port {port}")),
        [port, rest @ ..] => Some(format!("Ports {port}, +{}", rest.len())),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PortOpenTarget {
    private_port: u16,
    public_port: u16,
    protocol: String,
    url: String,
}

impl PortOpenTarget {
    fn new(port: &ContainerPortSummary) -> Option<Self> {
        if !port
            .protocol
            .as_deref()
            .is_none_or(|protocol| protocol.eq_ignore_ascii_case("tcp"))
        {
            return None;
        }

        let public_port = port.public_port?;
        let protocol = port.protocol.clone().unwrap_or_else(|| "tcp".to_string());

        Some(Self {
            private_port: port.private_port,
            public_port,
            protocol,
            url: format!("{}://localhost:{public_port}", web_scheme(public_port)),
        })
    }

    fn menu_label(&self) -> String {
        format!(
            "localhost:{} -> {}/{}",
            self.public_port, self.private_port, self.protocol
        )
    }
}

fn web_scheme(port: u16) -> &'static str {
    if matches!(port, 443 | 8443) {
        "https"
    } else {
        "http"
    }
}

fn openable_port_targets(ports: &[ContainerPortSummary]) -> Vec<PortOpenTarget> {
    let mut targets = ports
        .iter()
        .filter_map(PortOpenTarget::new)
        .collect::<Vec<_>>();
    targets.sort_by_key(|target| target.public_port);
    targets.dedup_by_key(|target| target.public_port);
    targets
}

fn theme_text_color(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        white()
    } else {
        hsla(0.0, 0.0, 0.098, 1.0)
    }
}

fn theme_muted_color(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        hsla(0.0, 0.0, 0.580, 1.0)
    } else {
        hsla(0.0, 0.0, 0.427, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{ContainerPortSummary, ContainerSummary};

    use super::{ContainerStatusVm, openable_port_targets, port_label, web_scheme};

    #[test]
    fn maps_running_container_status() {
        let container = ContainerSummary::new(
            "1234567890abcdef".to_string(),
            "postgres".to_string(),
            "postgres:latest".to_string(),
            "Up 2 hours".to_string(),
            Some("running".to_string()),
            Some("healthy".to_string()),
            vec![ContainerPortSummary::new(
                5432,
                Some(15432),
                Some("tcp".to_string()),
            )],
            false,
        );

        let status = ContainerStatusVm::from(&container);

        assert_eq!(status.state_label.as_ref(), "Running");
        assert_eq!(
            status.health_label.as_ref().map(|label| label.as_ref()),
            Some("Healthy")
        );
        assert_eq!(
            status.port_label.as_ref().map(|label| label.as_ref()),
            Some("Port 15432")
        );
    }

    #[test]
    fn hides_missing_health_status() {
        let container = ContainerSummary::new(
            "abcdef1234567890".to_string(),
            "worker".to_string(),
            "echo:latest".to_string(),
            "Exited (0) 1 minute ago".to_string(),
            Some("exited".to_string()),
            None,
            Vec::new(),
            false,
        );

        let status = ContainerStatusVm::from(&container);

        assert_eq!(status.state_label.as_ref(), "Exited");
        assert!(status.health_label.is_none());
        assert!(status.port_label.is_none());
    }

    #[test]
    fn summarizes_multiple_ports() {
        let container = ContainerSummary::new(
            "abcdef1234567890".to_string(),
            "api".to_string(),
            "echo:latest".to_string(),
            "Up 2 hours".to_string(),
            Some("running".to_string()),
            None,
            vec![
                ContainerPortSummary::new(8080, Some(18080), Some("tcp".to_string())),
                ContainerPortSummary::new(9090, Some(19090), Some("tcp".to_string())),
                ContainerPortSummary::new(7070, None, Some("tcp".to_string())),
            ],
            false,
        );

        assert_eq!(port_label(&container), Some("Ports 7070, +2".to_string()));
    }

    #[test]
    fn builds_openable_targets_for_published_tcp_ports() {
        let ports = vec![
            ContainerPortSummary::new(9090, Some(19090), Some("tcp".to_string())),
            ContainerPortSummary::new(8080, Some(18080), Some("tcp".to_string())),
        ];

        let targets = openable_port_targets(&ports);

        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].private_port, 8080);
        assert_eq!(targets[0].public_port, 18080);
        assert_eq!(targets[0].protocol, "tcp");
        assert_eq!(targets[0].url, "http://localhost:18080");
        assert_eq!(targets[0].menu_label(), "localhost:18080 -> 8080/tcp");
        assert_eq!(targets[1].public_port, 19090);
    }

    #[test]
    fn openable_targets_skip_unpublished_and_udp_ports() {
        let ports = vec![
            ContainerPortSummary::new(8080, None, Some("tcp".to_string())),
            ContainerPortSummary::new(5353, Some(15353), Some("udp".to_string())),
            ContainerPortSummary::new(3000, Some(13000), Some("TCP".to_string())),
            ContainerPortSummary::new(4000, Some(14000), None),
        ];

        let targets = openable_port_targets(&ports);

        assert_eq!(
            targets
                .iter()
                .map(|target| target.public_port)
                .collect::<Vec<_>>(),
            vec![13000, 14000]
        );
        assert_eq!(targets[1].protocol, "tcp");
    }

    #[test]
    fn openable_targets_deduplicate_by_public_port() {
        let ports = vec![
            ContainerPortSummary::new(8080, Some(18080), Some("tcp".to_string())),
            ContainerPortSummary::new(8081, Some(18080), Some("tcp".to_string())),
            ContainerPortSummary::new(9090, Some(19090), Some("tcp".to_string())),
        ];

        let targets = openable_port_targets(&ports);

        assert_eq!(
            targets
                .iter()
                .map(|target| target.public_port)
                .collect::<Vec<_>>(),
            vec![18080, 19090]
        );
    }

    #[test]
    fn uses_https_for_common_tls_ports() {
        assert_eq!(web_scheme(443), "https");
        assert_eq!(web_scheme(8443), "https");
        assert_eq!(web_scheme(80), "http");
        assert_eq!(web_scheme(8080), "http");

        let ports = vec![
            ContainerPortSummary::new(443, Some(443), Some("tcp".to_string())),
            ContainerPortSummary::new(8443, Some(8443), Some("tcp".to_string())),
            ContainerPortSummary::new(8080, Some(8080), Some("tcp".to_string())),
        ];

        let targets = openable_port_targets(&ports);

        assert_eq!(targets[0].url, "https://localhost:443");
        assert_eq!(targets[1].url, "http://localhost:8080");
        assert_eq!(targets[2].url, "https://localhost:8443");
    }
}

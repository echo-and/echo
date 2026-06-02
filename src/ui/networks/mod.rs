use std::time::{Duration, SystemTime};

mod create;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{
    Disableable, Icon, IconName, Sizable, ThemeMode, WindowExt,
    button::{Button, ButtonVariant},
    dialog::DialogButtonProps,
    h_flex,
    scroll::ScrollableElement as _,
    v_flex,
};
use rust_i18n::t;

use crate::{
    app::{EchoApp, NetworkNodeSelection, WorkspaceModel},
    bridge::{NetworkThroughputSnapshot, NetworkThroughputStatus},
    domain::{
        ContainerSummary, NetworkEndpointSummary, NetworkSummary, NetworkThroughputPoint,
        NetworkThroughputStats, NetworkThroughputTarget,
    },
    ui::{
        charts::{RealtimeSeries, realtime_area_chart},
        header::page_header,
        sidebar::nav_label,
        snapshot::WorkspaceSnapshot,
        theme::{theme_border, theme_content_bg, theme_list_bg, theme_secondary, theme_text},
    },
};

use self::create::open_create_network_dialog;

const ICON_NETWORK: &str = "assets/icons/network.svg";
const ICON_CONTAINER: &str = "assets/icons/box.svg";
const ICON_TRASH_2: &str = "assets/icons/trash-2.svg";
const DETAIL_PANEL_MIN_WIDTH: Pixels = px(256.);
const DETAIL_LABEL_WIDTH: Pixels = px(76.);
const CLUSTER_WIDTH: f32 = 720.;
const NETWORK_NODE_WIDTH: f32 = 160.;
const ENDPOINT_NODE_WIDTH: f32 = 160.;
const NODE_HEIGHT: f32 = 52.;
const FIRST_ENDPOINT_Y: f32 = 88.;
const ROW_GAP: f32 = 76.;
const ENDPOINT_GAP: f32 = 36.;
const ENDPOINTS_PER_ROW: usize = 4;

pub(super) fn networks_page(
    app: &mut EchoApp,
    snapshot: &WorkspaceSnapshot,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let model = app.model.clone();

    v_flex()
        .id("networks-page")
        .flex_1()
        .min_h_0()
        .h_full()
        .overflow_hidden()
        .bg(theme_content_bg(snapshot.theme_mode))
        .child(page_header(
            nav_label(snapshot.active_nav),
            Some(networks_header_actions(snapshot, cx).into_any_element()),
            snapshot.theme_mode,
        ))
        .child(networks_body(model, snapshot, cx))
}

fn networks_header_actions(
    snapshot: &WorkspaceSnapshot,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let pending = snapshot.pending_network_action.is_some();
    let theme_mode = snapshot.theme_mode;

    Button::new("networks-create")
        .outline()
        .icon(IconName::Plus)
        .label(t!("networks.create"))
        .small()
        .loading(pending)
        .disabled(pending)
        .on_click(cx.listener(move |app, _, window, cx| {
            open_create_network_dialog(app, theme_mode, window, cx);
        }))
}

fn networks_body(
    model: Entity<WorkspaceModel>,
    snapshot: &WorkspaceSnapshot,
    cx: &mut Context<EchoApp>,
) -> AnyElement {
    if snapshot.is_networks_loading && snapshot.networks.is_empty() {
        return centered_message(t!("networks.loading"), snapshot.theme_mode);
    }

    if snapshot.networks.is_empty() {
        return v_flex()
            .flex_1()
            .when_some(snapshot.network_error.clone(), |this, error| {
                this.child(error_banner(error, snapshot.theme_mode))
            })
            .child(centered_message(t!("networks.empty"), snapshot.theme_mode))
            .into_any_element();
    }

    h_flex()
        .flex_1()
        .min_h_0()
        .overflow_hidden()
        .child(topology_canvas(model, snapshot, cx))
        .child(detail_panel(snapshot, cx))
        .into_any_element()
}

fn topology_canvas(
    model: Entity<WorkspaceModel>,
    snapshot: &WorkspaceSnapshot,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    div()
        .relative()
        .flex()
        .flex_col()
        .justify_center()
        .flex_1()
        .min_w_0()
        .h_full()
        .overflow_y_scrollbar()
        .child(dot_grid_background(snapshot.theme_mode))
        .child(
            v_flex()
                .min_w(px(CLUSTER_WIDTH + 48.))
                .p(px(24.))
                .gap(px(18.))
                .when_some(snapshot.network_error.clone(), |this, error| {
                    this.child(error_banner(error, snapshot.theme_mode))
                })
                .children(snapshot.networks.iter().map(|network| {
                    network_cluster(model.clone(), network, snapshot, cx).into_any_element()
                })),
        )
}

fn network_cluster(
    model: Entity<WorkspaceModel>,
    network: &NetworkSummary,
    snapshot: &WorkspaceSnapshot,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let layout = ClusterLayout::new(network.endpoints.len());
    let link_color = if snapshot.theme_mode.is_dark() {
        hsla(0.555, 0.72, 0.61, 0.72)
    } else {
        hsla(0.555, 0.70, 0.45, 0.56)
    };
    let network_selection = NetworkNodeSelection::Network {
        network_id: network.id.clone(),
    };

    let connector_elements = network
        .endpoints
        .iter()
        .enumerate()
        .flat_map(|(index, _)| {
            let endpoint = layout.endpoint(index);
            connector_segments(
                ("network-link", network.id.clone(), index),
                layout.network_center_x(),
                NODE_HEIGHT,
                endpoint.center_x(),
                endpoint.y - px(24.).as_f32(),
                link_color,
            )
        })
        .collect::<Vec<_>>();
    let endpoint_nodes = network
        .endpoints
        .iter()
        .enumerate()
        .map(|(index, endpoint)| {
            let position = layout.endpoint(index);
            endpoint_node(model.clone(), network, endpoint, position, snapshot, cx)
                .into_any_element()
        })
        .collect::<Vec<_>>();

    div()
        .id(format!("network-cluster-{}", network.id))
        .relative()
        .mx_auto()
        .w(px(CLUSTER_WIDTH))
        .h(px(layout.height))
        .child(
            div()
                .absolute()
                .left(px(0.))
                .top(px(0.))
                .size_full()
                .children(connector_elements),
        )
        .child(network_node(
            model,
            network,
            layout.network_x(),
            snapshot,
            network_selection,
            cx,
        ))
        .children(endpoint_nodes)
}

fn network_node(
    model: Entity<WorkspaceModel>,
    network: &NetworkSummary,
    x: f32,
    snapshot: &WorkspaceSnapshot,
    selection: NetworkNodeSelection,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let selected = snapshot.selected_network_node.as_ref() == Some(&selection);
    let network_name = network.name.clone();
    let address = network
        .gateway
        .as_deref()
        .or(network.subnet.as_deref())
        .map(str::to_string)
        .unwrap_or_else(unavailable);

    node_shell(
        format!("network-node-{}", network.id),
        x,
        0.,
        NETWORK_NODE_WIDTH,
        selected,
        true,
        snapshot.theme_mode,
    )
    .on_click(cx.listener(move |_, _, _, cx| {
        model.update(cx, |model, cx| {
            model.select_network_node(selection.clone(), cx);
        });
    }))
    .child(network_node_icon())
    .child(
        v_flex()
            .min_w_0()
            .gap(px(4.))
            .child(
                div()
                    .text_xs()
                    .line_height(relative(1.2))
                    .truncate()
                    .child(network_name),
            )
            .child(
                div()
                    .text_xs()
                    .line_height(relative(1.2))
                    .text_color(theme_secondary(snapshot.theme_mode))
                    .whitespace_nowrap()
                    .child(address),
            ),
    )
}

fn endpoint_node(
    model: Entity<WorkspaceModel>,
    network: &NetworkSummary,
    endpoint: &NetworkEndpointSummary,
    position: NodePosition,
    snapshot: &WorkspaceSnapshot,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let selection = NetworkNodeSelection::Container {
        network_id: network.id.clone(),
        container_id: endpoint.container_id.clone(),
    };
    let selected = snapshot.selected_network_node.as_ref() == Some(&selection);
    let name = endpoint.name.clone();
    let address = endpoint_address(endpoint);

    node_shell(
        format!(
            "network-endpoint-node-{}-{}",
            network.id, endpoint.container_id
        ),
        position.x,
        position.y,
        ENDPOINT_NODE_WIDTH,
        selected,
        false,
        snapshot.theme_mode,
    )
    .on_click(cx.listener(move |_, _, _, cx| {
        model.update(cx, |model, cx| {
            model.select_network_node(selection.clone(), cx);
        });
    }))
    .child(container_node_icon(snapshot.theme_mode))
    .child(
        v_flex()
            .min_w_0()
            .gap(px(4.))
            .child(
                div()
                    .text_xs()
                    .line_height(relative(1.2))
                    .truncate()
                    .child(name),
            )
            .child(
                div()
                    .text_xs()
                    .line_height(relative(1.2))
                    .text_color(theme_secondary(snapshot.theme_mode))
                    .whitespace_nowrap()
                    .child(address),
            ),
    )
}

fn node_shell(
    id: impl Into<ElementId>,
    x: f32,
    y: f32,
    width: f32,
    selected: bool,
    network: bool,
    theme_mode: ThemeMode,
) -> Stateful<Div> {
    let accent = hsla(0.555, 0.86, 0.64, 1.0);
    let background = if network {
        if theme_mode.is_dark() {
            hsla(0.566, 0.66, 0.19, 1.0)
        } else {
            hsla(0.555, 0.78, 0.94, 1.0)
        }
    } else if selected {
        if theme_mode.is_dark() {
            hsla(0.555, 0.54, 0.16, 1.0)
        } else {
            hsla(0.555, 0.76, 0.93, 1.0)
        }
    } else {
        theme_list_bg(theme_mode)
    };

    h_flex()
        .id(id)
        .absolute()
        .left(px(x))
        .top(px(y))
        .w(px(width))
        .h(px(NODE_HEIGHT))
        .items_center()
        .gap(px(12.))
        .px(px(12.))
        .rounded(px(4.))
        .border_1()
        .border_color(if selected {
            accent
        } else {
            theme_border(theme_mode)
        })
        .bg(background)
        .cursor_pointer()
        .overflow_hidden()
}

fn network_node_icon() -> impl IntoElement {
    div()
        .size(px(20.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(
            Icon::new(Icon::empty())
                .path(ICON_NETWORK)
                .small()
                .text_color(hsla(0.555, 0.86, 0.64, 1.0)),
        )
}

fn container_node_icon(theme_mode: ThemeMode) -> impl IntoElement {
    div()
        .size(px(20.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(
            Icon::new(Icon::empty())
                .path(ICON_CONTAINER)
                .small()
                .text_color(theme_secondary(theme_mode)),
        )
}

fn connector_segments(
    id: impl std::fmt::Debug,
    from_x: f32,
    from_y: f32,
    to_x: f32,
    mid_y: f32,
    color: Hsla,
) -> Vec<AnyElement> {
    let stem_height = (mid_y - from_y).max(0.);
    let left = from_x.min(to_x);
    let width = (from_x - to_x).abs();

    vec![
        line_segment(
            format!("{id:?}:stem"),
            from_x,
            from_y,
            1.,
            stem_height,
            color,
        ),
        line_segment(format!("{id:?}:bar"), left, mid_y, width, 1., color),
        line_segment(
            format!("{id:?}:endpoint"),
            to_x,
            mid_y,
            1.,
            NODE_HEIGHT,
            color,
        ),
    ]
}

fn line_segment(
    id: impl Into<ElementId>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: Hsla,
) -> AnyElement {
    div()
        .id(id)
        .absolute()
        .left(px(x))
        .top(px(y))
        .w(px(width.max(1.)))
        .h(px(height.max(1.)))
        .bg(color)
        .into_any_element()
}

fn detail_panel(snapshot: &WorkspaceSnapshot, cx: &mut Context<EchoApp>) -> impl IntoElement {
    let content = selected_detail(snapshot, cx);

    v_flex()
        .w(DETAIL_PANEL_MIN_WIDTH)
        .min_w(DETAIL_PANEL_MIN_WIDTH)
        .h_full()
        .flex_none()
        .gap(px(24.))
        .px(px(16.))
        .py(px(12.))
        .border_l_1()
        .border_color(theme_border(snapshot.theme_mode))
        .bg(theme_list_bg(snapshot.theme_mode))
        .overflow_x_hidden()
        .overflow_y_scrollbar()
        .child(content)
}

fn selected_detail(snapshot: &WorkspaceSnapshot, cx: &mut Context<EchoApp>) -> AnyElement {
    let selection = snapshot.selected_network_node.as_ref();
    match selection {
        Some(NetworkNodeSelection::Container {
            network_id,
            container_id,
        }) => snapshot
            .networks
            .iter()
            .find(|network| &network.id == network_id)
            .and_then(|network| {
                network
                    .endpoints
                    .iter()
                    .find(|endpoint| &endpoint.container_id == container_id)
                    .map(|endpoint| {
                        container_detail(snapshot, network, endpoint).into_any_element()
                    })
            })
            .unwrap_or_else(|| empty_detail(snapshot.theme_mode)),
        Some(NetworkNodeSelection::Network { network_id }) => snapshot
            .networks
            .iter()
            .find(|network| &network.id == network_id)
            .map(|network| network_detail(snapshot, network, cx).into_any_element())
            .unwrap_or_else(|| empty_detail(snapshot.theme_mode)),
        None => snapshot
            .networks
            .first()
            .map(|network| network_detail(snapshot, network, cx).into_any_element())
            .unwrap_or_else(|| empty_detail(snapshot.theme_mode)),
    }
}

fn network_detail(
    snapshot: &WorkspaceSnapshot,
    network: &NetworkSummary,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let throughput = snapshot.network_throughput.as_ref().filter(|throughput| {
        matches!(
            &throughput.target,
            NetworkThroughputTarget::Network { network_id, .. } if network_id == &network.id
        )
    });
    let rows = vec![
        (t!("networks.driver").to_string(), network.driver.clone()),
        (
            t!("networks.subnet").to_string(),
            network.subnet.clone().unwrap_or_else(unavailable),
        ),
        (
            t!("networks.gateway").to_string(),
            network.gateway.clone().unwrap_or_else(unavailable),
        ),
        (
            t!("networks.ipv6").to_string(),
            if network.ipv6_enabled {
                t!("networks.enabled").to_string()
            } else {
                t!("networks.disabled").to_string()
            },
        ),
        (
            t!("networks.containers").to_string(),
            network.endpoints.len().to_string(),
        ),
        (
            t!("networks.created").to_string(),
            format_created(network.created_at),
        ),
        (
            t!("networks.labels").to_string(),
            label_summary(network.labels.len()),
        ),
    ];

    v_flex()
        .flex_1()
        .gap(px(24.))
        .child(detail_section(
            network.name.clone(),
            detail_rows(rows, snapshot.theme_mode),
        ))
        .child(detail_section(
            t!("networks.network_io"),
            network_io_section(throughput, snapshot.theme_mode),
        ))
        .child(detail_section(
            t!("networks.connected_containers"),
            connected_containers(network, snapshot.theme_mode).into_any_element(),
        ))
        .child(div().flex_1())
        .child(network_delete_action(snapshot, network, cx))
}

fn network_delete_action(
    snapshot: &WorkspaceSnapshot,
    network: &NetworkSummary,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let network_id = network.id.clone();
    let network_name = network.name.clone();
    let pending = snapshot
        .pending_network_action
        .as_ref()
        .is_some_and(|pending| pending.network_name == network.id);
    let disabled = snapshot.pending_network_action.is_some();

    h_flex().w_full().pt(px(4.)).justify_center().child(
        Button::new(format!("network-delete-{}", network.id))
            .outline()
            .icon(Icon::new(Icon::empty()).path(ICON_TRASH_2))
            .label(t!("networks.delete"))
            .small()
            .loading(pending)
            .disabled(disabled)
            .on_click(cx.listener(move |app, _, window, cx| {
                open_delete_network_dialog(
                    app,
                    network_id.clone(),
                    network_name.clone(),
                    window,
                    cx,
                );
            })),
    )
}

fn open_delete_network_dialog(
    _app: &mut EchoApp,
    network_id: String,
    network_name: String,
    window: &mut Window,
    cx: &mut Context<EchoApp>,
) {
    let app = cx.entity().downgrade();

    window.open_alert_dialog(cx, move |dialog, _, _| {
        let remove_app = app.clone();
        dialog
            .confirm()
            .title(t!("networks.delete_title"))
            .description(t!(
                "networks.delete_description",
                name = network_name.clone()
            ))
            .button_props(
                DialogButtonProps::default()
                    .ok_text(t!("networks.delete_confirm"))
                    .ok_variant(ButtonVariant::Danger)
                    .cancel_text(t!("networks.delete_cancel"))
                    .show_cancel(true),
            )
            .on_ok({
                let network_id = network_id.clone();
                move |_, _, cx| {
                    let _ = remove_app.update(cx, |app, cx| {
                        app.remove_network(network_id.clone(), cx);
                    });
                    true
                }
            })
    });
}

fn container_detail(
    snapshot: &WorkspaceSnapshot,
    network: &NetworkSummary,
    endpoint: &NetworkEndpointSummary,
) -> impl IntoElement {
    let container = snapshot
        .containers
        .iter()
        .find(|container| container.id == endpoint.container_id);
    let rows = vec![
        (
            t!("networks.container_id").to_string(),
            short_id(&endpoint.container_id),
        ),
        (
            t!("networks.image").to_string(),
            container
                .map(|container| container.image.clone())
                .unwrap_or_else(unavailable),
        ),
        (
            t!("networks.status").to_string(),
            container_status(container).unwrap_or_else(unavailable),
        ),
        (t!("networks.network").to_string(), network.name.clone()),
        (
            t!("networks.ipv4").to_string(),
            endpoint.ipv4_address.clone().unwrap_or_else(unavailable),
        ),
        (
            t!("networks.ipv6").to_string(),
            endpoint.ipv6_address.clone().unwrap_or_else(unavailable),
        ),
        (
            t!("networks.mac").to_string(),
            endpoint.mac_address.clone().unwrap_or_else(unavailable),
        ),
        (
            t!("networks.endpoint_id").to_string(),
            endpoint
                .endpoint_id
                .as_ref()
                .map(|id| short_id(id))
                .unwrap_or_else(unavailable),
        ),
    ];

    let throughput = snapshot.network_throughput.as_ref().filter(|throughput| {
        matches!(
            &throughput.target,
            NetworkThroughputTarget::Container {
                network_id,
                container_id,
                ..
            } if network_id == &network.id && container_id == &endpoint.container_id
        )
    });

    v_flex()
        .gap(px(24.))
        .child(detail_section(
            endpoint.name.clone(),
            detail_rows(rows, snapshot.theme_mode),
        ))
        .child(detail_section(
            t!("networks.network_io"),
            network_io_section(throughput, snapshot.theme_mode),
        ))
}

fn detail_section(title: impl Into<SharedString>, content: AnyElement) -> impl IntoElement {
    v_flex()
        .w_full()
        .min_w_0()
        .gap(px(10.))
        .child(
            div()
                .min_w_0()
                .text_sm()
                .line_height(relative(1.2))
                .truncate()
                .child(title.into()),
        )
        .child(content)
}

fn detail_rows(rows: Vec<(String, String)>, theme_mode: ThemeMode) -> AnyElement {
    h_flex()
        .w_full()
        .min_w_0()
        .overflow_x_hidden()
        .items_start()
        .gap(px(16.))
        .child(
            v_flex()
                .w(DETAIL_LABEL_WIDTH)
                .flex_none()
                .gap(px(6.))
                .children(rows.iter().map(|(label, _)| {
                    div()
                        .min_w_0()
                        .text_xs()
                        .line_height(relative(1.8))
                        .text_color(theme_secondary(theme_mode))
                        .truncate()
                        .child(label.clone())
                })),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(6.))
                .children(rows.into_iter().map(|(_, value)| {
                    div()
                        .text_xs()
                        .line_height(relative(1.8))
                        .text_color(theme_secondary(theme_mode))
                        .truncate()
                        .child(value)
                })),
        )
        .into_any_element()
}

fn network_io_section(
    throughput: Option<&NetworkThroughputSnapshot>,
    theme_mode: ThemeMode,
) -> AnyElement {
    let Some(throughput) = throughput else {
        return network_io_message(t!("detail.waiting_for_stats").to_string(), theme_mode);
    };

    if throughput.status == NetworkThroughputStatus::Idle {
        return network_io_message(t!("networks.io_idle").to_string(), theme_mode);
    }

    let Some(latest) = throughput.latest.as_ref() else {
        let message = throughput
            .error
            .clone()
            .filter(|_| throughput.status == NetworkThroughputStatus::Error)
            .unwrap_or_else(|| t!("detail.waiting_for_stats").to_string());
        return network_io_message(message, theme_mode);
    };

    let status = match throughput.status {
        NetworkThroughputStatus::Reconnecting => Some(
            throughput
                .error
                .clone()
                .unwrap_or_else(|| t!("networks.io_reconnecting").to_string()),
        ),
        NetworkThroughputStatus::Error => Some(
            throughput
                .error
                .clone()
                .unwrap_or_else(|| t!("networks.io_unavailable").to_string()),
        ),
        _ => None,
    };

    v_flex()
        .w_full()
        .min_w_0()
        .gap(px(10.))
        .child(network_io_rows(latest, theme_mode))
        .child(
            div()
                .w(px(180.))
                .max_w_full()
                .h(px(76.))
                .overflow_hidden()
                .when(!throughput.history.is_empty(), |this| {
                    this.child(network_io_chart(&throughput.history, theme_mode))
                }),
        )
        .when_some(status, |this, status| {
            this.child(
                div()
                    .text_size(px(10.))
                    .line_height(px(14.))
                    .text_color(theme_secondary(theme_mode))
                    .line_clamp(2)
                    .child(status),
            )
        })
        .into_any_element()
}

fn network_io_rows(latest: &NetworkThroughputStats, theme_mode: ThemeMode) -> impl IntoElement {
    let rows = vec![
        (
            t!("networks.total").to_string(),
            format_rate(latest.total_bytes_per_sec()),
            theme_text(theme_mode),
        ),
        (
            t!("detail.rx").to_string(),
            format_rate(latest.rx_bytes_per_sec),
            network_rx_color(),
        ),
        (
            t!("detail.tx").to_string(),
            format_rate(latest.tx_bytes_per_sec),
            network_tx_color(),
        ),
    ];

    h_flex()
        .w_full()
        .min_w_0()
        .overflow_x_hidden()
        .items_start()
        .gap(px(16.))
        .child(
            v_flex()
                .w(DETAIL_LABEL_WIDTH)
                .flex_none()
                .gap(px(6.))
                .children(rows.iter().map(|(label, _, _)| {
                    div()
                        .min_w_0()
                        .text_xs()
                        .line_height(relative(1.8))
                        .text_color(theme_secondary(theme_mode))
                        .truncate()
                        .child(label.clone())
                })),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(6.))
                .children(rows.into_iter().map(|(_, value, color)| {
                    div()
                        .min_w_0()
                        .text_xs()
                        .line_height(relative(1.8))
                        .text_color(color)
                        .truncate()
                        .child(value)
                })),
        )
}

fn network_io_message(message: String, theme_mode: ThemeMode) -> AnyElement {
    div()
        .text_sm()
        .line_height(relative(1.35))
        .text_color(theme_secondary(theme_mode))
        .child(message)
        .into_any_element()
}

fn network_io_chart(history: &[NetworkThroughputPoint], theme_mode: ThemeMode) -> impl IntoElement {
    let rx = network_rx_color();
    let tx = network_tx_color();

    realtime_area_chart(vec![
        RealtimeSeries::new(
            history.iter().map(|point| point.rx_bytes_per_sec),
            rx,
            linear_gradient(
                0.,
                linear_color_stop(rx.opacity(0.34), 1.),
                linear_color_stop(theme_content_bg(theme_mode).opacity(0.04), 0.),
            ),
        ),
        RealtimeSeries::new(
            history.iter().map(|point| point.tx_bytes_per_sec),
            tx,
            linear_gradient(
                0.,
                linear_color_stop(tx.opacity(0.24), 1.),
                linear_color_stop(theme_content_bg(theme_mode).opacity(0.04), 0.),
            ),
        ),
    ])
}

fn network_rx_color() -> Hsla {
    hsla(0.555, 0.72, 0.52, 1.0)
}

fn network_tx_color() -> Hsla {
    hsla(0.145, 0.82, 0.52, 1.0)
}

fn connected_containers(network: &NetworkSummary, theme_mode: ThemeMode) -> impl IntoElement {
    if network.endpoints.is_empty() {
        return v_flex()
            .text_sm()
            .text_color(theme_secondary(theme_mode))
            .child(t!("networks.no_containers"));
    }

    h_flex()
        .w_full()
        .min_w_0()
        .overflow_x_hidden()
        .items_start()
        .gap(px(16.))
        .child(
            v_flex()
                .w(DETAIL_LABEL_WIDTH)
                .flex_none()
                .gap(px(6.))
                .children(network.endpoints.iter().map(|endpoint| {
                    div()
                        .min_w_0()
                        .text_xs()
                        .line_height(relative(1.8))
                        .text_color(theme_secondary(theme_mode))
                        .truncate()
                        .child(endpoint.name.clone())
                })),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(6.))
                .children(network.endpoints.iter().map(|endpoint| {
                    div()
                        .min_w_0()
                        .text_xs()
                        .line_height(relative(1.8))
                        .truncate()
                        .text_color(theme_secondary(theme_mode))
                        .child(endpoint_address(endpoint))
                })),
        )
}

fn empty_detail(theme_mode: ThemeMode) -> AnyElement {
    div()
        .text_sm()
        .text_color(theme_secondary(theme_mode))
        .child(t!("networks.empty"))
        .into_any_element()
}

fn centered_message(text: impl Into<SharedString>, theme_mode: ThemeMode) -> AnyElement {
    div()
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .text_sm()
        .text_color(theme_secondary(theme_mode))
        .child(text.into())
        .into_any_element()
}

fn error_banner(error: String, theme_mode: ThemeMode) -> impl IntoElement {
    div()
        .m(px(16.))
        .p(px(10.))
        .rounded(px(4.))
        .border_1()
        .border_color(theme_border(theme_mode))
        .text_sm()
        .line_height(relative(1.35))
        .text_color(theme_text(theme_mode))
        .child(error)
}

fn dot_grid_background(theme_mode: ThemeMode) -> impl IntoElement {
    let color = if theme_mode.is_dark() {
        hsla(0.555, 0.42, 0.68, 0.16)
    } else {
        hsla(0.555, 0.36, 0.42, 0.12)
    };

    DotGridBackground { color }
}

struct DotGridBackground {
    color: Hsla,
}

impl IntoElement for DotGridBackground {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for DotGridBackground {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let style = Style {
            position: Position::Absolute,
            size: Size::full(),
            ..Default::default()
        };

        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Window,
        _: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        _: &mut App,
    ) {
        let spacing = px(24.);
        let diameter = px(2.);
        let radius = diameter / 2.;
        let start_offset = spacing / 2.;

        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            let mut y = bounds.origin.y + start_offset;
            while y < bounds.bottom() {
                let mut x = bounds.origin.x + start_offset;
                while x < bounds.right() {
                    window.paint_quad(PaintQuad {
                        bounds: Bounds::new(
                            point(x - radius, y - radius),
                            size(diameter, diameter),
                        ),
                        corner_radii: Corners::all(radius),
                        background: self.color.into(),
                        border_widths: Edges::all(px(0.)),
                        border_color: transparent_white(),
                        border_style: BorderStyle::default(),
                    });
                    x += spacing;
                }
                y += spacing;
            }
        });
    }
}

fn endpoint_address(endpoint: &NetworkEndpointSummary) -> String {
    endpoint
        .ipv4_address
        .clone()
        .or_else(|| endpoint.ipv6_address.clone())
        .unwrap_or_else(unavailable)
}

fn container_status(container: Option<&ContainerSummary>) -> Option<String> {
    let container = container?;
    container
        .state
        .clone()
        .filter(|state| !state.is_empty())
        .or_else(|| Some(container.status.clone()))
}

fn label_summary(count: usize) -> String {
    if count == 0 {
        unavailable()
    } else {
        count.to_string()
    }
}

fn format_created(created_at: Option<SystemTime>) -> String {
    let Some(created_at) = created_at else {
        return t!("networks.created_unknown").to_string();
    };

    let elapsed = SystemTime::now()
        .duration_since(created_at)
        .unwrap_or(Duration::ZERO);
    let days = elapsed.as_secs() / 86_400;
    if days >= 365 {
        t!("networks.created_years", count = days / 365).to_string()
    } else if days >= 30 {
        t!("networks.created_months", count = days / 30).to_string()
    } else if days >= 1 {
        t!("networks.created_days", count = days).to_string()
    } else {
        let hours = elapsed.as_secs() / 3_600;
        if hours >= 1 {
            t!("networks.created_hours", count = hours).to_string()
        } else {
            t!("networks.created_recently").to_string()
        }
    }
}

fn short_id(id: &str) -> String {
    id.chars().take(12).collect()
}

fn unavailable() -> String {
    t!("networks.unavailable").to_string()
}

fn format_rate(bytes_per_sec: f64) -> String {
    let (value, unit) = format_bytes_value(bytes_per_sec);
    format!("{}{}/s", value, unit)
}

fn format_bytes_value(bytes: f64) -> (String, String) {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes.max(0.);
    let mut unit = UNITS[0];

    for next_unit in UNITS.iter().skip(1) {
        if value < 1024. {
            break;
        }
        value /= 1024.;
        unit = next_unit;
    }

    let formatted = if value >= 100. {
        format!("{value:.0}")
    } else if value >= 10. {
        format!("{value:.1}")
    } else {
        format!("{value:.2}")
    };

    (formatted, unit.to_string())
}

#[derive(Clone, Copy)]
struct NodePosition {
    x: f32,
    y: f32,
}

impl NodePosition {
    fn center_x(&self) -> f32 {
        self.x + ENDPOINT_NODE_WIDTH / 2.
    }
}

struct ClusterLayout {
    height: f32,
    rows: usize,
    endpoint_count: usize,
}

impl ClusterLayout {
    fn new(endpoint_count: usize) -> Self {
        let rows = endpoint_count.div_ceil(ENDPOINTS_PER_ROW);
        let height = if rows == 0 {
            NODE_HEIGHT
        } else {
            FIRST_ENDPOINT_Y + (rows.saturating_sub(1)) as f32 * ROW_GAP + NODE_HEIGHT
        };

        Self {
            rows,
            endpoint_count,
            height,
        }
    }

    fn network_x(&self) -> f32 {
        CLUSTER_WIDTH / 2. - NETWORK_NODE_WIDTH / 2.
    }

    fn network_center_x(&self) -> f32 {
        self.network_x() + NETWORK_NODE_WIDTH / 2.
    }

    fn endpoint(&self, index: usize) -> NodePosition {
        let row = index / ENDPOINTS_PER_ROW;
        let col = index % ENDPOINTS_PER_ROW;
        let row_count = if row + 1 == self.rows {
            index_count_in_last_row(self.endpoint_count)
        } else {
            ENDPOINTS_PER_ROW
        };
        let row_width = row_count as f32 * ENDPOINT_NODE_WIDTH
            + (row_count.saturating_sub(1)) as f32 * ENDPOINT_GAP;
        let start_x = CLUSTER_WIDTH / 2. - row_width / 2.;

        NodePosition {
            x: start_x + col as f32 * (ENDPOINT_NODE_WIDTH + ENDPOINT_GAP),
            y: FIRST_ENDPOINT_Y + row as f32 * ROW_GAP,
        }
    }
}

fn index_count_in_last_row(count: usize) -> usize {
    let rem = count % ENDPOINTS_PER_ROW;
    if rem == 0 { ENDPOINTS_PER_ROW } else { rem }
}

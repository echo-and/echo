use gpui::*;
use gpui_component::{StyledExt as _, ThemeMode, h_flex, scroll::ScrollableElement as _, v_flex};
use rust_i18n::t;

use crate::{
    domain::ContainerSummary,
    ui::{
        containers::{
            format::{
                display_state, format_bytes, format_environment, format_labels, format_mounts,
                format_number, format_ports, format_rate, format_timestamp, format_uptime_value,
                memory_status_value, unavailable,
            },
            style::metric_card_bg,
        },
        snapshot::WorkspaceSnapshot,
        theme::{theme_border, theme_content_bg, theme_muted, theme_text},
    },
};

pub(super) fn status_panel(
    container: &ContainerSummary,
    snapshot: &WorkspaceSnapshot,
) -> AnyElement {
    let detail_snapshot = snapshot
        .container_detail
        .as_ref()
        .filter(|detail| detail.container_id == container.id);
    let detail = detail_snapshot.and_then(|snapshot| snapshot.detail.as_ref());
    let stats = detail_snapshot.and_then(|snapshot| snapshot.latest.as_ref());

    let runtime_rows = vec![
        (
            t!("detail.state").to_string(),
            container
                .state
                .as_deref()
                .map(display_state)
                .unwrap_or_else(|| "-".to_string()),
        ),
        (
            t!("detail.health").to_string(),
            container
                .health
                .as_deref()
                .map(display_state)
                .unwrap_or_else(|| "-".to_string()),
        ),
        (
            t!("detail.status").to_string(),
            if container.status.is_empty() {
                "-".to_string()
            } else {
                container.status.clone()
            },
        ),
        (
            t!("detail.uptime").to_string(),
            detail
                .and_then(|detail| detail.started_at.as_ref())
                .map(|started_at| {
                    let (value, unit) = format_uptime_value(started_at);
                    format!("{}{}", value, unit)
                })
                .unwrap_or_else(|| unavailable(detail_snapshot)),
        ),
        (
            t!("detail.started").to_string(),
            detail
                .and_then(|detail| detail.started_at.as_ref())
                .map(|started_at| format_timestamp(started_at))
                .unwrap_or_else(|| unavailable(detail_snapshot)),
        ),
        (
            t!("detail.restart").to_string(),
            detail
                .map(|detail| detail.restart_count.to_string())
                .unwrap_or_else(|| unavailable(detail_snapshot)),
        ),
        (
            t!("detail.policy").to_string(),
            detail
                .and_then(|detail| detail.restart_policy.clone())
                .unwrap_or_else(|| unavailable(detail_snapshot)),
        ),
    ];

    let resource_rows = vec![
        (
            t!("detail.cpu").to_string(),
            stats
                .map(|stats| format!("{}%", format_number(stats.cpu_percent, 2)))
                .unwrap_or_else(|| unavailable(detail_snapshot)),
        ),
        (
            t!("detail.memory").to_string(),
            stats
                .map(memory_status_value)
                .unwrap_or_else(|| unavailable(detail_snapshot)),
        ),
        (
            t!("detail.network_rx").to_string(),
            stats
                .map(|stats| format_rate(stats.network_rx_bytes_per_sec))
                .unwrap_or_else(|| unavailable(detail_snapshot)),
        ),
        (
            t!("detail.network_tx").to_string(),
            stats
                .map(|stats| format_rate(stats.network_tx_bytes_per_sec))
                .unwrap_or_else(|| unavailable(detail_snapshot)),
        ),
        (
            t!("detail.disk_read").to_string(),
            stats
                .map(|stats| format_rate(stats.disk_read_bytes_per_sec))
                .unwrap_or_else(|| unavailable(detail_snapshot)),
        ),
        (
            t!("detail.disk_write").to_string(),
            stats
                .map(|stats| format_rate(stats.disk_write_bytes_per_sec))
                .unwrap_or_else(|| unavailable(detail_snapshot)),
        ),
        (
            t!("detail.ports").to_string(),
            detail
                .map(|detail| format_ports(&detail.ports))
                .unwrap_or_else(|| format_ports(&container.ports)),
        ),
    ];

    let storage_rows = vec![
        (
            t!("detail.image").to_string(),
            detail
                .map(|detail| detail.image.clone())
                .unwrap_or_else(|| container.image.clone()),
        ),
        (
            t!("detail.created").to_string(),
            detail
                .and_then(|detail| detail.created_at.as_ref())
                .map(|created_at| format_timestamp(created_at))
                .unwrap_or_else(|| unavailable(detail_snapshot)),
        ),
        (
            t!("detail.writable_size").to_string(),
            detail
                .and_then(|detail| detail.size_rw_bytes)
                .map(format_bytes)
                .unwrap_or_else(|| unavailable(detail_snapshot)),
        ),
        (
            t!("detail.rootfs_size").to_string(),
            detail
                .and_then(|detail| detail.size_root_fs_bytes)
                .map(format_bytes)
                .unwrap_or_else(|| unavailable(detail_snapshot)),
        ),
        (
            t!("detail.volumes").to_string(),
            detail
                .map(format_mounts)
                .unwrap_or_else(|| unavailable(detail_snapshot)),
        ),
        (
            t!("detail.environment").to_string(),
            detail
                .map(format_environment)
                .unwrap_or_else(|| unavailable(detail_snapshot)),
        ),
        (
            t!("detail.labels").to_string(),
            detail
                .map(format_labels)
                .unwrap_or_else(|| unavailable(detail_snapshot)),
        ),
    ];

    div()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .p(px(12.))
        .bg(theme_content_bg(snapshot.theme_mode))
        .overflow_y_scrollbar()
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .items_start()
                .gap(px(12.))
                .overflow_hidden()
                .child(status_group(
                    t!("detail.runtime").to_string(),
                    runtime_rows,
                    snapshot.theme_mode,
                ))
                .child(status_group(
                    t!("detail.resources").to_string(),
                    resource_rows,
                    snapshot.theme_mode,
                ))
                .child(status_group(
                    t!("detail.configuration").to_string(),
                    storage_rows,
                    snapshot.theme_mode,
                )),
        )
        .into_any_element()
}

fn status_group(
    title: String,
    rows: Vec<(String, String)>,
    theme_mode: ThemeMode,
) -> impl IntoElement {
    v_flex()
        .flex_1()
        .w_0()
        .min_w_0()
        .overflow_hidden()
        .gap(px(8.))
        .p(px(12.))
        .rounded(px(4.))
        .border_1()
        .border_color(theme_border(theme_mode))
        .bg(metric_card_bg(theme_mode))
        .child(
            div()
                .text_xs()
                .font_medium()
                .line_height(relative(1.2))
                .text_color(theme_text(theme_mode))
                .child(title),
        )
        .children(
            rows.into_iter()
                .map(|(label, value)| status_value_row(label, value, theme_mode)),
        )
}

fn status_value_row(label: String, value: String, theme_mode: ThemeMode) -> impl IntoElement {
    h_flex()
        .w_full()
        .min_w_0()
        .overflow_hidden()
        .items_start()
        .gap(px(12.))
        .child(
            div()
                .w(px(92.))
                .flex_shrink_0()
                .text_xs()
                .line_height(relative(1.35))
                .text_color(theme_muted(theme_mode))
                .child(label),
        )
        .child(
            div()
                .flex_1()
                .w_0()
                .min_w_0()
                .overflow_hidden()
                .text_xs()
                .line_height(relative(1.35))
                .text_color(theme_text(theme_mode))
                .truncate()
                .child(value),
        )
}

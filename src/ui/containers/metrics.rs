use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{Icon, Sizable, ThemeMode, h_flex, v_flex};
use rust_i18n::t;

use crate::{
    bridge::ContainerDetailSnapshot,
    domain::{ContainerMetricPoint, ContainerRuntimeStats},
    ui::{
        charts::{RealtimeSeries, realtime_area_chart},
        containers::{
            detail::ContainerDetailVm,
            format::{format_bytes_value, format_number, format_rate},
            style::metric_card_bg,
        },
        theme::{theme_border, theme_content_bg, theme_muted, theme_text},
    },
};

const ICON_CPU: &str = "assets/icons/cpu.svg";
const ICON_MICROCHIP: &str = "assets/icons/microchip.svg";
const ICON_NETWORK: &str = "assets/icons/network.svg";
const ICON_HARD_DRIVE: &str = "assets/icons/hard-drive.svg";
const ICON_CLOCK: &str = "assets/icons/clock.svg";
const ICON_ROTATE_CW: &str = "assets/icons/rotate-cw.svg";

pub(super) fn metrics_overview(
    vm: &ContainerDetailVm,
    detail: Option<&ContainerDetailSnapshot>,
    theme_mode: ThemeMode,
) -> impl IntoElement {
    let history = detail
        .map(|detail| detail.history.clone())
        .unwrap_or_default();
    let latest = detail.and_then(|detail| detail.latest.as_ref());

    h_flex()
        .w_full()
        .min_w_0()
        .items_start()
        .gap(px(10.))
        .child(metric_card(
            MetricCard {
                icon: ICON_CPU,
                title: t!("detail.cpu").to_string(),
                value: latest
                    .map(|stats| format_number(stats.cpu_percent, 2))
                    .unwrap_or_else(|| "--".to_string()),
                unit: "%".to_string(),
                footer: latest
                    .and_then(cpu_footer)
                    .unwrap_or_else(|| t!("detail.waiting_for_stats").to_string()),
                chart: Some(MetricChart::Cpu),
            },
            &history,
            theme_mode,
        ))
        .child(metric_card(
            MetricCard {
                icon: ICON_MICROCHIP,
                title: t!("detail.memory").to_string(),
                value: latest
                    .and_then(|stats| stats.memory_usage_bytes)
                    .map(|bytes| format_bytes_value(bytes as f64).0)
                    .unwrap_or_else(|| "--".to_string()),
                unit: latest
                    .and_then(|stats| stats.memory_usage_bytes)
                    .map(|bytes| format_bytes_value(bytes as f64).1)
                    .unwrap_or_default(),
                footer: latest
                    .map(memory_footer)
                    .unwrap_or_else(|| t!("detail.waiting_for_stats").to_string()),
                chart: Some(MetricChart::Memory),
            },
            &history,
            theme_mode,
        ))
        .child(metric_card(
            MetricCard {
                icon: ICON_NETWORK,
                title: t!("detail.network").to_string(),
                value: latest
                    .map(|stats| {
                        format_bytes_value(
                            stats.network_rx_bytes_per_sec + stats.network_tx_bytes_per_sec,
                        )
                        .0
                    })
                    .unwrap_or_else(|| "--".to_string()),
                unit: latest
                    .map(|stats| {
                        format!(
                            "{}/s",
                            format_bytes_value(
                                stats.network_rx_bytes_per_sec + stats.network_tx_bytes_per_sec,
                            )
                            .1
                        )
                    })
                    .unwrap_or_default(),
                footer: latest
                    .map(network_footer)
                    .unwrap_or_else(|| t!("detail.waiting_for_stats").to_string()),
                chart: Some(MetricChart::Network),
            },
            &history,
            theme_mode,
        ))
        .child(metric_card(
            MetricCard {
                icon: ICON_HARD_DRIVE,
                title: t!("detail.disk").to_string(),
                value: latest
                    .map(|stats| {
                        format_bytes_value(
                            stats.disk_read_bytes_per_sec + stats.disk_write_bytes_per_sec,
                        )
                        .0
                    })
                    .unwrap_or_else(|| "--".to_string()),
                unit: latest
                    .map(|stats| {
                        format!(
                            "{}/s",
                            format_bytes_value(
                                stats.disk_read_bytes_per_sec + stats.disk_write_bytes_per_sec,
                            )
                            .1
                        )
                    })
                    .unwrap_or_default(),
                footer: latest
                    .map(disk_footer)
                    .unwrap_or_else(|| t!("detail.waiting_for_stats").to_string()),
                chart: Some(MetricChart::Disk),
            },
            &history,
            theme_mode,
        ))
        .child(metric_card(
            MetricCard {
                icon: ICON_CLOCK,
                title: t!("detail.uptime").to_string(),
                value: vm.uptime_value.clone(),
                unit: vm.uptime_unit.clone(),
                footer: vm.started_footer.clone(),
                chart: None,
            },
            &history,
            theme_mode,
        ))
        .child(metric_card(
            MetricCard {
                icon: ICON_ROTATE_CW,
                title: t!("detail.restart").to_string(),
                value: vm.restart_count.clone(),
                unit: String::new(),
                footer: vm.restart_policy.clone(),
                chart: None,
            },
            &history,
            theme_mode,
        ))
}

fn metric_card(
    card: MetricCard,
    history: &[ContainerMetricPoint],
    theme_mode: ThemeMode,
) -> impl IntoElement {
    let title = card.title.clone();
    let footer = card.footer.clone();

    v_flex()
        .flex_1()
        .w_0()
        .min_w_0()
        .h(px(150.))
        .p(px(12.))
        .gap(px(8.))
        .rounded(px(4.))
        .border_1()
        .border_color(theme_border(theme_mode))
        .bg(metric_card_bg(theme_mode))
        .overflow_hidden()
        .child(
            h_flex()
                .min_w_0()
                .items_center()
                .gap(px(6.))
                .child(
                    Icon::new(Icon::empty())
                        .path(card.icon)
                        .with_size(px(16.))
                        .text_color(theme_muted(theme_mode)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .line_height(relative(1.2))
                        .text_color(theme_muted(theme_mode))
                        .truncate()
                        .child(title),
                ),
        )
        .child(metric_card_body(&card, history, theme_mode))
        .child(
            div().flex_none().w_full().overflow_hidden().child(
                div()
                    .text_size(px(10.))
                    .line_height(px(14.))
                    .text_color(theme_muted(theme_mode))
                    .whitespace_normal()
                    .line_clamp(2)
                    .child(footer),
            ),
        )
}

fn metric_card_body(
    card: &MetricCard,
    history: &[ContainerMetricPoint],
    theme_mode: ThemeMode,
) -> impl IntoElement {
    v_flex()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .justify_between()
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .overflow_hidden()
                .items_end()
                .justify_start()
                .child(
                    h_flex()
                        .min_w_0()
                        .items_end()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(px(20.))
                                .line_height(px(20.))
                                .text_color(theme_text(theme_mode))
                                .child(card.value.clone()),
                        )
                        .when(!card.unit.is_empty(), |this| {
                            this.child(
                                div()
                                    .flex_none()
                                    .text_xs()
                                    .line_height(px(14.))
                                    .text_color(theme_text(theme_mode))
                                    .child(card.unit.clone()),
                            )
                        }),
                ),
        )
        .child(
            div()
                .h(px(40.))
                .w_full()
                .overflow_hidden()
                .when_some(card.chart, |this, chart| {
                    if !history.is_empty() {
                        this.child(metric_chart(chart, history, theme_mode))
                    } else {
                        this
                    }
                }),
        )
}

fn metric_chart(
    chart: MetricChart,
    history: &[ContainerMetricPoint],
    theme_mode: ThemeMode,
) -> impl IntoElement {
    let color = match chart {
        MetricChart::Cpu => hsla(0.674, 0.78, 0.56, 1.0),
        MetricChart::Memory => hsla(0.423, 0.68, 0.48, 1.0),
        MetricChart::Network => hsla(0.555, 0.72, 0.52, 1.0),
        MetricChart::Disk => hsla(0.145, 0.82, 0.52, 1.0),
    };
    let values = history.iter().map(move |point| match chart {
        MetricChart::Cpu => point.cpu_percent,
        MetricChart::Memory => point.memory_bytes,
        MetricChart::Network => point.network_bytes_per_sec,
        MetricChart::Disk => point.disk_bytes_per_sec,
    });

    realtime_area_chart(vec![RealtimeSeries::new(
        values,
        color,
        linear_gradient(
            0.,
            linear_color_stop(color.opacity(0.42), 1.),
            linear_color_stop(theme_content_bg(theme_mode).opacity(0.08), 0.),
        ),
    )])
}

struct MetricCard {
    icon: &'static str,
    title: String,
    value: String,
    unit: String,
    footer: String,
    chart: Option<MetricChart>,
}

#[derive(Clone, Copy)]
enum MetricChart {
    Cpu,
    Memory,
    Network,
    Disk,
}

fn cpu_footer(stats: &ContainerRuntimeStats) -> Option<String> {
    stats.online_cpus.map(|cpus| {
        format!(
            "{} / {} CPU",
            format_number(stats.cpu_percent / 100., 2),
            cpus
        )
    })
}

fn memory_footer(stats: &ContainerRuntimeStats) -> String {
    let Some(limit) = stats.memory_limit_bytes else {
        return t!("detail.no_limit").to_string();
    };
    let usage = stats.memory_usage_bytes.unwrap_or_default();
    let percent = if limit == 0 {
        0.
    } else {
        usage as f64 / limit as f64 * 100.
    };
    let (value, unit) = format_bytes_value(limit as f64);
    format!("{}% / {}{}", format_number(percent, 1), value, unit)
}

fn network_footer(stats: &ContainerRuntimeStats) -> String {
    format!(
        "{} {} / {} {}",
        t!("detail.rx"),
        format_rate(stats.network_rx_bytes_per_sec),
        t!("detail.tx"),
        format_rate(stats.network_tx_bytes_per_sec)
    )
}

fn disk_footer(stats: &ContainerRuntimeStats) -> String {
    format!(
        "{} {} / {} {}",
        t!("detail.read"),
        format_rate(stats.disk_read_bytes_per_sec),
        t!("detail.write"),
        format_rate(stats.disk_write_bytes_per_sec)
    )
}

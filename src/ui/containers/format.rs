use crate::{
    bridge::{ContainerDetailSnapshot, ContainerDetailStatus},
    domain::{ContainerDetail, ContainerMountSummary, ContainerPortSummary},
};
use rust_i18n::t;

pub(super) fn short_id(id: &str) -> String {
    id.chars().take(12).collect()
}

pub(super) fn format_ports(ports: &[ContainerPortSummary]) -> String {
    if ports.is_empty() {
        return "-".to_string();
    }

    ports
        .iter()
        .map(|port| match port.public_port {
            Some(public_port) => format!(
                "{}:{}/{}",
                public_port,
                port.private_port,
                port.protocol.as_deref().unwrap_or("tcp")
            ),
            None => format!(
                "{}/{}",
                port.private_port,
                port.protocol.as_deref().unwrap_or("tcp")
            ),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn format_mounts(detail: &ContainerDetail) -> String {
    if detail.mounts.is_empty() {
        return "-".to_string();
    }

    detail
        .mounts
        .iter()
        .take(2)
        .map(format_mount)
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn format_environment(detail: &ContainerDetail) -> String {
    let keys = detail
        .environment
        .iter()
        .map(|entry| {
            entry
                .split_once('=')
                .map_or(entry.as_str(), |(key, _)| key)
                .to_string()
        })
        .filter(|key| !key.is_empty())
        .collect::<Vec<_>>();
    format_counted_list(&keys)
}

pub(super) fn format_labels(detail: &ContainerDetail) -> String {
    let labels = detail
        .labels
        .iter()
        .map(|label| format!("{}={}", label.key, label.value))
        .collect::<Vec<_>>();
    format_counted_list(&labels)
}

pub(super) fn format_full_mounts(detail: &ContainerDetail) -> Option<String> {
    format_full_list(detail.mounts.iter().map(format_mount))
}

pub(super) fn format_full_environment(detail: &ContainerDetail) -> Option<String> {
    format_full_list(detail.environment.iter().cloned())
}

pub(super) fn format_full_labels(detail: &ContainerDetail) -> Option<String> {
    format_full_list(
        detail
            .labels
            .iter()
            .map(|label| format!("{}={}", label.key, label.value)),
    )
}

fn format_mount(mount: &ContainerMountSummary) -> String {
    mount
        .source
        .as_ref()
        .map(|source| format!("{}:{}", source, mount.destination))
        .unwrap_or_else(|| mount.destination.clone())
}

fn format_full_list(values: impl IntoIterator<Item = String>) -> Option<String> {
    let values = values
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    (!values.is_empty()).then(|| values.join("\n"))
}

pub(super) fn format_string_list(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(" ")
    }
}

pub(super) fn format_counted_list(values: &[String]) -> String {
    if values.is_empty() {
        return "-".to_string();
    }

    let shown = values
        .iter()
        .take(2)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let remaining = values.len().saturating_sub(2);
    if remaining == 0 {
        shown
    } else {
        format!("{}, +{} {}", shown, remaining, t!("detail.more_items"))
    }
}

pub(super) fn unavailable(snapshot: Option<&ContainerDetailSnapshot>) -> String {
    match snapshot.map(|snapshot| snapshot.status) {
        Some(ContainerDetailStatus::Error) => t!("detail.unavailable").to_string(),
        Some(ContainerDetailStatus::Loading) | None => t!("detail.loading_short").to_string(),
        _ => "-".to_string(),
    }
}

pub(super) fn format_rate(bytes_per_sec: f64) -> String {
    let (value, unit) = format_bytes_value(bytes_per_sec);
    format!("{}{}/s", value, unit)
}

pub(super) fn format_bytes(bytes: u64) -> String {
    let (value, unit) = format_bytes_value(bytes as f64);
    format!("{}{}", value, unit)
}

pub(super) fn format_bytes_value(bytes: f64) -> (String, String) {
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

    (
        format_number(value, if value >= 10. { 1 } else { 2 }),
        unit.to_string(),
    )
}

pub(super) fn format_number(value: f64, decimals: usize) -> String {
    let formatted = format!("{:.*}", decimals, value);
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

pub(super) fn format_timestamp(value: &str) -> String {
    value
        .split_once('.')
        .map(|(head, _)| head)
        .unwrap_or(value)
        .replace('T', " ")
        .trim_end_matches('Z')
        .to_string()
}

pub(super) fn format_uptime_value(started_at: &str) -> (String, String) {
    let Ok(started_at) =
        time::OffsetDateTime::parse(started_at, &time::format_description::well_known::Rfc3339)
    else {
        return ("--".to_string(), String::new());
    };

    let elapsed = time::OffsetDateTime::now_utc() - started_at;
    let total_minutes = elapsed.whole_minutes().max(0);
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;

    if hours > 0 {
        (hours.to_string(), format!("h {}m", minutes))
    } else {
        (minutes.to_string(), "m".to_string())
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{ContainerDetail, ContainerMountSummary, ContainerPortSummary};

    use super::{
        format_bytes_value, format_counted_list, format_environment, format_full_environment,
        format_full_labels, format_full_mounts, format_mounts, format_ports, format_string_list,
        format_timestamp,
    };

    #[test]
    fn formats_config_lists_with_counts() {
        assert_eq!(format_string_list(&[]), "-");
        assert_eq!(
            format_counted_list(&["ONE".to_string(), "TWO".to_string(), "THREE".to_string()]),
            "ONE, TWO, +1 more"
        );
    }

    #[test]
    fn formats_ports_and_mounts_for_detail_summary() {
        let ports = vec![
            ContainerPortSummary::new(80, Some(8080), Some("tcp".to_string())),
            ContainerPortSummary::new(443, None, Some("tcp".to_string())),
        ];
        let mounts = vec![
            ContainerMountSummary::new(
                Some("/host/data".to_string()),
                "/data".to_string(),
                Some("bind".to_string()),
                false,
            ),
            ContainerMountSummary::new(None, "/cache".to_string(), None, true),
        ];
        let detail = ContainerDetail {
            id: "abc".to_string(),
            image: "echo:latest".to_string(),
            created_at: None,
            started_at: None,
            restart_count: 0,
            restart_policy: None,
            user: None,
            working_dir: None,
            entrypoint: Vec::new(),
            command: Vec::new(),
            ports: Vec::new(),
            mounts,
            environment: vec![
                "DATABASE_URL=postgres://localhost/echo".to_string(),
                "RUST_LOG=debug".to_string(),
                "EMPTY=".to_string(),
            ],
            labels: Vec::new(),
            size_rw_bytes: None,
            size_root_fs_bytes: None,
        };

        assert_eq!(format_ports(&ports), "8080:80/tcp, 443/tcp");
        assert_eq!(format_mounts(&detail), "/host/data:/data, /cache");
        assert_eq!(
            format_full_mounts(&detail).as_deref(),
            Some("/host/data:/data\n/cache")
        );
        assert_eq!(
            format_environment(&detail),
            "DATABASE_URL, RUST_LOG, +1 more"
        );
        assert_eq!(
            format_full_environment(&detail).as_deref(),
            Some("DATABASE_URL=postgres://localhost/echo\nRUST_LOG=debug\nEMPTY=")
        );
        assert_eq!(format_full_labels(&detail), None);
    }

    #[test]
    fn formats_bytes_and_timestamps_compactly() {
        assert_eq!(
            format_bytes_value(1_536.),
            ("1.5".to_string(), "KB".to_string())
        );
        assert_eq!(
            format_timestamp("2026-05-19T10:20:30.123456Z"),
            "2026-05-19 10:20:30"
        );
    }
}

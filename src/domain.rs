use std::{path::PathBuf, time::SystemTime};

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub enum ConnectionTarget {
    DefaultContext,
    DockerHost(String),
    LocalSocket(PathBuf),
    Ssh {
        host: String,
        user: Option<String>,
        port: Option<u16>,
    },
    BoxLite,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveConnection {
    pub id: String,
    pub name: String,
    pub target: ConnectionTarget,
}

impl ActiveConnection {
    pub fn local_current(target: ConnectionTarget) -> Self {
        Self {
            id: target.stable_id(),
            name: target.display_name(),
            target,
        }
    }

    pub fn endpoint(&self) -> String {
        self.target.endpoint()
    }
}

impl ConnectionTarget {
    pub fn stable_id(&self) -> String {
        match self {
            ConnectionTarget::DefaultContext => "docker:default".to_string(),
            ConnectionTarget::DockerHost(host) => format!("docker:host:{}", host),
            ConnectionTarget::LocalSocket(path) => {
                format!("docker:socket:{}", path.to_string_lossy())
            }
            ConnectionTarget::Ssh { host, user, port } => {
                let user = user.as_deref().unwrap_or("");
                let port = port.map(|port| port.to_string()).unwrap_or_default();
                format!("docker:ssh:{}@{}:{}", user, host, port)
            }
            ConnectionTarget::BoxLite => "boxlite".to_string(),
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            ConnectionTarget::DefaultContext => "Current Docker context".to_string(),
            ConnectionTarget::DockerHost(host) => format!("Docker host {}", host),
            ConnectionTarget::LocalSocket(path) => {
                format!("Docker socket {}", path.display())
            }
            ConnectionTarget::Ssh { host, user, port } => {
                let user_prefix = user
                    .as_deref()
                    .filter(|user| !user.is_empty())
                    .map(|user| format!("{}@", user))
                    .unwrap_or_default();
                let port_suffix = port.map(|port| format!(":{}", port)).unwrap_or_default();
                format!("SSH Docker {}{}{}", user_prefix, host, port_suffix)
            }
            ConnectionTarget::BoxLite => "BoxLite".to_string(),
        }
    }

    pub fn endpoint(&self) -> String {
        match self {
            ConnectionTarget::DefaultContext => "Docker defaults".to_string(),
            ConnectionTarget::DockerHost(host) => host.clone(),
            ConnectionTarget::LocalSocket(path) => path.display().to_string(),
            ConnectionTarget::Ssh { host, user, port } => {
                let user_prefix = user
                    .as_deref()
                    .filter(|user| !user.is_empty())
                    .map(|user| format!("{}@", user))
                    .unwrap_or_default();
                let port_suffix = port.map(|port| format!(":{}", port)).unwrap_or_default();
                format!("ssh://{}{}{}", user_prefix, host, port_suffix)
            }
            ConnectionTarget::BoxLite => "Embedded BoxLite".to_string(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerSummary {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub state: Option<String>,
    pub health: Option<String>,
    pub ports: Vec<ContainerPortSummary>,
    pub is_compose: bool,
    pub compose: Option<ComposeMetadata>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComposeMetadata {
    pub project: String,
    pub service: Option<String>,
}

impl ComposeMetadata {
    pub fn new(project: String, service: Option<String>) -> Self {
        Self { project, service }
    }
}

impl ContainerSummary {
    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    pub fn new(
        id: String,
        name: String,
        image: String,
        status: String,
        state: Option<String>,
        health: Option<String>,
        ports: Vec<ContainerPortSummary>,
        is_compose: bool,
    ) -> Self {
        Self::new_with_compose(
            id, name, image, status, state, health, ports, None, is_compose,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_compose(
        id: String,
        name: String,
        image: String,
        status: String,
        state: Option<String>,
        health: Option<String>,
        ports: Vec<ContainerPortSummary>,
        compose: Option<ComposeMetadata>,
        is_compose: bool,
    ) -> Self {
        Self {
            id,
            name,
            image,
            status,
            state,
            health,
            ports,
            is_compose: is_compose || compose.is_some(),
            compose,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerPortSummary {
    pub private_port: u16,
    pub public_port: Option<u16>,
    pub protocol: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageSummary {
    pub id: String,
    pub repo_tags: Vec<String>,
    pub repo_digests: Vec<String>,
    pub size_bytes: u64,
    pub created_at: Option<SystemTime>,
    pub containers: Option<u64>,
}

impl ImageSummary {
    pub fn new(
        id: String,
        repo_tags: Vec<String>,
        repo_digests: Vec<String>,
        size_bytes: u64,
        created_at: Option<SystemTime>,
        containers: Option<u64>,
    ) -> Self {
        Self {
            id,
            repo_tags,
            repo_digests,
            size_bytes,
            created_at,
            containers,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolumeSummary {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub size_bytes: Option<u64>,
    pub created_at: Option<SystemTime>,
    pub ref_count: Option<u64>,
}

impl VolumeSummary {
    pub fn new(
        name: String,
        driver: String,
        mountpoint: String,
        size_bytes: Option<u64>,
        created_at: Option<SystemTime>,
        ref_count: Option<u64>,
    ) -> Self {
        Self {
            name,
            driver,
            mountpoint,
            size_bytes,
            created_at,
            ref_count,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkSummary {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: Option<String>,
    pub subnet: Option<String>,
    pub gateway: Option<String>,
    pub ipv6_enabled: bool,
    pub created_at: Option<SystemTime>,
    pub labels: Vec<NetworkLabelSummary>,
    pub endpoints: Vec<NetworkEndpointSummary>,
}

impl NetworkSummary {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        name: String,
        driver: String,
        scope: Option<String>,
        subnet: Option<String>,
        gateway: Option<String>,
        ipv6_enabled: bool,
        created_at: Option<SystemTime>,
        labels: Vec<NetworkLabelSummary>,
        endpoints: Vec<NetworkEndpointSummary>,
    ) -> Self {
        Self {
            id,
            name,
            driver,
            scope,
            subnet,
            gateway,
            ipv6_enabled,
            created_at,
            labels,
            endpoints,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkEndpointSummary {
    pub container_id: String,
    pub name: String,
    pub endpoint_id: Option<String>,
    pub mac_address: Option<String>,
    pub ipv4_address: Option<String>,
    pub ipv6_address: Option<String>,
}

impl NetworkEndpointSummary {
    pub fn new(
        container_id: String,
        name: String,
        endpoint_id: Option<String>,
        mac_address: Option<String>,
        ipv4_address: Option<String>,
        ipv6_address: Option<String>,
    ) -> Self {
        Self {
            container_id,
            name,
            endpoint_id,
            mac_address,
            ipv4_address,
            ipv6_address,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkLabelSummary {
    pub key: String,
    pub value: String,
}

impl NetworkLabelSummary {
    pub fn new(key: String, value: String) -> Self {
        Self { key, value }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContainerDetail {
    pub id: String,
    pub image: String,
    pub created_at: Option<String>,
    pub started_at: Option<String>,
    pub restart_count: u64,
    pub restart_policy: Option<String>,
    pub user: Option<String>,
    pub working_dir: Option<String>,
    pub entrypoint: Vec<String>,
    pub command: Vec<String>,
    pub ports: Vec<ContainerPortSummary>,
    pub mounts: Vec<ContainerMountSummary>,
    pub environment: Vec<String>,
    pub labels: Vec<ContainerLabelSummary>,
    pub size_rw_bytes: Option<u64>,
    pub size_root_fs_bytes: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerMountSummary {
    pub source: Option<String>,
    pub destination: String,
    pub kind: Option<String>,
    pub read_only: bool,
}

impl ContainerMountSummary {
    pub fn new(
        source: Option<String>,
        destination: String,
        kind: Option<String>,
        read_only: bool,
    ) -> Self {
        Self {
            source,
            destination,
            kind,
            read_only,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerLabelSummary {
    pub key: String,
    pub value: String,
}

impl ContainerLabelSummary {
    pub fn new(key: String, value: String) -> Self {
        Self { key, value }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerLogStreamKind {
    Stdout,
    Stderr,
    Stdin,
    Console,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerLogLine {
    pub timestamp: Option<String>,
    pub stream: ContainerLogStreamKind,
    pub message: String,
}

impl ContainerLogLine {
    pub fn new(timestamp: Option<String>, stream: ContainerLogStreamKind, message: String) -> Self {
        Self {
            timestamp,
            stream,
            message,
        }
    }

    pub fn searchable_text(&self) -> String {
        format!(
            "{} {}",
            self.timestamp.as_deref().unwrap_or_default(),
            self.message
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContainerRuntimeStats {
    pub sample_time: SystemTime,
    pub cpu_percent: f64,
    pub online_cpus: Option<u32>,
    pub memory_usage_bytes: Option<u64>,
    pub memory_limit_bytes: Option<u64>,
    pub network_rx_bytes_per_sec: f64,
    pub network_tx_bytes_per_sec: f64,
    pub disk_read_bytes_per_sec: f64,
    pub disk_write_bytes_per_sec: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContainerMetricPoint {
    pub sequence: u64,
    pub sample_time: SystemTime,
    pub cpu_percent: f64,
    pub memory_bytes: f64,
    pub network_bytes_per_sec: f64,
    pub disk_bytes_per_sec: f64,
}

impl ContainerMetricPoint {
    pub fn from_stats(stats: &ContainerRuntimeStats, sequence: u64) -> Self {
        Self {
            sequence,
            sample_time: stats.sample_time,
            cpu_percent: stats.cpu_percent,
            memory_bytes: stats.memory_usage_bytes.unwrap_or_default() as f64,
            network_bytes_per_sec: stats.network_rx_bytes_per_sec + stats.network_tx_bytes_per_sec,
            disk_bytes_per_sec: stats.disk_read_bytes_per_sec + stats.disk_write_bytes_per_sec,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum NetworkThroughputTarget {
    Network {
        network_id: String,
        container_ids: Vec<String>,
    },
    Container {
        network_id: String,
        container_id: String,
        is_running: bool,
    },
}

impl NetworkThroughputTarget {
    pub fn container_ids(&self) -> Vec<String> {
        match self {
            Self::Network { container_ids, .. } => container_ids.clone(),
            Self::Container {
                container_id,
                is_running,
                ..
            } => {
                if *is_running {
                    vec![container_id.clone()]
                } else {
                    Vec::new()
                }
            }
        }
    }

    pub fn is_idle(&self) -> bool {
        match self {
            Self::Network { container_ids, .. } => container_ids.is_empty(),
            Self::Container { is_running, .. } => !*is_running,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetworkThroughputStats {
    pub sample_time: SystemTime,
    pub rx_bytes_per_sec: f64,
    pub tx_bytes_per_sec: f64,
}

impl NetworkThroughputStats {
    pub fn zero(sample_time: SystemTime) -> Self {
        Self {
            sample_time,
            rx_bytes_per_sec: 0.,
            tx_bytes_per_sec: 0.,
        }
    }

    pub fn total_bytes_per_sec(&self) -> f64 {
        self.rx_bytes_per_sec + self.tx_bytes_per_sec
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetworkThroughputPoint {
    pub sequence: u64,
    pub sample_time: SystemTime,
    pub rx_bytes_per_sec: f64,
    pub tx_bytes_per_sec: f64,
}

impl NetworkThroughputPoint {
    pub fn from_stats(stats: &NetworkThroughputStats, sequence: u64) -> Self {
        Self {
            sequence,
            sample_time: stats.sample_time,
            rx_bytes_per_sec: stats.rx_bytes_per_sec,
            tx_bytes_per_sec: stats.tx_bytes_per_sec,
        }
    }
}

impl ContainerPortSummary {
    pub fn new(private_port: u16, public_port: Option<u16>, protocol: Option<String>) -> Self {
        Self {
            private_port,
            public_port,
            protocol,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::UNIX_EPOCH};

    use super::{ConnectionTarget, ContainerMetricPoint, ContainerRuntimeStats};

    #[test]
    fn creates_stable_connection_ids() {
        assert_eq!(
            ConnectionTarget::DefaultContext.stable_id(),
            "docker:default"
        );
        assert_eq!(
            ConnectionTarget::DockerHost("unix:///var/run/docker.sock".to_string()).stable_id(),
            "docker:host:unix:///var/run/docker.sock"
        );
        assert_eq!(
            ConnectionTarget::LocalSocket(PathBuf::from("/tmp/docker.sock")).stable_id(),
            "docker:socket:/tmp/docker.sock"
        );
    }

    #[test]
    fn creates_readable_connection_names() {
        assert_eq!(
            ConnectionTarget::DefaultContext.display_name(),
            "Current Docker context"
        );
        assert_eq!(
            ConnectionTarget::Ssh {
                host: "example.com".to_string(),
                user: Some("deploy".to_string()),
                port: Some(2222),
            }
            .display_name(),
            "SSH Docker deploy@example.com:2222"
        );
    }

    #[test]
    fn creates_readable_connection_endpoints() {
        assert_eq!(
            ConnectionTarget::DefaultContext.endpoint(),
            "Docker defaults"
        );
        assert_eq!(
            ConnectionTarget::DockerHost("unix:///tmp/docker.sock".to_string()).endpoint(),
            "unix:///tmp/docker.sock"
        );
        assert_eq!(
            ConnectionTarget::LocalSocket(PathBuf::from("/tmp/docker.sock")).endpoint(),
            "/tmp/docker.sock"
        );
        assert_eq!(
            ConnectionTarget::Ssh {
                host: "example.com".to_string(),
                user: Some("deploy".to_string()),
                port: Some(2222),
            }
            .endpoint(),
            "ssh://deploy@example.com:2222"
        );
    }

    #[test]
    fn metric_points_preserve_sample_sequence_and_time() {
        let stats = ContainerRuntimeStats {
            sample_time: UNIX_EPOCH,
            cpu_percent: 12.5,
            online_cpus: Some(4),
            memory_usage_bytes: Some(2_048),
            memory_limit_bytes: Some(4_096),
            network_rx_bytes_per_sec: 10.,
            network_tx_bytes_per_sec: 20.,
            disk_read_bytes_per_sec: 30.,
            disk_write_bytes_per_sec: 40.,
        };

        let point = ContainerMetricPoint::from_stats(&stats, 42);

        assert_eq!(point.sequence, 42);
        assert_eq!(point.sample_time, UNIX_EPOCH);
        assert_eq!(point.network_bytes_per_sec, 30.);
        assert_eq!(point.disk_bytes_per_sec, 70.);
    }
}

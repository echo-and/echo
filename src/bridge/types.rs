use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use crate::domain::{
    ContainerDetail, ContainerLogLine, ContainerMetricPoint, ContainerRuntimeStats,
    ContainerSummary, ImageSummary, NetworkSummary, NetworkThroughputPoint, NetworkThroughputStats,
    NetworkThroughputTarget, VolumeSummary,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionStatus {
    Connecting,
    Live,
    Polling,
    Reconnecting,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerSyncSignal {
    Changed(ContainerEventKind),
    Poll,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkSyncSignal {
    Changed,
    Poll,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceSyncSignal {
    Changed,
    Poll,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerEventKind {
    StateChanged,
    Removed,
    Other,
}

#[derive(Clone, Debug)]
pub struct ContainerSnapshot {
    pub containers: Vec<ContainerSummary>,
    pub status: ConnectionStatus,
    pub error: Option<String>,
    pub retry_after: Option<Duration>,
    pub last_updated: Option<SystemTime>,
}

#[derive(Clone, Debug)]
pub struct ImageSnapshot {
    pub images: Vec<ImageSummary>,
    pub error: Option<String>,
    pub last_updated: Option<SystemTime>,
}

#[derive(Clone, Debug)]
pub struct VolumeSnapshot {
    pub volumes: Vec<VolumeSummary>,
    pub error: Option<String>,
    pub last_updated: Option<SystemTime>,
}

#[derive(Clone, Debug)]
pub struct NetworkSnapshot {
    pub networks: Vec<NetworkSummary>,
    pub error: Option<String>,
    pub last_updated: Option<SystemTime>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkCreateConfig {
    pub name: String,
    pub driver: String,
    pub subnet: Option<String>,
    pub gateway: Option<String>,
    pub enable_ipv6: bool,
    pub internal: bool,
}

impl NetworkSnapshot {
    pub fn loading() -> Self {
        Self {
            networks: Vec::new(),
            error: None,
            last_updated: None,
        }
    }
}

impl ImageSnapshot {
    pub fn loading() -> Self {
        Self {
            images: Vec::new(),
            error: None,
            last_updated: None,
        }
    }
}

impl VolumeSnapshot {
    pub fn loading() -> Self {
        Self {
            volumes: Vec::new(),
            error: None,
            last_updated: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkThroughputStatus {
    Loading,
    Live,
    Idle,
    Reconnecting,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetworkThroughputSnapshot {
    pub target: NetworkThroughputTarget,
    pub latest: Option<NetworkThroughputStats>,
    pub history: Vec<NetworkThroughputPoint>,
    pub status: NetworkThroughputStatus,
    pub error: Option<String>,
    pub last_updated: Option<SystemTime>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerDetailStatus {
    Loading,
    Live,
    Stopped,
    Reconnecting,
    Error,
}

#[derive(Clone, Debug)]
pub struct ContainerDetailSnapshot {
    pub container_id: String,
    pub detail: Option<ContainerDetail>,
    pub latest: Option<ContainerRuntimeStats>,
    pub history: Vec<ContainerMetricPoint>,
    pub status: ContainerDetailStatus,
    pub error: Option<String>,
    pub last_updated: Option<SystemTime>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerLogsStatus {
    Loading,
    Live,
    Stopped,
    Reconnecting,
    Error,
}

#[derive(Clone, Debug)]
pub struct ContainerLogsSnapshot {
    pub container_id: String,
    pub lines: Arc<Vec<ContainerLogLine>>,
    pub status: ContainerLogsStatus,
    pub error: Option<String>,
    pub last_updated: Option<SystemTime>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerShellStatus {
    Loading,
    Live,
    Stopped,
    Exited,
    Error,
}

#[derive(Clone, Debug)]
pub struct ContainerShellSnapshot {
    pub container_id: String,
    pub status: ContainerShellStatus,
    pub error: Option<String>,
    pub last_updated: Option<SystemTime>,
}

impl ContainerSnapshot {
    pub fn connecting() -> Self {
        Self {
            containers: Vec::new(),
            status: ConnectionStatus::Connecting,
            error: None,
            retry_after: None,
            last_updated: None,
        }
    }
}

impl ContainerDetailSnapshot {
    pub fn loading(container_id: String) -> Self {
        Self {
            container_id,
            detail: None,
            latest: None,
            history: Vec::new(),
            status: ContainerDetailStatus::Loading,
            error: None,
            last_updated: None,
        }
    }
}

impl NetworkThroughputSnapshot {
    pub fn loading(target: NetworkThroughputTarget) -> Self {
        Self {
            target,
            latest: None,
            history: Vec::new(),
            status: NetworkThroughputStatus::Loading,
            error: None,
            last_updated: None,
        }
    }
}

impl ContainerLogsSnapshot {
    pub fn loading(container_id: String) -> Self {
        Self {
            container_id,
            lines: Arc::new(Vec::new()),
            status: ContainerLogsStatus::Loading,
            error: None,
            last_updated: None,
        }
    }
}

impl ContainerShellSnapshot {
    pub fn loading(container_id: String) -> Self {
        Self {
            container_id,
            status: ContainerShellStatus::Loading,
            error: None,
            last_updated: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConnectionStatus, ContainerSnapshot};

    #[test]
    fn connecting_snapshot_clears_retry_after() {
        let snapshot = ContainerSnapshot::connecting();

        assert_eq!(snapshot.status, ConnectionStatus::Connecting);
        assert_eq!(snapshot.retry_after, None);
    }
}

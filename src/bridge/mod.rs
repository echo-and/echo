mod backends;
mod resolver;
mod session;
mod sync;
mod types;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use tokio::{runtime::Runtime, sync::watch};

pub use types::{
    ConnectionStatus, ContainerDetailSnapshot, ContainerDetailStatus, ContainerLogsSnapshot,
    ContainerLogsStatus, ContainerShellSnapshot, ContainerShellStatus, ContainerSnapshot,
    ImageSnapshot, NetworkCreateConfig, NetworkSnapshot, NetworkThroughputSnapshot,
    NetworkThroughputStatus, VolumeSnapshot,
};

use crate::domain::{ActiveConnection, ConnectionTarget, NetworkThroughputTarget};

pub use self::session::ContainerShellSession;

use self::session::ConnectionSession;

#[derive(Clone)]
pub struct Bridge {
    runtime: Arc<Runtime>,
    sessions: Arc<Mutex<HashMap<String, Arc<ConnectionSession>>>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerAction {
    Start,
    Stop,
    Restart,
    Pause,
    Unpause,
    Remove,
}

impl Bridge {
    pub fn new() -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("failed to create Docker runtime")?;

        Ok(Self {
            runtime: Arc::new(runtime),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn resolve_active_connection(&self) -> ActiveConnection {
        ActiveConnection::local_current(resolver::resolve_current_target())
    }

    pub fn volume_name_from_archive_path(path: &Path) -> String {
        backends::local::volume_name_from_archive_path(path)
    }

    pub fn stop_session(&self, target: &ConnectionTarget) {
        let session = self
            .sessions
            .lock()
            .expect("bridge session lock poisoned")
            .remove(&target.stable_id());

        if let Some(session) = session {
            session.stop();
        }
    }

    pub fn subscribe_containers(
        &self,
        target: ConnectionTarget,
    ) -> watch::Receiver<ContainerSnapshot> {
        self.session(target).subscribe()
    }

    pub fn subscribe_networks(&self, target: ConnectionTarget) -> watch::Receiver<NetworkSnapshot> {
        self.session(target).subscribe_networks()
    }

    pub fn subscribe_images(&self, target: ConnectionTarget) -> watch::Receiver<ImageSnapshot> {
        self.session(target).subscribe_images()
    }

    pub fn subscribe_volumes(&self, target: ConnectionTarget) -> watch::Receiver<VolumeSnapshot> {
        self.session(target).subscribe_volumes()
    }

    pub fn subscribe_network_throughput(
        &self,
        target: ConnectionTarget,
        throughput_target: NetworkThroughputTarget,
    ) -> watch::Receiver<NetworkThroughputSnapshot> {
        self.session(target)
            .subscribe_network_throughput(throughput_target)
    }

    pub fn subscribe_container_detail(
        &self,
        target: ConnectionTarget,
        container_id: String,
    ) -> watch::Receiver<ContainerDetailSnapshot> {
        self.session(target)
            .subscribe_container_detail(container_id)
    }

    pub fn subscribe_container_logs(
        &self,
        target: ConnectionTarget,
        container_id: String,
    ) -> watch::Receiver<ContainerLogsSnapshot> {
        self.session(target).subscribe_container_logs(container_id)
    }

    pub fn subscribe_container_shell(
        &self,
        target: ConnectionTarget,
        container_id: String,
    ) -> watch::Receiver<ContainerShellSnapshot> {
        self.session(target).subscribe_container_shell(container_id)
    }

    pub fn open_container_shell(
        &self,
        target: ConnectionTarget,
        container_id: String,
    ) -> Result<ContainerShellSession> {
        self.session(target).open_container_shell(container_id)
    }

    pub fn refresh_containers(&self, target: ConnectionTarget) -> Result<ContainerSnapshot> {
        let session = self.session(target);
        self.runtime.block_on(async { session.refresh().await })
    }

    pub fn control_container(
        &self,
        target: ConnectionTarget,
        container_id: String,
        action: ContainerAction,
    ) -> Result<()> {
        let session = self.session(target);
        self.runtime
            .block_on(async { session.control_container(container_id, action).await })
    }

    pub fn refresh_images(&self, target: ConnectionTarget) -> Result<ImageSnapshot> {
        let session = self.session(target);
        self.runtime
            .block_on(async { session.refresh_images().await })
    }

    pub fn remove_image(&self, target: ConnectionTarget, image_id: String) -> Result<()> {
        self.runtime
            .block_on(async { backends::local::remove_image(target, &image_id).await })
    }

    pub fn import_image(&self, target: ConnectionTarget, archive_path: PathBuf) -> Result<()> {
        self.runtime
            .block_on(async { backends::local::import_image(target, archive_path).await })
    }

    pub fn refresh_volumes(&self, target: ConnectionTarget) -> Result<VolumeSnapshot> {
        let session = self.session(target);
        self.runtime
            .block_on(async { session.refresh_volumes().await })
    }

    pub fn refresh_networks(&self, target: ConnectionTarget) -> Result<NetworkSnapshot> {
        let session = self.session(target);
        self.runtime
            .block_on(async { session.refresh_networks().await })
    }

    pub fn create_network(
        &self,
        target: ConnectionTarget,
        config: NetworkCreateConfig,
    ) -> Result<String> {
        self.runtime
            .block_on(async { backends::local::create_network(target, config).await })
    }

    pub fn remove_network(&self, target: ConnectionTarget, network_id: String) -> Result<()> {
        self.runtime
            .block_on(async { backends::local::remove_network(target, &network_id).await })
    }

    pub fn remove_volume(&self, target: ConnectionTarget, volume_name: String) -> Result<()> {
        self.runtime
            .block_on(async { backends::local::remove_volume(target, &volume_name).await })
    }

    pub fn import_volume_archive(
        &self,
        target: ConnectionTarget,
        archive_path: PathBuf,
        volume_name: String,
    ) -> Result<()> {
        self.runtime.block_on(async {
            backends::local::import_volume_archive(target, archive_path, volume_name).await
        })
    }

    fn session(&self, target: ConnectionTarget) -> Arc<ConnectionSession> {
        let id = target.stable_id();
        let mut sessions = self.sessions.lock().expect("bridge session lock poisoned");

        if let Some(session) = sessions.get(&id) {
            session.start();
            return session.clone();
        }

        let session = ConnectionSession::new(target, self.runtime.clone());
        session.start();
        sessions.insert(id, session.clone());
        session
    }
}

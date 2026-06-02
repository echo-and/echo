use std::{
    collections::{HashMap, VecDeque},
    io::{self, Read, Write},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime},
};

use anyhow::{Context as _, Result};
use futures_util::StreamExt;
use tokio::{
    io::AsyncWriteExt,
    runtime::Runtime,
    sync::{mpsc, watch},
    task::JoinHandle,
};

use crate::{
    bridge::{
        ContainerAction,
        backends::local,
        sync,
        types::{
            ConnectionStatus, ContainerDetailSnapshot, ContainerDetailStatus, ContainerEventKind,
            ContainerLogsSnapshot, ContainerLogsStatus, ContainerShellSnapshot,
            ContainerShellStatus, ContainerSnapshot, ContainerSyncSignal, ImageSnapshot,
            NetworkSnapshot, NetworkSyncSignal, NetworkThroughputSnapshot, NetworkThroughputStatus,
            ResourceSyncSignal, VolumeSnapshot,
        },
    },
    domain::{
        ConnectionTarget, ContainerLogLine, ContainerMetricPoint, ContainerRuntimeStats,
        ContainerSummary, ImageSummary, NetworkSummary, NetworkThroughputPoint,
        NetworkThroughputStats, NetworkThroughputTarget, VolumeSummary,
    },
};

const EVENT_REFRESH_DEBOUNCE: Duration = Duration::from_millis(300);
const POLL_INTERVAL: Duration = Duration::from_secs(30);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(10);
const EVENT_BUFFER_SIZE: usize = 64;
const DETAIL_HISTORY_LIMIT: usize = 60;
const NETWORK_THROUGHPUT_INTERVAL: Duration = Duration::from_secs(1);
const LOG_LINE_LIMIT: usize = 1_000;
const LOG_BATCH_LIMIT: usize = 64;
const LOG_BATCH_INTERVAL: Duration = Duration::from_millis(50);
const SHELL_WRITE_BUFFER_SIZE: usize = 128;
const SHELL_OUTPUT_BUFFER_SIZE: usize = 128;
const DEFAULT_SHELL_COLS: u16 = 100;
const DEFAULT_SHELL_ROWS: u16 = 28;

pub struct ConnectionSession {
    target: ConnectionTarget,
    runtime: Arc<Runtime>,
    snapshot_tx: watch::Sender<ContainerSnapshot>,
    image_snapshot_tx: watch::Sender<ImageSnapshot>,
    volume_snapshot_tx: watch::Sender<VolumeSnapshot>,
    network_snapshot_tx: watch::Sender<NetworkSnapshot>,
    task: Mutex<Option<JoinHandle<()>>>,
    image_task: Mutex<Option<JoinHandle<()>>>,
    volume_task: Mutex<Option<JoinHandle<()>>>,
    network_task: Mutex<Option<JoinHandle<()>>>,
    detail_tasks: Mutex<HashMap<String, ContainerDetailTask>>,
    network_throughput_tasks: Mutex<HashMap<NetworkThroughputTarget, NetworkThroughputTask>>,
    logs_task: Mutex<Option<ContainerLogsTask>>,
    shell_tasks: Mutex<HashMap<String, ContainerShellTask>>,
}

struct ContainerDetailTask {
    is_running: bool,
    tx: watch::Sender<ContainerDetailSnapshot>,
    handle: JoinHandle<()>,
}

struct NetworkThroughputTask {
    tx: watch::Sender<NetworkThroughputSnapshot>,
    handle: JoinHandle<()>,
}

enum NetworkThroughputUpdate {
    Stats(String, ContainerRuntimeStats),
    Error(String, String),
}

struct ContainerLogsTask {
    container_id: String,
    is_running: bool,
    tx: watch::Sender<ContainerLogsSnapshot>,
    handle: JoinHandle<()>,
}

struct ContainerShellTask {
    is_running: bool,
    exec_id: Arc<Mutex<Option<String>>>,
    input_tx: mpsc::Sender<Vec<u8>>,
    output_rx: Option<mpsc::Receiver<Vec<u8>>>,
    resize_tx: mpsc::Sender<(u16, u16)>,
    tx: watch::Sender<ContainerShellSnapshot>,
    handle: JoinHandle<()>,
}

struct ContainerShellOpen {
    input_tx: mpsc::Sender<Vec<u8>>,
    output_rx: Option<mpsc::Receiver<Vec<u8>>>,
    status_rx: watch::Receiver<ContainerShellSnapshot>,
    resize_tx: mpsc::Sender<(u16, u16)>,
}

struct ContainerShellIo {
    status_tx: watch::Sender<ContainerShellSnapshot>,
    input_rx: mpsc::Receiver<Vec<u8>>,
    output_tx: mpsc::Sender<Vec<u8>>,
    resize_rx: mpsc::Receiver<(u16, u16)>,
    exec_id_slot: Arc<Mutex<Option<String>>>,
}

pub struct ContainerShellSession {
    pub reader: ContainerShellReader,
    pub writer: ContainerShellWriter,
    pub status_rx: watch::Receiver<ContainerShellSnapshot>,
    resize_tx: mpsc::Sender<(u16, u16)>,
}

impl ContainerShellSession {
    pub fn resizer(&self) -> ContainerShellResizer {
        ContainerShellResizer {
            tx: self.resize_tx.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ContainerShellResizer {
    tx: mpsc::Sender<(u16, u16)>,
}

impl ContainerShellResizer {
    pub fn resize(&self, cols: u16, rows: u16) {
        let _ = self.tx.try_send((cols.max(1), rows.max(1)));
    }
}

pub struct ContainerShellReader {
    rx: mpsc::Receiver<Vec<u8>>,
    pending: Vec<u8>,
    offset: usize,
}

impl ContainerShellReader {
    fn new(rx: mpsc::Receiver<Vec<u8>>) -> Self {
        Self {
            rx,
            pending: Vec::new(),
            offset: 0,
        }
    }
}

impl Read for ContainerShellReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        loop {
            if self.offset < self.pending.len() {
                let available = self.pending.len() - self.offset;
                let count = available.min(buf.len());
                buf[..count].copy_from_slice(&self.pending[self.offset..self.offset + count]);
                self.offset += count;
                if self.offset >= self.pending.len() {
                    self.pending.clear();
                    self.offset = 0;
                }
                return Ok(count);
            }

            match self.rx.blocking_recv() {
                Some(bytes) if bytes.is_empty() => continue,
                Some(bytes) => {
                    self.pending = bytes;
                    self.offset = 0;
                }
                None => return Ok(0),
            }
        }
    }
}

#[derive(Clone)]
pub struct ContainerShellWriter {
    tx: mpsc::Sender<Vec<u8>>,
}

impl ContainerShellWriter {
    fn new(tx: mpsc::Sender<Vec<u8>>) -> Self {
        Self { tx }
    }
}

impl Write for ContainerShellWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.tx
            .blocking_send(buf.to_vec())
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "container shell closed"))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl ConnectionSession {
    pub fn new(target: ConnectionTarget, runtime: Arc<Runtime>) -> Arc<Self> {
        Arc::new(Self {
            target,
            runtime,
            snapshot_tx: watch::channel(ContainerSnapshot::connecting()).0,
            image_snapshot_tx: watch::channel(ImageSnapshot::loading()).0,
            volume_snapshot_tx: watch::channel(VolumeSnapshot::loading()).0,
            network_snapshot_tx: watch::channel(NetworkSnapshot::loading()).0,
            task: Mutex::new(None),
            image_task: Mutex::new(None),
            volume_task: Mutex::new(None),
            network_task: Mutex::new(None),
            detail_tasks: Mutex::new(HashMap::new()),
            network_throughput_tasks: Mutex::new(HashMap::new()),
            logs_task: Mutex::new(None),
            shell_tasks: Mutex::new(HashMap::new()),
        })
    }

    pub fn start(self: &Arc<Self>) {
        let mut task = self.task.lock().expect("session task lock poisoned");
        if task.is_some() {
            return;
        }

        let session = self.clone();
        *task = Some(self.runtime.spawn(async move {
            session.run().await;
        }));
    }

    pub fn stop(&self) {
        if let Some(task) = self.task.lock().expect("session task lock poisoned").take() {
            task.abort();
        }
        if let Some(task) = self
            .image_task
            .lock()
            .expect("image sync task lock poisoned")
            .take()
        {
            task.abort();
        }
        if let Some(task) = self
            .volume_task
            .lock()
            .expect("volume sync task lock poisoned")
            .take()
        {
            task.abort();
        }
        if let Some(task) = self
            .network_task
            .lock()
            .expect("network sync task lock poisoned")
            .take()
        {
            task.abort();
        }
        for (_, task) in self
            .detail_tasks
            .lock()
            .expect("container detail task lock poisoned")
            .drain()
        {
            task.handle.abort();
        }
        for (_, task) in self
            .network_throughput_tasks
            .lock()
            .expect("network throughput task lock poisoned")
            .drain()
        {
            task.handle.abort();
        }
        if let Some(task) = self
            .logs_task
            .lock()
            .expect("container logs task lock poisoned")
            .take()
        {
            task.handle.abort();
        }
        for (_, task) in self
            .shell_tasks
            .lock()
            .expect("container shell task lock poisoned")
            .drain()
        {
            task.handle.abort();
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<ContainerSnapshot> {
        self.snapshot_tx.subscribe()
    }

    pub fn subscribe_images(self: &Arc<Self>) -> watch::Receiver<ImageSnapshot> {
        self.start_image_sync();
        self.image_snapshot_tx.subscribe()
    }

    pub fn subscribe_volumes(self: &Arc<Self>) -> watch::Receiver<VolumeSnapshot> {
        self.start_volume_sync();
        self.volume_snapshot_tx.subscribe()
    }

    pub fn subscribe_networks(self: &Arc<Self>) -> watch::Receiver<NetworkSnapshot> {
        self.start_network_sync();
        self.network_snapshot_tx.subscribe()
    }

    pub fn subscribe_network_throughput(
        self: &Arc<Self>,
        target: NetworkThroughputTarget,
    ) -> watch::Receiver<NetworkThroughputSnapshot> {
        let mut tasks = self
            .network_throughput_tasks
            .lock()
            .expect("network throughput task lock poisoned");

        tasks.retain(|_, task| {
            if task.tx.receiver_count() == 0 || task.handle.is_finished() {
                task.handle.abort();
                return false;
            }
            true
        });

        if let Some(current) = tasks.get(&target) {
            return current.tx.subscribe();
        }

        let (tx, rx) = watch::channel(NetworkThroughputSnapshot::loading(target.clone()));
        let handle = self.spawn_network_throughput_handle(target.clone(), tx.clone());
        tasks.insert(target, NetworkThroughputTask { tx, handle });
        rx
    }

    fn spawn_network_throughput_handle(
        self: &Arc<Self>,
        target: NetworkThroughputTarget,
        tx: watch::Sender<NetworkThroughputSnapshot>,
    ) -> JoinHandle<()> {
        let session = self.clone();
        self.runtime.spawn(async move {
            session.run_network_throughput(target, tx).await;
        })
    }

    fn start_image_sync(self: &Arc<Self>) {
        let mut task = self
            .image_task
            .lock()
            .expect("image sync task lock poisoned");
        if task.is_some() {
            return;
        }

        let session = self.clone();
        *task = Some(self.runtime.spawn(async move {
            session.run_images().await;
        }));
    }

    fn start_volume_sync(self: &Arc<Self>) {
        let mut task = self
            .volume_task
            .lock()
            .expect("volume sync task lock poisoned");
        if task.is_some() {
            return;
        }

        let session = self.clone();
        *task = Some(self.runtime.spawn(async move {
            session.run_volumes().await;
        }));
    }

    fn start_network_sync(self: &Arc<Self>) {
        let mut task = self
            .network_task
            .lock()
            .expect("network sync task lock poisoned");
        if task.is_some() {
            return;
        }

        let session = self.clone();
        *task = Some(self.runtime.spawn(async move {
            session.run_networks().await;
        }));
    }

    pub fn subscribe_container_detail(
        self: &Arc<Self>,
        container_id: String,
    ) -> watch::Receiver<ContainerDetailSnapshot> {
        let mut tasks = self
            .detail_tasks
            .lock()
            .expect("container detail task lock poisoned");

        if let Some(current) = tasks.get(&container_id) {
            return current.tx.subscribe();
        }

        let is_running = self
            .container_is_running_from_snapshot(&container_id)
            .unwrap_or(true);
        let (task, rx) = self.spawn_container_detail_task(container_id.clone(), is_running);
        tasks.insert(container_id, task);
        rx
    }

    fn spawn_container_detail_task(
        self: &Arc<Self>,
        container_id: String,
        is_running: bool,
    ) -> (
        ContainerDetailTask,
        watch::Receiver<ContainerDetailSnapshot>,
    ) {
        let (tx, rx) = watch::channel(ContainerDetailSnapshot::loading(container_id.clone()));
        let handle =
            self.spawn_container_detail_handle(container_id.clone(), is_running, tx.clone());

        (
            ContainerDetailTask {
                is_running,
                tx,
                handle,
            },
            rx,
        )
    }

    fn spawn_container_detail_handle(
        self: &Arc<Self>,
        container_id: String,
        is_running: bool,
        tx: watch::Sender<ContainerDetailSnapshot>,
    ) -> JoinHandle<()> {
        let session = self.clone();
        self.runtime.spawn(async move {
            session
                .run_container_detail(container_id, is_running, tx)
                .await;
        })
    }

    pub fn subscribe_container_logs(
        self: &Arc<Self>,
        container_id: String,
    ) -> watch::Receiver<ContainerLogsSnapshot> {
        let mut task = self
            .logs_task
            .lock()
            .expect("container logs task lock poisoned");

        if let Some(current) = task.as_ref()
            && current.container_id == container_id
            && !current.handle.is_finished()
        {
            return current.tx.subscribe();
        }

        if let Some(current) = task.take() {
            current.handle.abort();
        }

        let (tx, rx) = watch::channel(ContainerLogsSnapshot::loading(container_id.clone()));
        let is_running = self
            .container_is_running_from_snapshot(&container_id)
            .unwrap_or(true);
        let handle = self.spawn_container_logs_handle(container_id.clone(), tx.clone());

        *task = Some(ContainerLogsTask {
            container_id,
            is_running,
            tx,
            handle,
        });

        rx
    }

    fn spawn_container_logs_handle(
        self: &Arc<Self>,
        container_id: String,
        tx: watch::Sender<ContainerLogsSnapshot>,
    ) -> JoinHandle<()> {
        let session = self.clone();
        self.runtime.spawn(async move {
            session.run_container_logs(container_id, tx).await;
        })
    }

    pub fn subscribe_container_shell(
        self: &Arc<Self>,
        container_id: String,
    ) -> watch::Receiver<ContainerShellSnapshot> {
        self.ensure_container_shell_status(container_id)
    }

    pub fn open_container_shell(
        self: &Arc<Self>,
        container_id: String,
    ) -> Result<ContainerShellSession> {
        let shell = self.ensure_container_shell_task(container_id.clone());
        let output_rx = shell.output_rx.with_context(|| {
            format!(
                "shell output stream is already attached for {}",
                container_id
            )
        })?;

        Ok(ContainerShellSession {
            reader: ContainerShellReader::new(output_rx),
            writer: ContainerShellWriter::new(shell.input_tx),
            status_rx: shell.status_rx,
            resize_tx: shell.resize_tx,
        })
    }

    fn ensure_container_shell_task(self: &Arc<Self>, container_id: String) -> ContainerShellOpen {
        let mut tasks = self
            .shell_tasks
            .lock()
            .expect("container shell task lock poisoned");

        if let Some(current) = tasks.get_mut(&container_id)
            && !current.handle.is_finished()
        {
            if let Some(output_rx) = current.output_rx.take() {
                let input_tx = current.input_tx.clone();
                let status_rx = current.tx.subscribe();
                let resize_tx = current.resize_tx.clone();
                return ContainerShellOpen {
                    input_tx,
                    output_rx: Some(output_rx),
                    status_rx,
                    resize_tx,
                };
            }
            let input_tx = current.input_tx.clone();
            let status_rx = current.tx.subscribe();
            let resize_tx = current.resize_tx.clone();
            return ContainerShellOpen {
                input_tx,
                output_rx: None,
                status_rx,
                resize_tx,
            };
        }

        if let Some(current) = tasks.remove(&container_id) {
            current.handle.abort();
        }

        let is_running = self
            .container_is_running_from_snapshot(&container_id)
            .unwrap_or(true);
        let (input_tx, input_rx) = mpsc::channel(SHELL_WRITE_BUFFER_SIZE);
        let (output_tx, output_rx) = mpsc::channel(SHELL_OUTPUT_BUFFER_SIZE);
        let (resize_tx, resize_rx) = mpsc::channel(SHELL_WRITE_BUFFER_SIZE);
        let (tx, rx) = watch::channel(ContainerShellSnapshot::loading(container_id.clone()));
        let exec_id = Arc::new(Mutex::new(None));
        let handle = self.spawn_container_shell_handle(
            container_id.clone(),
            is_running,
            ContainerShellIo {
                status_tx: tx.clone(),
                input_rx,
                output_tx,
                resize_rx,
                exec_id_slot: exec_id.clone(),
            },
        );

        let status_rx = rx;
        tasks.insert(
            container_id.clone(),
            ContainerShellTask {
                is_running,
                exec_id,
                input_tx: input_tx.clone(),
                output_rx: Some(output_rx),
                resize_tx: resize_tx.clone(),
                tx,
                handle,
            },
        );

        let task = tasks
            .get_mut(&container_id)
            .expect("container shell task just inserted");
        ContainerShellOpen {
            input_tx,
            output_rx: task.output_rx.take(),
            status_rx,
            resize_tx,
        }
    }

    fn ensure_container_shell_status(
        self: &Arc<Self>,
        container_id: String,
    ) -> watch::Receiver<ContainerShellSnapshot> {
        let mut tasks = self
            .shell_tasks
            .lock()
            .expect("container shell task lock poisoned");

        if let Some(current) = tasks.get(&container_id)
            && !current.handle.is_finished()
        {
            return current.tx.subscribe();
        }

        if let Some(current) = tasks.remove(&container_id) {
            current.handle.abort();
        }

        let is_running = self
            .container_is_running_from_snapshot(&container_id)
            .unwrap_or(true);
        let (input_tx, input_rx) = mpsc::channel(SHELL_WRITE_BUFFER_SIZE);
        let (output_tx, output_rx) = mpsc::channel(SHELL_OUTPUT_BUFFER_SIZE);
        let (resize_tx, resize_rx) = mpsc::channel(SHELL_WRITE_BUFFER_SIZE);
        let (tx, rx) = watch::channel(ContainerShellSnapshot::loading(container_id.clone()));
        let exec_id = Arc::new(Mutex::new(None));
        let handle = self.spawn_container_shell_handle(
            container_id.clone(),
            is_running,
            ContainerShellIo {
                status_tx: tx.clone(),
                input_rx,
                output_tx,
                resize_rx,
                exec_id_slot: exec_id.clone(),
            },
        );

        tasks.insert(
            container_id,
            ContainerShellTask {
                is_running,
                exec_id,
                input_tx,
                output_rx: Some(output_rx),
                resize_tx,
                tx,
                handle,
            },
        );

        rx
    }

    fn spawn_container_shell_handle(
        self: &Arc<Self>,
        container_id: String,
        is_running: bool,
        io: ContainerShellIo,
    ) -> JoinHandle<()> {
        let session = self.clone();
        self.runtime.spawn(async move {
            session
                .run_container_shell(container_id, is_running, io)
                .await;
        })
    }

    pub async fn refresh(self: &Arc<Self>) -> Result<ContainerSnapshot> {
        self.refresh_with_status(ConnectionStatus::Live).await
    }

    pub async fn refresh_images(self: &Arc<Self>) -> Result<ImageSnapshot> {
        self.refresh_images_with_send().await
    }

    pub async fn refresh_volumes(self: &Arc<Self>) -> Result<VolumeSnapshot> {
        self.refresh_volumes_with_send().await
    }

    pub async fn refresh_networks(self: &Arc<Self>) -> Result<NetworkSnapshot> {
        self.refresh_networks_with_send().await
    }

    pub async fn control_container(
        &self,
        container_id: String,
        action: ContainerAction,
    ) -> Result<()> {
        local::control_container(self.target.clone(), &container_id, action).await
    }

    async fn run_network_throughput(
        self: Arc<Self>,
        target: NetworkThroughputTarget,
        tx: watch::Sender<NetworkThroughputSnapshot>,
    ) {
        let container_ids = target.container_ids();
        let mut snapshot = NetworkThroughputSnapshot::loading(target.clone());
        let _ = tx.send(snapshot.clone());

        if target.is_idle() {
            snapshot.latest = Some(NetworkThroughputStats::zero(SystemTime::now()));
            snapshot.status = NetworkThroughputStatus::Idle;
            snapshot.last_updated = Some(SystemTime::now());
            let _ = tx.send(snapshot);
            return;
        }

        let (update_tx, mut update_rx) = mpsc::channel(container_ids.len().max(1) * 4);
        let mut source_handles = Vec::with_capacity(container_ids.len());
        for container_id in container_ids {
            let session = self.clone();
            let update_tx = update_tx.clone();
            source_handles.push(self.runtime.spawn(async move {
                session
                    .run_network_throughput_source(container_id, update_tx)
                    .await;
            }));
        }
        drop(update_tx);

        let mut latest_by_container = HashMap::new();
        let mut errors = HashMap::new();
        let mut history = VecDeque::new();
        let mut next_sequence = 0;
        let mut interval = tokio::time::interval(NETWORK_THROUGHPUT_INTERVAL);

        loop {
            tokio::select! {
                update = update_rx.recv() => {
                    match update {
                        Some(NetworkThroughputUpdate::Stats(container_id, stats)) => {
                            errors.remove(&container_id);
                            latest_by_container.insert(container_id, stats);
                        }
                        Some(NetworkThroughputUpdate::Error(container_id, error)) => {
                            latest_by_container.remove(&container_id);
                            errors.insert(container_id, error);
                        }
                        None => {
                            break;
                        }
                    }
                }
                _ = interval.tick() => {
                    if tx.receiver_count() == 0 {
                        break;
                    }

                    let now = SystemTime::now();
                    if latest_by_container.is_empty() {
                        snapshot.latest = None;
                        snapshot.status = if errors.is_empty() {
                            NetworkThroughputStatus::Loading
                        } else {
                            NetworkThroughputStatus::Reconnecting
                        };
                        snapshot.error = network_throughput_error_summary(&errors);
                        snapshot.last_updated = Some(now);
                        let _ = tx.send(snapshot.clone());
                        continue;
                    }

                    let stats = aggregate_network_throughput(latest_by_container.values(), now);
                    push_network_throughput_history(&mut history, &stats, next_sequence);
                    next_sequence += 1;
                    snapshot.latest = Some(stats);
                    snapshot.history = history.iter().cloned().collect();
                    snapshot.status = if errors.is_empty() {
                        NetworkThroughputStatus::Live
                    } else {
                        NetworkThroughputStatus::Reconnecting
                    };
                    snapshot.error = network_throughput_error_summary(&errors);
                    snapshot.last_updated = Some(now);
                    let _ = tx.send(snapshot.clone());
                }
            }
        }

        for handle in source_handles {
            handle.abort();
        }
    }

    async fn run_network_throughput_source(
        self: Arc<Self>,
        container_id: String,
        update_tx: mpsc::Sender<NetworkThroughputUpdate>,
    ) {
        while !update_tx.is_closed() {
            let mut samples =
                match local::stream_container_stats(self.target.clone(), &container_id) {
                    Ok(samples) => samples,
                    Err(error) => {
                        let _ = update_tx
                            .send(NetworkThroughputUpdate::Error(
                                container_id.clone(),
                                error.to_string(),
                            ))
                            .await;
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };

            loop {
                match samples.next().await {
                    Some(Ok(stats)) => {
                        if update_tx
                            .send(NetworkThroughputUpdate::Stats(container_id.clone(), stats))
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                    Some(Err(error)) => {
                        let _ = update_tx
                            .send(NetworkThroughputUpdate::Error(
                                container_id.clone(),
                                error.to_string(),
                            ))
                            .await;
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        break;
                    }
                    None => {
                        let _ = update_tx
                            .send(NetworkThroughputUpdate::Error(
                                container_id.clone(),
                                "Docker stats stream ended".to_string(),
                            ))
                            .await;
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        break;
                    }
                }
            }
        }
    }

    async fn run_container_detail(
        self: Arc<Self>,
        container_id: String,
        is_running: bool,
        tx: watch::Sender<ContainerDetailSnapshot>,
    ) {
        let mut history = VecDeque::new();
        let mut next_sequence = 0;
        let detail = match local::inspect_container(self.target.clone(), &container_id).await {
            Ok(detail) => detail,
            Err(error) => {
                let _ = tx.send(ContainerDetailSnapshot {
                    container_id,
                    detail: None,
                    latest: None,
                    history: Vec::new(),
                    status: ContainerDetailStatus::Error,
                    error: Some(error.to_string()),
                    last_updated: Some(SystemTime::now()),
                });
                return;
            }
        };

        let mut snapshot = ContainerDetailSnapshot {
            container_id: container_id.clone(),
            detail: Some(detail),
            latest: None,
            history: Vec::new(),
            status: ContainerDetailStatus::Loading,
            error: None,
            last_updated: Some(SystemTime::now()),
        };
        let _ = tx.send(snapshot.clone());

        if !is_running {
            snapshot.status = ContainerDetailStatus::Stopped;
            snapshot.last_updated = Some(SystemTime::now());
            let _ = tx.send(snapshot);
            return;
        }

        let mut samples = match local::stream_container_stats(self.target.clone(), &container_id) {
            Ok(samples) => samples,
            Err(error) => {
                snapshot.status = ContainerDetailStatus::Error;
                snapshot.error = Some(error.to_string());
                snapshot.last_updated = Some(SystemTime::now());
                let _ = tx.send(snapshot);
                return;
            }
        };

        loop {
            match samples.next().await {
                Some(Ok(stats)) => {
                    history.push_back(ContainerMetricPoint::from_stats(&stats, next_sequence));
                    next_sequence += 1;
                    if history.len() > DETAIL_HISTORY_LIMIT {
                        history.pop_front();
                    }

                    snapshot.latest = Some(stats);
                    snapshot.history = history.iter().cloned().collect();
                    snapshot.status = ContainerDetailStatus::Live;
                    snapshot.error = None;
                    snapshot.last_updated = Some(SystemTime::now());
                    let _ = tx.send(snapshot.clone());
                }
                Some(Err(error)) => {
                    snapshot.status = ContainerDetailStatus::Reconnecting;
                    snapshot.error = Some(error.to_string());
                    snapshot.last_updated = Some(SystemTime::now());
                    let _ = tx.send(snapshot.clone());
                    tokio::time::sleep(Duration::from_secs(1)).await;

                    match local::stream_container_stats(self.target.clone(), &container_id) {
                        Ok(new_samples) => {
                            samples = new_samples;
                        }
                        Err(error) => {
                            snapshot.status = ContainerDetailStatus::Error;
                            snapshot.error = Some(error.to_string());
                            snapshot.last_updated = Some(SystemTime::now());
                            let _ = tx.send(snapshot.clone());
                            break;
                        }
                    }
                }
                None => {
                    snapshot.status = ContainerDetailStatus::Stopped;
                    snapshot.last_updated = Some(SystemTime::now());
                    let _ = tx.send(snapshot);
                    break;
                }
            }
        }
    }

    async fn run_container_logs(
        self: Arc<Self>,
        container_id: String,
        tx: watch::Sender<ContainerLogsSnapshot>,
    ) {
        let mut snapshot = ContainerLogsSnapshot::loading(container_id.clone());
        let mut lines = Vec::new();
        let mut pending = Vec::new();
        let mut last_flush = Instant::now();

        let mut stream = match local::stream_container_logs(self.target.clone(), &container_id) {
            Ok(stream) => stream,
            Err(error) => {
                snapshot.status = ContainerLogsStatus::Error;
                snapshot.error = Some(error.to_string());
                snapshot.last_updated = Some(SystemTime::now());
                let _ = tx.send(snapshot);
                return;
            }
        };

        snapshot.status = ContainerLogsStatus::Live;
        snapshot.last_updated = Some(SystemTime::now());
        let _ = tx.send(snapshot.clone());

        loop {
            if tx.receiver_count() == 0 {
                break;
            }

            let next_line = if pending.is_empty() {
                stream.next().await
            } else {
                tokio::time::timeout(LOG_BATCH_INTERVAL, stream.next())
                    .await
                    .ok()
                    .flatten()
            };

            match next_line {
                Some(Ok(line)) => {
                    pending.push(line);
                    if pending.len() >= LOG_BATCH_LIMIT
                        || last_flush.elapsed() >= LOG_BATCH_INTERVAL
                    {
                        flush_log_lines(&mut lines, &mut pending, &mut snapshot, &tx);
                        last_flush = Instant::now();
                    }
                }
                Some(Err(error)) => {
                    if !pending.is_empty() {
                        flush_log_lines(&mut lines, &mut pending, &mut snapshot, &tx);
                        last_flush = Instant::now();
                    }
                    snapshot.status = ContainerLogsStatus::Reconnecting;
                    snapshot.error = Some(error.to_string());
                    snapshot.last_updated = Some(SystemTime::now());
                    let _ = tx.send(snapshot.clone());
                    tokio::time::sleep(Duration::from_secs(1)).await;

                    match local::stream_container_logs(self.target.clone(), &container_id) {
                        Ok(new_stream) => {
                            stream = new_stream;
                        }
                        Err(error) => {
                            snapshot.status = ContainerLogsStatus::Error;
                            snapshot.error = Some(error.to_string());
                            snapshot.last_updated = Some(SystemTime::now());
                            let _ = tx.send(snapshot.clone());
                            break;
                        }
                    }
                }
                None if pending.is_empty() => {
                    snapshot.status = ContainerLogsStatus::Stopped;
                    snapshot.last_updated = Some(SystemTime::now());
                    let _ = tx.send(snapshot);
                    break;
                }
                None => {
                    if !pending.is_empty() {
                        flush_log_lines(&mut lines, &mut pending, &mut snapshot, &tx);
                        last_flush = Instant::now();
                    }
                }
            }
        }
    }

    async fn run_container_shell(
        self: Arc<Self>,
        container_id: String,
        is_running: bool,
        io: ContainerShellIo,
    ) {
        let ContainerShellIo {
            status_tx: tx,
            mut input_rx,
            output_tx,
            mut resize_rx,
            exec_id_slot,
        } = io;
        let mut snapshot = ContainerShellSnapshot::loading(container_id.clone());

        if !is_running {
            snapshot.status = ContainerShellStatus::Stopped;
            snapshot.last_updated = Some(SystemTime::now());
            let _ = tx.send(snapshot);
            return;
        }

        let exec = match local::start_container_shell(
            self.target.clone(),
            &container_id,
            DEFAULT_SHELL_COLS,
            DEFAULT_SHELL_ROWS,
        )
        .await
        {
            Ok(exec) => exec,
            Err(error) => {
                snapshot.status = ContainerShellStatus::Error;
                snapshot.error = Some(error.to_string());
                snapshot.last_updated = Some(SystemTime::now());
                let _ = tx.send(snapshot);
                return;
            }
        };

        {
            let mut exec_id = exec_id_slot
                .lock()
                .expect("container shell exec id lock poisoned");
            *exec_id = Some(exec.exec_id.clone());
        }

        snapshot.status = ContainerShellStatus::Live;
        snapshot.last_updated = Some(SystemTime::now());
        let _ = tx.send(snapshot.clone());

        let mut output = exec.output;
        let mut input = exec.input;
        let exec_id = exec.exec_id.clone();

        loop {
            tokio::select! {
                chunk = output.next() => {
                    match chunk {
                        Some(Ok(output)) => {
                            let bytes = output.as_ref().to_vec();
                            snapshot.status = ContainerShellStatus::Live;
                            snapshot.error = None;
                            snapshot.last_updated = Some(SystemTime::now());
                            if tx.send(snapshot.clone()).is_err() {
                                break;
                            }
                            if output_tx.send(bytes).await.is_err() {
                                break;
                            }
                        }
                        Some(Err(error)) => {
                            snapshot.status = ContainerShellStatus::Error;
                            snapshot.error = Some(error.to_string());
                            snapshot.last_updated = Some(SystemTime::now());
                            let _ = tx.send(snapshot);
                            break;
                        }
                        None => {
                            snapshot.status = ContainerShellStatus::Exited;
                            snapshot.last_updated = Some(SystemTime::now());
                            let _ = tx.send(snapshot);
                            break;
                        }
                    }
                }
                input_bytes = input_rx.recv() => {
                    let Some(input_bytes) = input_bytes else {
                        break;
                    };
                    if input.write_all(&input_bytes).await.is_err() {
                        snapshot.status = ContainerShellStatus::Exited;
                        snapshot.last_updated = Some(SystemTime::now());
                        let _ = tx.send(snapshot);
                        break;
                    }
                    let _ = input.flush().await;
                }
                resize = resize_rx.recv() => {
                    let Some((cols, rows)) = resize else {
                        break;
                    };
                    if let Err(error) = local::resize_container_shell(
                        self.target.clone(),
                        &exec_id,
                        cols,
                        rows,
                    )
                    .await
                    {
                        snapshot.status = ContainerShellStatus::Error;
                        snapshot.error = Some(error.to_string());
                        snapshot.last_updated = Some(SystemTime::now());
                        let _ = tx.send(snapshot);
                        break;
                    }
                }
            }
        }
    }

    async fn run(self: Arc<Self>) {
        let _ = self.refresh_with_status(ConnectionStatus::Connecting).await;

        let mut reconnect_delay = Duration::from_secs(1);

        loop {
            let (signal_tx, mut signal_rx) = mpsc::channel(EVENT_BUFFER_SIZE);
            let target = self.target.clone();
            let watcher = tokio::spawn(async move {
                sync::watch_container_events(target, POLL_INTERVAL, signal_tx).await
            });

            while let Some(signal) = signal_rx.recv().await {
                reconnect_delay = Duration::from_secs(1);
                match signal {
                    ContainerSyncSignal::Changed(kind) => {
                        if !should_refresh_for_event_kind(kind) {
                            continue;
                        }
                        self.send_status(ConnectionStatus::Live, None, None);
                        if kind != ContainerEventKind::Removed {
                            tokio::time::sleep(EVENT_REFRESH_DEBOUNCE).await;
                        }
                        drain_changed_signals(&mut signal_rx);
                        let _ = self.refresh_with_status(ConnectionStatus::Live).await;
                    }
                    ContainerSyncSignal::Poll => {
                        self.send_status(ConnectionStatus::Polling, None, None);
                        let _ = self.refresh_with_status(ConnectionStatus::Live).await;
                    }
                }
            }

            let error = match watcher.await {
                Ok(Err(error)) => error.to_string(),
                Ok(Ok(())) => "Docker event stream ended".to_string(),
                Err(error) => error.to_string(),
            };

            self.send_status(
                ConnectionStatus::Reconnecting,
                Some(error),
                Some(reconnect_delay),
            );
            tokio::time::sleep(reconnect_delay).await;
            reconnect_delay = (reconnect_delay * 2).min(MAX_RECONNECT_DELAY);
        }
    }

    async fn run_images(self: Arc<Self>) {
        let _ = self.refresh_images_with_send().await;

        let mut reconnect_delay = Duration::from_secs(1);

        loop {
            let (signal_tx, mut signal_rx) = mpsc::channel(EVENT_BUFFER_SIZE);
            let target = self.target.clone();
            let watcher = tokio::spawn(async move {
                sync::watch_image_events(target, POLL_INTERVAL, signal_tx).await
            });

            while let Some(signal) = signal_rx.recv().await {
                reconnect_delay = Duration::from_secs(1);
                match signal {
                    ResourceSyncSignal::Changed => {
                        tokio::time::sleep(EVENT_REFRESH_DEBOUNCE).await;
                        drain_resource_signals(&mut signal_rx);
                        let _ = self.refresh_images_with_send().await;
                    }
                    ResourceSyncSignal::Poll => {
                        let _ = self.refresh_images_with_send().await;
                    }
                }
            }

            let error = match watcher.await {
                Ok(Err(error)) => error.to_string(),
                Ok(Ok(())) => "Docker image event stream ended".to_string(),
                Err(error) => error.to_string(),
            };

            self.send_image_error(error);
            tokio::time::sleep(reconnect_delay).await;
            reconnect_delay = (reconnect_delay * 2).min(MAX_RECONNECT_DELAY);
        }
    }

    async fn run_volumes(self: Arc<Self>) {
        let _ = self.refresh_volumes_with_send().await;

        let mut reconnect_delay = Duration::from_secs(1);

        loop {
            let (signal_tx, mut signal_rx) = mpsc::channel(EVENT_BUFFER_SIZE);
            let target = self.target.clone();
            let watcher = tokio::spawn(async move {
                sync::watch_volume_events(target, POLL_INTERVAL, signal_tx).await
            });

            while let Some(signal) = signal_rx.recv().await {
                reconnect_delay = Duration::from_secs(1);
                match signal {
                    ResourceSyncSignal::Changed => {
                        tokio::time::sleep(EVENT_REFRESH_DEBOUNCE).await;
                        drain_resource_signals(&mut signal_rx);
                        let _ = self.refresh_volumes_with_send().await;
                    }
                    ResourceSyncSignal::Poll => {
                        let _ = self.refresh_volumes_with_send().await;
                    }
                }
            }

            let error = match watcher.await {
                Ok(Err(error)) => error.to_string(),
                Ok(Ok(())) => "Docker volume event stream ended".to_string(),
                Err(error) => error.to_string(),
            };

            self.send_volume_error(error);
            tokio::time::sleep(reconnect_delay).await;
            reconnect_delay = (reconnect_delay * 2).min(MAX_RECONNECT_DELAY);
        }
    }

    async fn run_networks(self: Arc<Self>) {
        let _ = self.refresh_networks_with_send().await;

        let mut reconnect_delay = Duration::from_secs(1);

        loop {
            let (signal_tx, mut signal_rx) = mpsc::channel(EVENT_BUFFER_SIZE);
            let target = self.target.clone();
            let watcher = tokio::spawn(async move {
                sync::watch_network_events(target, POLL_INTERVAL, signal_tx).await
            });

            while let Some(signal) = signal_rx.recv().await {
                reconnect_delay = Duration::from_secs(1);
                match signal {
                    NetworkSyncSignal::Changed => {
                        tokio::time::sleep(EVENT_REFRESH_DEBOUNCE).await;
                        drain_network_signals(&mut signal_rx);
                        let _ = self.refresh_networks_with_send().await;
                    }
                    NetworkSyncSignal::Poll => {
                        let _ = self.refresh_networks_with_send().await;
                    }
                }
            }

            let error = match watcher.await {
                Ok(Err(error)) => error.to_string(),
                Ok(Ok(())) => "Docker network event stream ended".to_string(),
                Err(error) => error.to_string(),
            };

            self.send_network_error(error);
            tokio::time::sleep(reconnect_delay).await;
            reconnect_delay = (reconnect_delay * 2).min(MAX_RECONNECT_DELAY);
        }
    }

    async fn refresh_with_status(
        self: &Arc<Self>,
        status: ConnectionStatus,
    ) -> Result<ContainerSnapshot> {
        match local::list_containers(self.target.clone()).await {
            Ok(containers) => {
                self.reconcile_container_detail_tasks(&containers);
                self.reconcile_container_logs_task(&containers);
                self.reconcile_container_shell_tasks(&containers);
                let snapshot = ContainerSnapshot {
                    containers,
                    status,
                    error: None,
                    retry_after: None,
                    last_updated: Some(SystemTime::now()),
                };
                let _ = self.snapshot_tx.send(snapshot.clone());
                Ok(snapshot)
            }
            Err(error) => {
                let snapshot = self.error_snapshot(
                    if self.snapshot_tx.borrow().containers.is_empty() {
                        ConnectionStatus::Error
                    } else {
                        ConnectionStatus::Reconnecting
                    },
                    error.to_string(),
                    Some(Duration::from_secs(1)),
                );
                let _ = self.snapshot_tx.send(snapshot.clone());
                Err(error)
            }
        }
    }

    async fn refresh_images_with_send(self: &Arc<Self>) -> Result<ImageSnapshot> {
        match local::list_images(self.target.clone()).await {
            Ok(images) => {
                let snapshot = ImageSnapshot {
                    images,
                    error: None,
                    last_updated: Some(SystemTime::now()),
                };
                let _ = self.image_snapshot_tx.send(snapshot.clone());
                Ok(snapshot)
            }
            Err(error) => {
                let snapshot = self.image_error_snapshot(error.to_string());
                let _ = self.image_snapshot_tx.send(snapshot);
                Err(error)
            }
        }
    }

    async fn refresh_volumes_with_send(self: &Arc<Self>) -> Result<VolumeSnapshot> {
        match local::list_volumes(self.target.clone()).await {
            Ok(volumes) => {
                let snapshot = VolumeSnapshot {
                    volumes,
                    error: None,
                    last_updated: Some(SystemTime::now()),
                };
                let _ = self.volume_snapshot_tx.send(snapshot.clone());
                Ok(snapshot)
            }
            Err(error) => {
                let snapshot = self.volume_error_snapshot(error.to_string());
                let _ = self.volume_snapshot_tx.send(snapshot);
                Err(error)
            }
        }
    }

    async fn refresh_networks_with_send(self: &Arc<Self>) -> Result<NetworkSnapshot> {
        match local::list_networks(self.target.clone()).await {
            Ok(networks) => {
                let snapshot = NetworkSnapshot {
                    networks,
                    error: None,
                    last_updated: Some(SystemTime::now()),
                };
                let _ = self.network_snapshot_tx.send(snapshot.clone());
                Ok(snapshot)
            }
            Err(error) => {
                let snapshot = self.network_error_snapshot(error.to_string());
                let _ = self.network_snapshot_tx.send(snapshot);
                Err(error)
            }
        }
    }

    fn send_image_error(&self, error: String) {
        let snapshot = self.image_error_snapshot(error);
        let _ = self.image_snapshot_tx.send(snapshot);
    }

    fn image_error_snapshot(&self, error: String) -> ImageSnapshot {
        ImageSnapshot {
            images: self.current_images(),
            error: Some(error),
            last_updated: Some(SystemTime::now()),
        }
    }

    fn current_images(&self) -> Vec<ImageSummary> {
        self.image_snapshot_tx.borrow().images.clone()
    }

    fn send_volume_error(&self, error: String) {
        let snapshot = self.volume_error_snapshot(error);
        let _ = self.volume_snapshot_tx.send(snapshot);
    }

    fn volume_error_snapshot(&self, error: String) -> VolumeSnapshot {
        VolumeSnapshot {
            volumes: self.current_volumes(),
            error: Some(error),
            last_updated: Some(SystemTime::now()),
        }
    }

    fn current_volumes(&self) -> Vec<VolumeSummary> {
        self.volume_snapshot_tx.borrow().volumes.clone()
    }

    fn send_network_error(&self, error: String) {
        let snapshot = self.network_error_snapshot(error);
        let _ = self.network_snapshot_tx.send(snapshot);
    }

    fn network_error_snapshot(&self, error: String) -> NetworkSnapshot {
        NetworkSnapshot {
            networks: self.current_networks(),
            error: Some(error),
            last_updated: Some(SystemTime::now()),
        }
    }

    fn current_networks(&self) -> Vec<NetworkSummary> {
        self.network_snapshot_tx.borrow().networks.clone()
    }

    fn reconcile_container_detail_tasks(self: &Arc<Self>, containers: &[ContainerSummary]) {
        let mut tasks = self
            .detail_tasks
            .lock()
            .expect("container detail task lock poisoned");
        let expected = expected_detail_tasks(containers);

        tasks.retain(|container_id, task| {
            if task.tx.receiver_count() == 0 {
                task.handle.abort();
                return false;
            }

            let Some(is_running) = expected.get(container_id).copied() else {
                task.handle.abort();
                return false;
            };

            if should_restart_detail_task(task.is_running, is_running, task.handle.is_finished()) {
                task.handle.abort();
                task.is_running = is_running;
                task.handle = self.spawn_container_detail_handle(
                    container_id.clone(),
                    is_running,
                    task.tx.clone(),
                );
            }

            true
        });
    }

    fn reconcile_container_logs_task(self: &Arc<Self>, containers: &[ContainerSummary]) {
        let mut task = self
            .logs_task
            .lock()
            .expect("container logs task lock poisoned");
        let Some(current) = task.as_mut() else {
            return;
        };

        if current.tx.receiver_count() == 0 {
            if let Some(current) = task.take() {
                current.handle.abort();
            }
            return;
        }

        let Some(is_running) = containers
            .iter()
            .find(|container| container.id == current.container_id)
            .map(container_is_running)
        else {
            if let Some(current) = task.take() {
                current.handle.abort();
            }
            return;
        };

        if should_restart_stream_task(current.is_running, is_running, current.handle.is_finished())
        {
            current.handle.abort();
            current.is_running = is_running;
            current.handle =
                self.spawn_container_logs_handle(current.container_id.clone(), current.tx.clone());
        }
    }

    fn reconcile_container_shell_tasks(self: &Arc<Self>, containers: &[ContainerSummary]) {
        let mut tasks = self
            .shell_tasks
            .lock()
            .expect("container shell task lock poisoned");

        tasks.retain(|container_id, task| {
            if task.tx.receiver_count() == 0 {
                task.handle.abort();
                return false;
            }

            let Some(is_running) = containers
                .iter()
                .find(|container| container.id == *container_id)
                .map(container_is_running)
            else {
                task.handle.abort();
                return false;
            };

            if should_restart_stream_task(task.is_running, is_running, task.handle.is_finished()) {
                task.handle.abort();
                task.is_running = is_running;
                let (input_tx, input_rx) = mpsc::channel(SHELL_WRITE_BUFFER_SIZE);
                let (output_tx, output_rx) = mpsc::channel(SHELL_OUTPUT_BUFFER_SIZE);
                let (resize_tx, resize_rx) = mpsc::channel(SHELL_WRITE_BUFFER_SIZE);
                task.input_tx = input_tx;
                task.output_rx = Some(output_rx);
                task.resize_tx = resize_tx;
                *task
                    .exec_id
                    .lock()
                    .expect("container shell exec id lock poisoned") = None;
                task.handle = self.spawn_container_shell_handle(
                    container_id.clone(),
                    is_running,
                    ContainerShellIo {
                        status_tx: task.tx.clone(),
                        input_rx,
                        output_tx,
                        resize_rx,
                        exec_id_slot: task.exec_id.clone(),
                    },
                );
            }

            true
        });
    }

    fn send_status(
        &self,
        status: ConnectionStatus,
        error: Option<String>,
        retry_after: Option<Duration>,
    ) {
        let current = self.snapshot_tx.borrow().clone();
        let snapshot = ContainerSnapshot {
            containers: current.containers,
            status,
            error,
            retry_after,
            last_updated: current.last_updated,
        };
        let _ = self.snapshot_tx.send(snapshot);
    }

    fn error_snapshot(
        &self,
        status: ConnectionStatus,
        error: String,
        retry_after: Option<Duration>,
    ) -> ContainerSnapshot {
        let current = self.snapshot_tx.borrow().clone();
        ContainerSnapshot {
            containers: current.containers,
            status,
            error: Some(error),
            retry_after,
            last_updated: current.last_updated,
        }
    }

    fn container_is_running_from_snapshot(&self, container_id: &str) -> Option<bool> {
        self.snapshot_tx
            .borrow()
            .containers
            .iter()
            .find(|container| container.id == container_id)
            .map(container_is_running)
    }
}

fn flush_log_lines(
    lines: &mut Vec<ContainerLogLine>,
    pending: &mut Vec<ContainerLogLine>,
    snapshot: &mut ContainerLogsSnapshot,
    tx: &watch::Sender<ContainerLogsSnapshot>,
) {
    for line in pending.drain(..) {
        push_log_line(lines, line);
    }
    snapshot.lines = Arc::new(lines.clone());
    snapshot.status = ContainerLogsStatus::Live;
    snapshot.error = None;
    snapshot.last_updated = Some(SystemTime::now());
    let _ = tx.send(snapshot.clone());
}

fn push_log_line(lines: &mut Vec<ContainerLogLine>, line: ContainerLogLine) {
    lines.push(line);
    if lines.len() > LOG_LINE_LIMIT {
        let overflow = lines.len() - LOG_LINE_LIMIT;
        lines.drain(0..overflow);
    }
}

fn drain_changed_signals(receiver: &mut mpsc::Receiver<ContainerSyncSignal>) {
    while matches!(
        receiver.try_recv(),
        Ok(ContainerSyncSignal::Changed(_)) | Ok(ContainerSyncSignal::Poll)
    ) {}
}

fn drain_network_signals(receiver: &mut mpsc::Receiver<NetworkSyncSignal>) {
    while matches!(
        receiver.try_recv(),
        Ok(NetworkSyncSignal::Changed) | Ok(NetworkSyncSignal::Poll)
    ) {}
}

fn drain_resource_signals(receiver: &mut mpsc::Receiver<ResourceSyncSignal>) {
    while matches!(
        receiver.try_recv(),
        Ok(ResourceSyncSignal::Changed) | Ok(ResourceSyncSignal::Poll)
    ) {}
}

fn should_refresh_for_event_kind(kind: ContainerEventKind) -> bool {
    matches!(
        kind,
        ContainerEventKind::StateChanged | ContainerEventKind::Removed
    )
}

fn should_restart_detail_task(
    previous_is_running: bool,
    current_is_running: bool,
    handle_is_finished: bool,
) -> bool {
    previous_is_running != current_is_running || (current_is_running && handle_is_finished)
}

fn should_restart_stream_task(
    previous_is_running: bool,
    current_is_running: bool,
    handle_is_finished: bool,
) -> bool {
    previous_is_running != current_is_running || (current_is_running && handle_is_finished)
}

fn aggregate_network_throughput<'a>(
    stats: impl IntoIterator<Item = &'a ContainerRuntimeStats>,
    sample_time: SystemTime,
) -> NetworkThroughputStats {
    let (rx_bytes_per_sec, tx_bytes_per_sec) =
        stats.into_iter().fold((0., 0.), |(rx, tx), stats| {
            (
                rx + stats.network_rx_bytes_per_sec,
                tx + stats.network_tx_bytes_per_sec,
            )
        });

    NetworkThroughputStats {
        sample_time,
        rx_bytes_per_sec,
        tx_bytes_per_sec,
    }
}

fn push_network_throughput_history(
    history: &mut VecDeque<NetworkThroughputPoint>,
    stats: &NetworkThroughputStats,
    sequence: u64,
) {
    history.push_back(NetworkThroughputPoint::from_stats(stats, sequence));
    if history.len() > DETAIL_HISTORY_LIMIT {
        history.pop_front();
    }
}

fn network_throughput_error_summary(errors: &HashMap<String, String>) -> Option<String> {
    if errors.is_empty() {
        return None;
    }

    let mut errors = errors
        .iter()
        .map(|(container_id, error)| format!("{}: {}", short_id(container_id), error))
        .collect::<Vec<_>>();
    errors.sort();
    Some(errors.join("; "))
}

fn container_is_running(container: &ContainerSummary) -> bool {
    container
        .state
        .as_deref()
        .is_some_and(|state| state.eq_ignore_ascii_case("running"))
}

fn short_id(id: &str) -> String {
    id.chars().take(12).collect()
}

fn expected_detail_tasks(containers: &[ContainerSummary]) -> HashMap<String, bool> {
    containers
        .iter()
        .map(|container| (container.id.clone(), container_is_running(container)))
        .collect()
}

#[allow(dead_code)]
fn _assert_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ConnectionSession>();
    assert_send_sync::<ContainerSummary>();
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, time::UNIX_EPOCH};

    use tokio::sync::mpsc;

    use crate::{
        bridge::types::{ContainerEventKind, ContainerSyncSignal, ResourceSyncSignal},
        domain::{
            ContainerLogLine, ContainerLogStreamKind, ContainerPortSummary, ContainerRuntimeStats,
            ContainerSummary, NetworkThroughputStats,
        },
    };

    use super::{
        DETAIL_HISTORY_LIMIT, LOG_LINE_LIMIT, aggregate_network_throughput, container_is_running,
        drain_changed_signals, drain_resource_signals, expected_detail_tasks, push_log_line,
        push_network_throughput_history, should_refresh_for_event_kind, should_restart_detail_task,
        should_restart_stream_task,
    };

    #[tokio::test]
    async fn drains_queued_sync_signals() {
        let (tx, mut rx) = mpsc::channel(4);
        tx.send(ContainerSyncSignal::Changed(
            ContainerEventKind::StateChanged,
        ))
        .await
        .unwrap();
        tx.send(ContainerSyncSignal::Poll).await.unwrap();

        drain_changed_signals(&mut rx);

        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn drains_queued_resource_sync_signals() {
        let (tx, mut rx) = mpsc::channel(4);
        tx.send(ResourceSyncSignal::Changed).await.unwrap();
        tx.send(ResourceSyncSignal::Poll).await.unwrap();

        drain_resource_signals(&mut rx);

        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn trims_log_lines_to_buffer_limit() {
        let mut lines = Vec::new();
        for index in 0..(LOG_LINE_LIMIT + 3) {
            push_log_line(
                &mut lines,
                ContainerLogLine::new(
                    None,
                    ContainerLogStreamKind::Stdout,
                    format!("line {}", index),
                ),
            );
        }

        assert_eq!(lines.len(), LOG_LINE_LIMIT);
        assert_eq!(lines.first().unwrap().message, "line 3");
    }

    #[test]
    fn aggregates_network_throughput_from_container_stats() {
        let stats = vec![runtime_stats(1_024., 2_048.), runtime_stats(256., 512.)];

        let aggregate = aggregate_network_throughput(stats.iter(), UNIX_EPOCH);

        assert_eq!(aggregate.sample_time, UNIX_EPOCH);
        assert_eq!(aggregate.rx_bytes_per_sec, 1_280.);
        assert_eq!(aggregate.tx_bytes_per_sec, 2_560.);
    }

    #[test]
    fn trims_network_throughput_history_to_detail_limit() {
        let mut history = VecDeque::new();
        for index in 0..(DETAIL_HISTORY_LIMIT + 3) {
            let stats = NetworkThroughputStats {
                sample_time: UNIX_EPOCH,
                rx_bytes_per_sec: index as f64,
                tx_bytes_per_sec: 0.,
            };
            push_network_throughput_history(&mut history, &stats, index as u64);
        }

        assert_eq!(history.len(), DETAIL_HISTORY_LIMIT);
        assert_eq!(history.front().unwrap().sequence, 3);
        assert_eq!(
            history.back().unwrap().sequence,
            (DETAIL_HISTORY_LIMIT + 2) as u64
        );
        assert_eq!(history.front().unwrap().rx_bytes_per_sec, 3.);
    }

    #[test]
    fn detects_running_containers_from_summary_state() {
        assert!(container_is_running(&container_summary(
            "one",
            Some("running")
        )));
        assert!(container_is_running(&container_summary(
            "two",
            Some("RUNNING")
        )));
        assert!(!container_is_running(&container_summary(
            "three",
            Some("exited")
        )));
        assert!(!container_is_running(&container_summary("four", None)));
    }

    #[test]
    fn expected_detail_tasks_track_container_ids_and_runtime_state() {
        let containers = vec![
            container_summary("running-container", Some("running")),
            container_summary("stopped-container", Some("exited")),
        ];

        let expected = expected_detail_tasks(&containers);

        assert_eq!(expected.len(), 2);
        assert_eq!(expected.get("running-container"), Some(&true));
        assert_eq!(expected.get("stopped-container"), Some(&false));
    }

    #[test]
    fn ignores_other_container_events_for_refresh() {
        assert!(should_refresh_for_event_kind(
            ContainerEventKind::StateChanged
        ));
        assert!(should_refresh_for_event_kind(ContainerEventKind::Removed));
        assert!(!should_refresh_for_event_kind(ContainerEventKind::Other));
    }

    #[test]
    fn finished_stopped_detail_task_stays_stopped_until_state_changes() {
        assert!(!should_restart_detail_task(false, false, true));
        assert!(should_restart_detail_task(false, true, true));
        assert!(should_restart_detail_task(true, false, false));
        assert!(should_restart_detail_task(true, true, true));
        assert!(!should_restart_detail_task(true, true, false));
    }

    #[test]
    fn finished_stopped_log_task_stays_stopped_until_state_changes() {
        assert!(!should_restart_stream_task(false, false, true));
        assert!(should_restart_stream_task(false, true, true));
        assert!(should_restart_stream_task(true, false, false));
        assert!(should_restart_stream_task(true, true, true));
        assert!(!should_restart_stream_task(true, true, false));
    }

    fn container_summary(id: &str, state: Option<&str>) -> ContainerSummary {
        ContainerSummary::new(
            id.to_string(),
            id.to_string(),
            "image".to_string(),
            state.unwrap_or("unknown").to_string(),
            state.map(ToString::to_string),
            None,
            Vec::<ContainerPortSummary>::new(),
            false,
        )
    }

    fn runtime_stats(rx_bytes_per_sec: f64, tx_bytes_per_sec: f64) -> ContainerRuntimeStats {
        ContainerRuntimeStats {
            sample_time: UNIX_EPOCH,
            cpu_percent: 0.,
            online_cpus: None,
            memory_usage_bytes: None,
            memory_limit_bytes: None,
            network_rx_bytes_per_sec: rx_bytes_per_sec,
            network_tx_bytes_per_sec: tx_bytes_per_sec,
            disk_read_bytes_per_sec: 0.,
            disk_write_bytes_per_sec: 0.,
        }
    }
}

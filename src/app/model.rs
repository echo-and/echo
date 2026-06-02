use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use gpui::*;
use gpui_component::ThemeMode;

use crate::{
    app::{AppFontFamily, AppPreferences, UpdateStatus, clamp_container_list_width, updates},
    bridge::{
        Bridge, ConnectionStatus, ContainerAction, ContainerDetailSnapshot, ContainerDetailStatus,
        ContainerLogsSnapshot, ContainerLogsStatus, ContainerShellSession, ContainerShellSnapshot,
        ContainerShellStatus, ContainerSnapshot, ImageSnapshot, NetworkCreateConfig,
        NetworkSnapshot, NetworkThroughputSnapshot, NetworkThroughputStatus, VolumeSnapshot,
    },
    domain::{
        ActiveConnection, ContainerSummary, ImageSummary, NetworkSummary, NetworkThroughputTarget,
        VolumeSummary,
    },
    i18n::{self, AppLocale},
};

pub struct WorkspaceModel {
    bridge: Arc<Bridge>,
    pub active_connection: ActiveConnection,
    pub containers: Vec<ContainerSummary>,
    pub selected_container_id: Option<String>,
    pub search_text: String,
    pub active_nav: NavSection,
    pub error: Option<String>,
    pub refresh_error: Option<String>,
    pub is_loading: bool,
    pub sync_status: SyncStatus,
    pub reconnect_retry_at: Option<Instant>,
    pub reconnect_seconds_remaining: Option<u64>,
    pub last_updated: Option<SystemTime>,
    pub locale: AppLocale,
    pub theme_mode: ThemeMode,
    pub font_family: AppFontFamily,
    pub auto_check_updates: bool,
    pub notify_new_version: bool,
    pub update_status: UpdateStatus,
    pub container_list_width: u16,
    pub expanded_compose_projects: BTreeSet<String>,
    pub pending_container_action: Option<PendingContainerAction>,
    pub container_detail: Option<ContainerDetailSnapshot>,
    pub container_logs: Option<ContainerLogsSnapshot>,
    pub container_shell: Option<ContainerShellSnapshot>,
    pub container_detail_tab: ContainerDetailTab,
    pub container_bottom_maximized: bool,
    pub container_log_filter: String,
    pub container_logs_revision: u64,
    container_detail_tabs: HashMap<ContainerCacheKey, ContainerDetailTab>,
    pub images: Vec<ImageSummary>,
    pub image_search_text: String,
    pub image_error: Option<String>,
    pub is_images_loading: bool,
    pub is_image_importing: bool,
    pub pending_image_action: Option<PendingImageAction>,
    pub images_last_updated: Option<SystemTime>,
    pub volumes: Vec<VolumeSummary>,
    pub volume_search_text: String,
    pub volume_error: Option<String>,
    pub is_volumes_loading: bool,
    pub is_volume_importing: bool,
    pub pending_volume_action: Option<PendingVolumeAction>,
    pub volumes_last_updated: Option<SystemTime>,
    pub networks: Vec<NetworkSummary>,
    pub network_error: Option<String>,
    pub is_networks_loading: bool,
    pub pending_network_action: Option<PendingNetworkAction>,
    pub selected_network_node: Option<NetworkNodeSelection>,
    pub network_throughput: Option<NetworkThroughputSnapshot>,
    pub networks_last_updated: Option<SystemTime>,
    network_throughput_cache: HashMap<NetworkThroughputCacheKey, NetworkThroughputSnapshot>,
    container_detail_cache: ContainerDetailCache,
    container_logs_cache: ContainerLogsCache,
    _sync_task: Option<Task<()>>,
    _refresh_task: Option<Task<()>>,
    _container_action_task: Option<Task<()>>,
    _container_detail_task: Option<Task<()>>,
    _container_logs_task: Option<Task<()>>,
    _container_shell_task: Option<Task<()>>,
    _image_sync_task: Option<Task<()>>,
    _image_action_task: Option<Task<()>>,
    _volume_sync_task: Option<Task<()>>,
    _volume_action_task: Option<Task<()>>,
    _network_sync_task: Option<Task<()>>,
    _network_refresh_task: Option<Task<()>>,
    _network_action_task: Option<Task<()>>,
    _network_throughput_task: Option<Task<()>>,
    _connection_monitor_task: Option<Task<()>>,
    _connection_probe_task: Option<Task<()>>,
    _reconnect_countdown_task: Option<Task<()>>,
    _update_check_task: Option<Task<()>>,
}

const CONNECTION_MONITOR_INTERVAL: Duration = Duration::from_secs(5);
const DEFAULT_RECONNECT_RETRY_AFTER: Duration = Duration::from_secs(1);
const CONTAINER_DETAIL_CACHE_LIMIT: usize = 20;
const CONTAINER_LOGS_CACHE_LIMIT: usize = 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavSection {
    Containers,
    Images,
    Volumes,
    Networks,
    Settings,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncStatus {
    Loading,
    Live,
    Polling,
    Reconnecting,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingContainerAction {
    pub container_id: String,
    pub action: ContainerAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingImageAction {
    pub image_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingVolumeAction {
    pub volume_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingNetworkAction {
    pub network_name: String,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum NetworkNodeSelection {
    Network {
        network_id: String,
    },
    Container {
        network_id: String,
        container_id: String,
    },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct NetworkThroughputCacheKey {
    connection_id: String,
    selection: NetworkNodeSelection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerDetailTab {
    Logs,
    Shell,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ContainerCacheKey {
    connection_id: String,
    container_id: String,
}

#[derive(Clone, Debug, Default)]
struct ContainerDetailCache {
    entries: HashMap<ContainerCacheKey, ContainerDetailSnapshot>,
    lru: VecDeque<ContainerCacheKey>,
}

#[derive(Clone, Debug)]
struct CachedContainerLogs {
    snapshot: ContainerLogsSnapshot,
    revision: u64,
}

#[derive(Clone, Debug, Default)]
struct ContainerLogsCache {
    entries: HashMap<ContainerCacheKey, CachedContainerLogs>,
    lru: VecDeque<ContainerCacheKey>,
}

impl WorkspaceModel {
    pub fn new(bridge: Arc<Bridge>, preferences: AppPreferences, _cx: &mut Context<Self>) -> Self {
        i18n::set_locale(preferences.locale);

        let active_connection = bridge.resolve_active_connection();

        Self {
            bridge,
            active_connection,
            containers: Vec::new(),
            selected_container_id: None,
            search_text: String::new(),
            active_nav: NavSection::Containers,
            error: None,
            refresh_error: None,
            is_loading: true,
            sync_status: SyncStatus::Loading,
            reconnect_retry_at: None,
            reconnect_seconds_remaining: None,
            last_updated: None,
            locale: preferences.locale,
            theme_mode: preferences.theme_mode,
            font_family: preferences.font_family,
            auto_check_updates: preferences.auto_check_updates,
            notify_new_version: preferences.notify_new_version,
            update_status: UpdateStatus::NotChecked,
            container_list_width: preferences.container_list_width,
            expanded_compose_projects: BTreeSet::new(),
            pending_container_action: None,
            container_detail: None,
            container_logs: None,
            container_shell: None,
            container_detail_tab: ContainerDetailTab::Logs,
            container_bottom_maximized: false,
            container_log_filter: String::new(),
            container_logs_revision: 0,
            container_detail_tabs: HashMap::new(),
            images: Vec::new(),
            image_search_text: String::new(),
            image_error: None,
            is_images_loading: false,
            is_image_importing: false,
            pending_image_action: None,
            images_last_updated: None,
            volumes: Vec::new(),
            volume_search_text: String::new(),
            volume_error: None,
            is_volumes_loading: false,
            is_volume_importing: false,
            pending_volume_action: None,
            volumes_last_updated: None,
            networks: Vec::new(),
            network_error: None,
            is_networks_loading: false,
            pending_network_action: None,
            selected_network_node: None,
            network_throughput: None,
            networks_last_updated: None,
            network_throughput_cache: HashMap::new(),
            container_detail_cache: ContainerDetailCache::default(),
            container_logs_cache: ContainerLogsCache::default(),
            _sync_task: None,
            _refresh_task: None,
            _container_action_task: None,
            _container_detail_task: None,
            _container_logs_task: None,
            _container_shell_task: None,
            _image_sync_task: None,
            _image_action_task: None,
            _volume_sync_task: None,
            _volume_action_task: None,
            _network_sync_task: None,
            _network_refresh_task: None,
            _network_action_task: None,
            _network_throughput_task: None,
            _connection_monitor_task: None,
            _connection_probe_task: None,
            _reconnect_countdown_task: None,
            _update_check_task: None,
        }
    }

    pub fn start_container_sync(&mut self, cx: &mut Context<Self>) {
        if !self.docker_unavailable() {
            self.is_loading = true;
            self.sync_status = SyncStatus::Loading;
            self.error = None;
            self.refresh_error = None;
            self.clear_reconnect_retry();
        }
        cx.notify();

        let bridge = self.bridge.clone();
        let target = self.active_connection.target.clone();
        let target_id = self.active_connection.id.clone();
        let current_target_id = target_id.clone();

        self._sync_task = Some(cx.spawn(async move |this, cx| {
            let mut snapshots = bridge.subscribe_containers(target);

            let snapshot = snapshots.borrow_and_update().clone();
            let _ = this.update(cx, |model, cx| {
                if model.active_connection.id != current_target_id {
                    return;
                }
                let selected_changed = model.apply_container_snapshot(snapshot, cx);
                model.start_selected_container_detail_if_needed(selected_changed, cx);
                model.start_selected_container_logs_if_needed(selected_changed, cx);
                model.start_selected_container_shell_if_needed(selected_changed, cx);
                model.start_selected_network_throughput_if_needed(cx);
                if model.snapshot_should_probe_connection() {
                    model.probe_active_connection(cx);
                }
                cx.notify();
            });

            loop {
                if snapshots.changed().await.is_err() {
                    break;
                }

                let snapshot = snapshots.borrow_and_update().clone();
                let _ = this.update(cx, |model, cx| {
                    if model.active_connection.id != target_id {
                        return;
                    }
                    let selected_changed = model.apply_container_snapshot(snapshot, cx);
                    model.start_selected_container_detail_if_needed(selected_changed, cx);
                    model.start_selected_container_logs_if_needed(selected_changed, cx);
                    model.start_selected_container_shell_if_needed(selected_changed, cx);
                    model.start_selected_network_throughput_if_needed(cx);
                    if model.snapshot_should_probe_connection() {
                        model.probe_active_connection(cx);
                    }
                    cx.notify();
                });
            }
        }));
    }

    pub fn start_connection_monitor(&mut self, cx: &mut Context<Self>) {
        if self._connection_monitor_task.is_some() {
            return;
        }

        let bridge = self.bridge.clone();
        self._connection_monitor_task = Some(cx.spawn(async move |this, cx| {
            loop {
                let active_connection = cx
                    .background_spawn({
                        let bridge = bridge.clone();
                        async move { bridge.resolve_active_connection() }
                    })
                    .await;

                let _ = this.update(cx, |model, cx| {
                    model.apply_detected_connection(active_connection, cx);
                });

                cx.background_executor()
                    .timer(CONNECTION_MONITOR_INTERVAL)
                    .await;
            }
        }));
    }

    pub fn refresh_containers(&mut self, show_loading: bool, cx: &mut Context<Self>) {
        if show_loading {
            self.is_loading = true;
            self.sync_status = SyncStatus::Loading;
            self.clear_reconnect_retry();
        }
        self.error = None;
        cx.notify();

        let bridge = self.bridge.clone();
        let target_id = self.active_connection.id.clone();

        self._refresh_task = Some(cx.spawn(async move |this, cx| {
            refresh_with_bridge(&this, cx, bridge, target_id, show_loading).await;
        }));
    }

    pub fn reconnect_active_connection(&mut self, cx: &mut Context<Self>) {
        let target = self.active_connection.target.clone();
        self.bridge.stop_session(&target);
        self.sync_status = SyncStatus::Reconnecting;
        self.is_loading = false;
        self.reconnect_retry_at = Some(Instant::now());
        self.reconnect_seconds_remaining = Some(0);
        self.image_error = None;
        self.volume_error = None;
        self.network_error = None;
        self._sync_task = None;
        self._refresh_task = None;
        self._container_detail_task = None;
        self._container_logs_task = None;
        self._container_shell_task = None;
        self._image_sync_task = None;
        self._volume_sync_task = None;
        self._network_sync_task = None;
        self._network_throughput_task = None;
        cx.notify();

        self.start_container_sync(cx);
        self.start_resource_preload(cx);
    }

    pub fn docker_unavailable(&self) -> bool {
        matches!(self.sync_status, SyncStatus::Reconnecting)
            && (self.error.is_some() || self.refresh_error.is_some())
    }

    pub fn control_container(
        &mut self,
        container_id: String,
        action: ContainerAction,
        cx: &mut Context<Self>,
    ) {
        if self.pending_container_action.is_some() {
            return;
        }

        self.pending_container_action = Some(PendingContainerAction {
            container_id: container_id.clone(),
            action,
        });
        self.refresh_error = None;
        self.error = None;
        cx.notify();

        let bridge = self.bridge.clone();

        self._container_action_task = Some(cx.spawn(async move |this, cx| {
            let target = match this.read_with(cx, |model, _| model.active_connection.target.clone())
            {
                Ok(target) => target,
                Err(_) => return,
            };
            let target_id = target.stable_id();

            let action_result = cx
                .background_spawn({
                    let bridge = bridge.clone();
                    let container_id = container_id.clone();
                    async move { bridge.control_container(target, container_id, action) }
                })
                .await;

            match action_result {
                Ok(()) => {
                    refresh_with_bridge(&this, cx, bridge, target_id.clone(), false).await;
                    let _ = this.update(cx, |model, cx| {
                        if model.active_connection.id != target_id {
                            return;
                        }
                        model.pending_container_action = None;
                        cx.notify();
                    });
                }
                Err(error) => {
                    let message = error.to_string();
                    let _ = this.update(cx, |model, cx| {
                        if model.active_connection.id != target_id {
                            return;
                        }
                        model.pending_container_action = None;
                        model.refresh_error = Some(message.clone());
                        if model.containers.is_empty() {
                            model.error = Some(message);
                        }
                        model.probe_active_connection(cx);
                        cx.notify();
                    });
                }
            }
        }));
    }

    #[allow(dead_code)]
    pub fn set_active_connection(
        &mut self,
        active_connection: ActiveConnection,
        cx: &mut Context<Self>,
    ) {
        if self.active_connection.id == active_connection.id {
            return;
        }

        let previous_target = self.active_connection.target.clone();
        self.bridge.stop_session(&previous_target);
        self.active_connection = active_connection;
        self.containers.clear();
        self.selected_container_id = None;
        self.error = None;
        self.refresh_error = None;
        self.is_loading = true;
        self.sync_status = SyncStatus::Loading;
        self.clear_reconnect_retry();
        self.last_updated = None;
        self.pending_container_action = None;
        self.images.clear();
        self.image_error = None;
        self.is_images_loading = false;
        self.is_image_importing = false;
        self.pending_image_action = None;
        self.images_last_updated = None;
        self.volumes.clear();
        self.volume_error = None;
        self.is_volumes_loading = false;
        self.is_volume_importing = false;
        self.pending_volume_action = None;
        self.volumes_last_updated = None;
        self.networks.clear();
        self.network_error = None;
        self.is_networks_loading = false;
        self.pending_network_action = None;
        self.selected_network_node = None;
        self.network_throughput = None;
        self.networks_last_updated = None;
        self.network_throughput_cache.clear();
        self.container_detail = None;
        self.container_logs = None;
        self.container_shell = None;
        self.container_logs_revision = self.container_logs_revision.wrapping_add(1);
        self.container_detail_cache = ContainerDetailCache::default();
        self.container_logs_cache = ContainerLogsCache::default();
        self.container_detail_tab = ContainerDetailTab::Logs;
        self.container_detail_tabs.clear();
        self.container_bottom_maximized = false;
        self._sync_task = None;
        self._refresh_task = None;
        self._container_action_task = None;
        self._container_detail_task = None;
        self._container_logs_task = None;
        self._container_shell_task = None;
        self._image_sync_task = None;
        self._image_action_task = None;
        self._volume_sync_task = None;
        self._volume_action_task = None;
        self._network_sync_task = None;
        self._network_refresh_task = None;
        self._network_action_task = None;
        self._network_throughput_task = None;
        self._reconnect_countdown_task = None;
        cx.notify();

        self.start_container_sync(cx);
        self.start_resource_preload(cx);
    }

    fn apply_detected_connection(
        &mut self,
        active_connection: ActiveConnection,
        cx: &mut Context<Self>,
    ) {
        if self.active_connection.id != active_connection.id {
            self.set_active_connection(active_connection, cx);
        }
    }

    fn probe_active_connection(&mut self, cx: &mut Context<Self>) {
        if self._connection_probe_task.is_some() {
            return;
        }

        let bridge = self.bridge.clone();
        self._connection_probe_task = Some(cx.spawn(async move |this, cx| {
            let active_connection = cx
                .background_spawn({
                    let bridge = bridge.clone();
                    async move { bridge.resolve_active_connection() }
                })
                .await;

            let _ = this.update(cx, |model, cx| {
                model._connection_probe_task = None;
                model.apply_detected_connection(active_connection, cx);
            });
        }));
    }

    fn snapshot_should_probe_connection(&self) -> bool {
        self.refresh_error.is_some() && matches!(self.sync_status, SyncStatus::Reconnecting)
    }

    fn clear_reconnect_retry(&mut self) {
        self.reconnect_retry_at = None;
        self.reconnect_seconds_remaining = None;
    }

    fn apply_reconnect_retry_after(
        &mut self,
        retry_after: Option<Duration>,
        cx: &mut Context<Self>,
    ) {
        let Some(retry_after) = retry_after else {
            if !matches!(self.sync_status, SyncStatus::Reconnecting) {
                self.clear_reconnect_retry();
            }
            return;
        };

        let deadline = Instant::now() + retry_after;
        self.reconnect_retry_at = Some(deadline);
        self.update_reconnect_seconds_remaining();
        self.start_reconnect_countdown(cx);
    }

    fn update_reconnect_seconds_remaining(&mut self) {
        self.reconnect_seconds_remaining = self.reconnect_retry_at.map(|deadline| {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                0
            } else {
                remaining.as_secs().saturating_add(1)
            }
        });
    }

    fn start_reconnect_countdown(&mut self, cx: &mut Context<Self>) {
        if self._reconnect_countdown_task.is_some() {
            return;
        }

        self._reconnect_countdown_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;

                let should_continue = this
                    .update(cx, |model, cx| {
                        if model.reconnect_retry_at.is_none() {
                            model._reconnect_countdown_task = None;
                            cx.notify();
                            return false;
                        }

                        model.update_reconnect_seconds_remaining();
                        let should_continue = matches!(model.sync_status, SyncStatus::Reconnecting)
                            && model.reconnect_retry_at.is_some();
                        if !should_continue {
                            model._reconnect_countdown_task = None;
                        }
                        cx.notify();
                        should_continue
                    })
                    .unwrap_or(false);

                if !should_continue {
                    break;
                }
            }
        }));
    }

    fn apply_container_result(
        &mut self,
        result: anyhow::Result<ContainerSnapshot>,
        initial_load: bool,
        cx: &mut Context<Self>,
    ) {
        self.is_loading = false;

        match result {
            Ok(snapshot) => {
                let selected_changed = self.apply_container_snapshot(snapshot, cx);
                self.start_selected_container_detail_if_needed(selected_changed, cx);
                self.start_selected_container_logs_if_needed(selected_changed, cx);
                self.start_selected_container_shell_if_needed(selected_changed, cx);
                self.start_selected_network_throughput_if_needed(cx);
            }
            Err(error) => {
                let message = error.to_string();
                self.sync_status = SyncStatus::Reconnecting;
                self.refresh_error = Some(message.clone());
                self.apply_reconnect_retry_after(Some(DEFAULT_RECONNECT_RETRY_AFTER), cx);

                if initial_load || self.containers.is_empty() {
                    self.error = Some(message);
                }
            }
        }
    }

    fn apply_container_snapshot(
        &mut self,
        snapshot: ContainerSnapshot,
        cx: &mut Context<Self>,
    ) -> bool {
        let ContainerSnapshot {
            containers,
            status,
            error,
            retry_after,
            last_updated,
        } = snapshot;

        self.is_loading = matches!(status, ConnectionStatus::Connecting) && containers.is_empty();
        self.sync_status = SyncStatus::from(status);
        self.apply_reconnect_retry_after(retry_after, cx);
        if matches!(
            self.sync_status,
            SyncStatus::Live | SyncStatus::Polling | SyncStatus::Loading
        ) && error.is_none()
        {
            self.clear_reconnect_retry();
        }
        let previous_selected_container_id = self.selected_container_id.clone();
        self.selected_container_id = self
            .selected_container_id
            .as_ref()
            .filter(|selected| {
                containers
                    .iter()
                    .any(|container| &container.id == *selected)
            })
            .cloned()
            .or_else(|| containers.first().map(|container| container.id.clone()));
        self.containers = containers;
        self.refresh_error = error.clone();
        self.error = if self.containers.is_empty() {
            error
        } else {
            None
        };

        if let Some(last_updated) = last_updated {
            self.last_updated = Some(last_updated);
        } else if self.last_updated.is_none() && self.refresh_error.is_none() {
            self.last_updated = Some(SystemTime::now());
        }

        let selected_changed = self.selected_container_id != previous_selected_container_id;
        if selected_changed {
            self.restore_selected_container_detail_tab();
            self.container_detail = None;
            if let Some(container_id) = self.selected_container_id.clone() {
                if !self.restore_cached_container_logs(&container_id) {
                    self.container_logs = None;
                    self.container_logs_revision = self.container_logs_revision.wrapping_add(1);
                }
                self.container_shell = None;
            } else {
                self.container_logs = None;
                self.container_shell = None;
                self.container_logs_revision = self.container_logs_revision.wrapping_add(1);
            }
            self.container_bottom_maximized = false;
            self._container_detail_task = None;
            self._container_logs_task = None;
            self._container_shell_task = None;
        }

        selected_changed
    }

    fn apply_image_result(&mut self, result: anyhow::Result<ImageSnapshot>) {
        self.is_images_loading = false;

        match result {
            Ok(snapshot) => {
                self.images = snapshot.images;
                self.image_error = snapshot.error;
                self.images_last_updated = snapshot.last_updated;
            }
            Err(error) => {
                self.image_error = Some(error.to_string());
            }
        }
    }

    fn apply_volume_result(&mut self, result: anyhow::Result<VolumeSnapshot>) {
        self.is_volumes_loading = false;

        match result {
            Ok(snapshot) => {
                self.volumes = snapshot.volumes;
                self.volume_error = snapshot.error;
                self.volumes_last_updated = snapshot.last_updated;
            }
            Err(error) => {
                self.volume_error = Some(error.to_string());
            }
        }
    }

    fn apply_network_result(&mut self, result: anyhow::Result<NetworkSnapshot>) {
        self.is_networks_loading = false;

        match result {
            Ok(snapshot) => {
                self.networks = snapshot.networks;
                self.network_error = snapshot.error;
                self.networks_last_updated = snapshot.last_updated;
                self.restore_selected_network_node();
            }
            Err(error) => {
                self.network_error = Some(error.to_string());
                if self.networks.is_empty() {
                    self.selected_network_node = None;
                }
            }
        }
    }

    pub fn set_locale(&mut self, locale: AppLocale, cx: &mut Context<Self>) {
        self.locale = locale;
        i18n::set_locale(self.locale);
        let _ = self.save_preferences();
        cx.notify();
    }

    pub fn set_theme_mode(&mut self, theme_mode: ThemeMode, cx: &mut Context<Self>) {
        if self.theme_mode == theme_mode {
            return;
        }
        self.theme_mode = theme_mode;
        let _ = self.save_preferences();
        cx.notify();
    }

    pub fn set_font_family(&mut self, font_family: AppFontFamily, cx: &mut Context<Self>) {
        if self.font_family == font_family {
            return;
        }

        self.font_family = font_family;
        let _ = self.save_preferences();
        cx.notify();
    }

    pub fn set_auto_check_updates(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.auto_check_updates == enabled {
            return;
        }

        self.auto_check_updates = enabled;
        let _ = self.save_preferences();
        if enabled {
            self.check_for_updates(cx);
        }
        cx.notify();
    }

    pub fn set_notify_new_version(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.notify_new_version == enabled {
            return;
        }

        self.notify_new_version = enabled;
        let _ = self.save_preferences();
        cx.notify();
    }

    pub fn check_for_updates_if_needed(&mut self, cx: &mut Context<Self>) {
        if self.auto_check_updates {
            self.check_for_updates(cx);
        }
    }

    pub fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        if self.update_status.is_checking() {
            return;
        }

        self.update_status = UpdateStatus::Checking;
        cx.notify();

        self._update_check_task = Some(cx.spawn(async move |this, cx| {
            let status = cx.background_spawn(updates::check_for_updates()).await;
            let _ = this.update(cx, |model, cx| {
                model.update_status = status;
                cx.notify();
            });
        }));
    }

    pub fn set_container_list_width(&mut self, width: u16, cx: &mut Context<Self>) {
        let width = clamp_container_list_width(width);
        if self.container_list_width == width {
            return;
        }

        self.container_list_width = width;
        let _ = self.save_preferences();
        cx.notify();
    }

    pub fn toggle_compose_project(&mut self, project: String, cx: &mut Context<Self>) {
        if !self.expanded_compose_projects.insert(project.clone()) {
            self.expanded_compose_projects.remove(&project);
        }

        cx.notify();
    }

    pub fn set_nav_section(&mut self, section: NavSection, cx: &mut Context<Self>) {
        if self.active_nav == section {
            return;
        }
        self.active_nav = section;
        if section == NavSection::Images
            && self.images_last_updated.is_none()
            && !self.is_images_loading
        {
            self.start_image_sync(cx);
        }
        if section == NavSection::Volumes
            && self.volumes_last_updated.is_none()
            && !self.is_volumes_loading
        {
            self.start_volume_sync(cx);
        }
        if section == NavSection::Networks
            && self.networks_last_updated.is_none()
            && !self.is_networks_loading
        {
            self.refresh_networks(true, cx);
        }
        if section == NavSection::Networks {
            self.start_network_sync(cx);
            self.start_selected_network_throughput_if_needed(cx);
        }
        cx.notify();
    }

    pub fn select_container(&mut self, container_id: Option<String>, cx: &mut Context<Self>) {
        if self.selected_container_id == container_id {
            return;
        }

        self.selected_container_id = container_id;
        self.restore_selected_container_detail_tab();
        if let Some(container_id) = self.selected_container_id.clone() {
            if !self.restore_cached_container_detail(&container_id) {
                self.container_detail = None;
            }
            if !self.restore_cached_container_logs(&container_id) {
                self.container_logs = None;
                self.container_logs_revision = self.container_logs_revision.wrapping_add(1);
            }
            self.container_shell = None;
        } else {
            self.container_detail = None;
            self.container_logs = None;
            self.container_shell = None;
            self.container_logs_revision = self.container_logs_revision.wrapping_add(1);
        }
        self.container_bottom_maximized = false;
        self._container_detail_task = None;
        self._container_logs_task = None;
        self._container_shell_task = None;
        self.start_selected_container_detail_if_needed(true, cx);
        self.start_selected_container_logs_if_needed(true, cx);
        self.start_selected_container_shell_if_needed(true, cx);
        cx.notify();
    }

    pub fn set_search_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.search_text = text;
        cx.notify();
    }

    pub fn set_image_search_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.image_search_text = text;
        cx.notify();
    }

    pub fn set_volume_search_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.volume_search_text = text;
        cx.notify();
    }

    pub fn start_resource_preload(&mut self, cx: &mut Context<Self>) {
        self.start_image_sync(cx);
        self.start_volume_sync(cx);
        if self.networks_last_updated.is_none() && !self.is_networks_loading {
            self.refresh_networks(true, cx);
        }
        self.start_network_sync(cx);
    }

    fn start_image_sync(&mut self, cx: &mut Context<Self>) {
        if self._image_sync_task.is_some() {
            return;
        }

        if self.images_last_updated.is_none() {
            self.is_images_loading = true;
            self.image_error = None;
            cx.notify();
        }

        let bridge = self.bridge.clone();
        let target = self.active_connection.target.clone();
        let target_id = self.active_connection.id.clone();
        let current_target_id = target_id.clone();

        self._image_sync_task = Some(cx.spawn(async move |this, cx| {
            let mut snapshots = bridge.subscribe_images(target);

            let snapshot = snapshots.borrow_and_update().clone();
            let _ = this.update(cx, |model, cx| {
                if model.active_connection.id != current_target_id {
                    return;
                }
                if should_ignore_image_loading_snapshot(&snapshot) {
                    return;
                }
                model.apply_image_result(Ok(snapshot));
                cx.notify();
            });

            loop {
                if snapshots.changed().await.is_err() {
                    break;
                }

                let snapshot = snapshots.borrow_and_update().clone();
                let _ = this.update(cx, |model, cx| {
                    if model.active_connection.id != target_id {
                        return;
                    }
                    model.apply_image_result(Ok(snapshot));
                    cx.notify();
                });
            }
        }));
    }

    fn start_volume_sync(&mut self, cx: &mut Context<Self>) {
        if self._volume_sync_task.is_some() {
            return;
        }

        if self.volumes_last_updated.is_none() {
            self.is_volumes_loading = true;
            self.volume_error = None;
            cx.notify();
        }

        let bridge = self.bridge.clone();
        let target = self.active_connection.target.clone();
        let target_id = self.active_connection.id.clone();
        let current_target_id = target_id.clone();

        self._volume_sync_task = Some(cx.spawn(async move |this, cx| {
            let mut snapshots = bridge.subscribe_volumes(target);

            let snapshot = snapshots.borrow_and_update().clone();
            let _ = this.update(cx, |model, cx| {
                if model.active_connection.id != current_target_id {
                    return;
                }
                if should_ignore_volume_loading_snapshot(&snapshot) {
                    return;
                }
                model.apply_volume_result(Ok(snapshot));
                cx.notify();
            });

            loop {
                if snapshots.changed().await.is_err() {
                    break;
                }

                let snapshot = snapshots.borrow_and_update().clone();
                let _ = this.update(cx, |model, cx| {
                    if model.active_connection.id != target_id {
                        return;
                    }
                    model.apply_volume_result(Ok(snapshot));
                    cx.notify();
                });
            }
        }));
    }

    pub fn remove_image(&mut self, image_id: String, cx: &mut Context<Self>) {
        if self.pending_image_action.is_some() || self.is_image_importing {
            return;
        }

        self.pending_image_action = Some(PendingImageAction {
            image_id: image_id.clone(),
        });
        self.image_error = None;
        cx.notify();

        let bridge = self.bridge.clone();
        self._image_action_task = Some(cx.spawn(async move |this, cx| {
            let target = match this.read_with(cx, |model, _| model.active_connection.target.clone())
            {
                Ok(target) => target,
                Err(_) => return,
            };
            let target_id = target.stable_id();

            let result = cx
                .background_spawn({
                    let bridge = bridge.clone();
                    let image_id = image_id.clone();
                    async move { bridge.remove_image(target, image_id) }
                })
                .await;

            match result {
                Ok(()) => {
                    refresh_images_with_bridge(&this, cx, bridge, target_id.clone()).await;
                    let _ = this.update(cx, |model, cx| {
                        if model.active_connection.id != target_id {
                            return;
                        }
                        model.pending_image_action = None;
                        cx.notify();
                    });
                }
                Err(error) => {
                    let message = error.to_string();
                    let _ = this.update(cx, |model, cx| {
                        if model.active_connection.id != target_id {
                            return;
                        }
                        model.pending_image_action = None;
                        model.image_error = Some(message);
                        cx.notify();
                    });
                }
            }
        }));
    }

    pub fn import_image(&mut self, archive_path: PathBuf, cx: &mut Context<Self>) {
        if self.pending_image_action.is_some() || self.is_image_importing {
            return;
        }

        self.is_image_importing = true;
        self.image_error = None;
        cx.notify();

        let bridge = self.bridge.clone();
        self._image_action_task = Some(cx.spawn(async move |this, cx| {
            let target = match this.read_with(cx, |model, _| model.active_connection.target.clone())
            {
                Ok(target) => target,
                Err(_) => return,
            };
            let target_id = target.stable_id();

            let result = cx
                .background_spawn({
                    let bridge = bridge.clone();
                    let archive_path = archive_path.clone();
                    async move { bridge.import_image(target, archive_path) }
                })
                .await;

            match result {
                Ok(()) => {
                    refresh_images_with_bridge(&this, cx, bridge, target_id.clone()).await;
                    let _ = this.update(cx, |model, cx| {
                        if model.active_connection.id != target_id {
                            return;
                        }
                        model.is_image_importing = false;
                        cx.notify();
                    });
                }
                Err(error) => {
                    let message = error.to_string();
                    let _ = this.update(cx, |model, cx| {
                        if model.active_connection.id != target_id {
                            return;
                        }
                        model.is_image_importing = false;
                        model.image_error = Some(message);
                        cx.notify();
                    });
                }
            }
        }));
    }

    pub fn refresh_networks(&mut self, show_loading: bool, cx: &mut Context<Self>) {
        if show_loading {
            self.is_networks_loading = true;
        }
        self.network_error = None;
        cx.notify();

        let bridge = self.bridge.clone();
        let target_id = self.active_connection.id.clone();

        self._network_refresh_task = Some(cx.spawn(async move |this, cx| {
            refresh_networks_with_bridge(&this, cx, bridge, target_id).await;
        }));
    }

    pub fn create_network(&mut self, config: NetworkCreateConfig, cx: &mut Context<Self>) {
        if self.pending_network_action.is_some() {
            return;
        }

        let network_name = config.name.trim().to_string();
        self.pending_network_action = Some(PendingNetworkAction { network_name });
        self.network_error = None;
        cx.notify();

        let bridge = self.bridge.clone();
        self._network_action_task = Some(cx.spawn(async move |this, cx| {
            let target = match this.read_with(cx, |model, _| model.active_connection.target.clone())
            {
                Ok(target) => target,
                Err(_) => return,
            };
            let target_id = target.stable_id();

            let result = cx
                .background_spawn({
                    let bridge = bridge.clone();
                    let target = target.clone();
                    let config = config.clone();
                    async move {
                        let created_id = bridge.create_network(target.clone(), config)?;
                        let snapshot = bridge.refresh_networks(target)?;
                        Ok::<_, anyhow::Error>((created_id, snapshot))
                    }
                })
                .await;

            let _ = this.update(cx, |model, cx| {
                if model.active_connection.id != target_id {
                    return;
                }

                model.pending_network_action = None;
                match result {
                    Ok((created_id, snapshot)) => {
                        model.apply_network_result(Ok(snapshot));
                        let selection = NetworkNodeSelection::Network {
                            network_id: created_id,
                        };
                        if network_selection_exists(&model.networks, &selection) {
                            model.selected_network_node = Some(selection);
                        }
                        model.start_selected_network_throughput_if_needed(cx);
                    }
                    Err(error) => {
                        model.network_error = Some(error.to_string());
                    }
                }
                cx.notify();
            });
        }));
    }

    pub fn remove_network(&mut self, network_id: String, cx: &mut Context<Self>) {
        if self.pending_network_action.is_some() {
            return;
        }

        self.pending_network_action = Some(PendingNetworkAction {
            network_name: network_id.clone(),
        });
        self.network_error = None;
        cx.notify();

        let bridge = self.bridge.clone();
        self._network_action_task = Some(cx.spawn(async move |this, cx| {
            let target = match this.read_with(cx, |model, _| model.active_connection.target.clone())
            {
                Ok(target) => target,
                Err(_) => return,
            };
            let target_id = target.stable_id();

            let result = cx
                .background_spawn({
                    let bridge = bridge.clone();
                    let target = target.clone();
                    let network_id = network_id.clone();
                    async move {
                        bridge.remove_network(target.clone(), network_id)?;
                        bridge.refresh_networks(target)
                    }
                })
                .await;

            let _ = this.update(cx, |model, cx| {
                if model.active_connection.id != target_id {
                    return;
                }

                model.pending_network_action = None;
                match result {
                    Ok(snapshot) => {
                        model.apply_network_result(Ok(snapshot));
                        model.start_selected_network_throughput_if_needed(cx);
                    }
                    Err(error) => {
                        model.network_error = Some(error.to_string());
                    }
                }
                cx.notify();
            });
        }));
    }

    fn start_network_sync(&mut self, cx: &mut Context<Self>) {
        if self._network_sync_task.is_some() {
            return;
        }

        if self.networks_last_updated.is_none() {
            self.is_networks_loading = true;
            self.network_error = None;
            cx.notify();
        }

        let bridge = self.bridge.clone();
        let target = self.active_connection.target.clone();
        let target_id = self.active_connection.id.clone();
        let current_target_id = target_id.clone();

        self._network_sync_task = Some(cx.spawn(async move |this, cx| {
            let mut snapshots = bridge.subscribe_networks(target);

            let snapshot = snapshots.borrow_and_update().clone();
            let _ = this.update(cx, |model, cx| {
                if model.active_connection.id != current_target_id {
                    return;
                }
                if should_ignore_network_loading_snapshot(&snapshot) {
                    return;
                }
                model.apply_network_result(Ok(snapshot));
                model.start_selected_network_throughput_if_needed(cx);
                cx.notify();
            });

            loop {
                if snapshots.changed().await.is_err() {
                    break;
                }

                let snapshot = snapshots.borrow_and_update().clone();
                let _ = this.update(cx, |model, cx| {
                    if model.active_connection.id != target_id {
                        return;
                    }
                    model.apply_network_result(Ok(snapshot));
                    model.start_selected_network_throughput_if_needed(cx);
                    cx.notify();
                });
            }
        }));
    }

    pub fn select_network_node(&mut self, selection: NetworkNodeSelection, cx: &mut Context<Self>) {
        if self.selected_network_node == Some(selection.clone()) {
            return;
        }

        if network_selection_exists(&self.networks, &selection) {
            self.selected_network_node = Some(selection);
            self.start_selected_network_throughput_if_needed(cx);
            cx.notify();
        }
    }

    fn start_selected_network_throughput_if_needed(&mut self, cx: &mut Context<Self>) {
        if self.active_nav != NavSection::Networks {
            return;
        }

        let Some(throughput_target) = self.selected_network_throughput_target() else {
            self._network_throughput_task = None;
            return;
        };
        let cache_key = self.network_throughput_cache_key(network_selection_for_throughput_target(
            &throughput_target,
        ));

        let target_matches = self
            .network_throughput
            .as_ref()
            .is_some_and(|snapshot| snapshot.target == throughput_target);
        if target_matches && self._network_throughput_task.is_some() {
            return;
        }

        if self.network_throughput.as_ref().is_none_or(|snapshot| {
            !network_throughput_targets_match_selection(&snapshot.target, &throughput_target)
        }) && !self.restore_cached_network_throughput(&cache_key)
        {
            self.network_throughput = Some(NetworkThroughputSnapshot::loading(
                throughput_target.clone(),
            ));
        }
        let bridge = self.bridge.clone();
        let target = self.active_connection.target.clone();
        let target_id = self.active_connection.id.clone();
        let current_target_id = target_id.clone();

        self._network_throughput_task = Some(cx.spawn(async move |this, cx| {
            let mut snapshots =
                bridge.subscribe_network_throughput(target, throughput_target.clone());

            let snapshot = snapshots.borrow_and_update().clone();
            let _ = this.update(cx, |model, cx| {
                if model.active_connection.id != current_target_id {
                    return;
                }
                if model.apply_network_throughput_snapshot(snapshot) {
                    cx.notify();
                }
            });

            loop {
                if snapshots.changed().await.is_err() {
                    break;
                }

                let snapshot = snapshots.borrow_and_update().clone();
                let _ = this.update(cx, |model, cx| {
                    if model.active_connection.id != target_id {
                        return;
                    }
                    if model.apply_network_throughput_snapshot(snapshot) {
                        cx.notify();
                    }
                });
            }
        }));
    }

    pub fn remove_volume(&mut self, volume_name: String, cx: &mut Context<Self>) {
        if self.pending_volume_action.is_some() || self.is_volume_importing {
            return;
        }

        self.pending_volume_action = Some(PendingVolumeAction {
            volume_name: volume_name.clone(),
        });
        self.volume_error = None;
        cx.notify();

        let bridge = self.bridge.clone();
        self._volume_action_task = Some(cx.spawn(async move |this, cx| {
            let target = match this.read_with(cx, |model, _| model.active_connection.target.clone())
            {
                Ok(target) => target,
                Err(_) => return,
            };
            let target_id = target.stable_id();

            let result = cx
                .background_spawn({
                    let bridge = bridge.clone();
                    let volume_name = volume_name.clone();
                    async move { bridge.remove_volume(target, volume_name) }
                })
                .await;

            match result {
                Ok(()) => {
                    refresh_volumes_with_bridge(&this, cx, bridge, target_id.clone()).await;
                    let _ = this.update(cx, |model, cx| {
                        if model.active_connection.id != target_id {
                            return;
                        }
                        model.pending_volume_action = None;
                        cx.notify();
                    });
                }
                Err(error) => {
                    let message = error.to_string();
                    let _ = this.update(cx, |model, cx| {
                        if model.active_connection.id != target_id {
                            return;
                        }
                        model.pending_volume_action = None;
                        model.volume_error = Some(message);
                        cx.notify();
                    });
                }
            }
        }));
    }

    pub fn import_volume_archive(&mut self, archive_path: PathBuf, cx: &mut Context<Self>) {
        if self.pending_volume_action.is_some() || self.is_volume_importing {
            return;
        }

        let volume_name = Bridge::volume_name_from_archive_path(&archive_path);
        self.is_volume_importing = true;
        self.volume_error = None;
        cx.notify();

        let bridge = self.bridge.clone();
        self._volume_action_task = Some(cx.spawn(async move |this, cx| {
            let target = match this.read_with(cx, |model, _| model.active_connection.target.clone())
            {
                Ok(target) => target,
                Err(_) => return,
            };
            let target_id = target.stable_id();

            let result = cx
                .background_spawn({
                    let bridge = bridge.clone();
                    let archive_path = archive_path.clone();
                    let volume_name = volume_name.clone();
                    async move { bridge.import_volume_archive(target, archive_path, volume_name) }
                })
                .await;

            match result {
                Ok(()) => {
                    refresh_volumes_with_bridge(&this, cx, bridge, target_id.clone()).await;
                    let _ = this.update(cx, |model, cx| {
                        if model.active_connection.id != target_id {
                            return;
                        }
                        model.is_volume_importing = false;
                        cx.notify();
                    });
                }
                Err(error) => {
                    let message = error.to_string();
                    let _ = this.update(cx, |model, cx| {
                        if model.active_connection.id != target_id {
                            return;
                        }
                        model.is_volume_importing = false;
                        model.volume_error = Some(message);
                        cx.notify();
                    });
                }
            }
        }));
    }

    pub fn set_container_detail_tab(&mut self, tab: ContainerDetailTab, cx: &mut Context<Self>) {
        if self.container_detail_tab == tab {
            return;
        }
        self.container_detail_tab = tab;
        if let Some(container_id) = self.selected_container_id.clone() {
            let key = self.container_cache_key(container_id);
            self.container_detail_tabs.insert(key, tab);
        }
        self.start_selected_container_logs_if_needed(true, cx);
        self.start_selected_container_shell_if_needed(true, cx);
        cx.notify();
    }

    pub fn toggle_container_bottom_maximized(&mut self, cx: &mut Context<Self>) {
        self.container_bottom_maximized = !self.container_bottom_maximized;
        cx.notify();
    }

    pub fn open_selected_container_shell(&self) -> anyhow::Result<ContainerShellSession> {
        let container_id = self
            .selected_container_id
            .clone()
            .ok_or_else(|| anyhow::anyhow!("no selected container"))?;
        if self.container_detail_tab != ContainerDetailTab::Shell {
            anyhow::bail!("container shell tab is not active");
        }
        self.bridge
            .open_container_shell(self.active_connection.target.clone(), container_id)
    }

    pub fn set_container_log_filter(&mut self, text: String, cx: &mut Context<Self>) {
        self.container_log_filter = text;
        self.container_logs_revision = self.container_logs_revision.wrapping_add(1);
        cx.notify();
    }

    pub fn clear_container_logs(&mut self, cx: &mut Context<Self>) {
        if let Some(logs) = self.container_logs.as_mut() {
            logs.lines = Arc::new(Vec::new());
        }
        self.container_logs_revision = self.container_logs_revision.wrapping_add(1);
        if let Some(container_id) = self.selected_container_id.clone() {
            let key = self.container_cache_key(container_id);
            if let Some(logs) = self.container_logs.clone() {
                self.container_logs_cache.insert(
                    key,
                    CachedContainerLogs {
                        snapshot: logs,
                        revision: self.container_logs_revision,
                    },
                );
            } else {
                self.container_logs_cache.remove(&key);
            }
        }
        cx.notify();
    }

    fn save_preferences(&self) -> std::io::Result<()> {
        AppPreferences {
            locale: self.locale,
            theme_mode: self.theme_mode,
            font_family: self.font_family,
            auto_check_updates: self.auto_check_updates,
            notify_new_version: self.notify_new_version,
            container_list_width: self.container_list_width,
        }
        .save()
    }

    fn restore_selected_network_node(&mut self) {
        if self
            .selected_network_node
            .as_ref()
            .is_some_and(|selection| network_selection_exists(&self.networks, selection))
        {
            return;
        }

        self.selected_network_node =
            self.networks
                .first()
                .map(|network| NetworkNodeSelection::Network {
                    network_id: network.id.clone(),
                });
    }

    fn selected_network_throughput_target(&self) -> Option<NetworkThroughputTarget> {
        network_throughput_target_for_selection(
            self.selected_network_node.as_ref(),
            &self.networks,
            &self.containers,
        )
    }

    fn network_throughput_cache_key(
        &self,
        selection: NetworkNodeSelection,
    ) -> NetworkThroughputCacheKey {
        NetworkThroughputCacheKey {
            connection_id: self.active_connection.id.clone(),
            selection,
        }
    }

    fn restore_cached_network_throughput(&mut self, key: &NetworkThroughputCacheKey) -> bool {
        let Some(cached) = self.network_throughput_cache.get(key).cloned() else {
            return false;
        };

        self.network_throughput = Some(cached);
        true
    }

    fn cache_network_throughput_snapshot(&mut self, snapshot: &NetworkThroughputSnapshot) {
        if snapshot.status == NetworkThroughputStatus::Loading && snapshot.history.is_empty() {
            return;
        }

        let key = self.network_throughput_cache_key(network_selection_for_throughput_target(
            &snapshot.target,
        ));
        self.network_throughput_cache.insert(key, snapshot.clone());
    }

    fn apply_network_throughput_snapshot(&mut self, snapshot: NetworkThroughputSnapshot) -> bool {
        let Some(target) = self.selected_network_throughput_target() else {
            return false;
        };
        if !network_throughput_targets_match_selection(&target, &snapshot.target) {
            return false;
        }

        if should_ignore_network_throughput_warmup_snapshot(
            self.network_throughput.as_ref(),
            &snapshot,
        ) {
            return false;
        }

        let snapshot = merge_network_throughput_history(self.network_throughput.as_ref(), snapshot);
        self.cache_network_throughput_snapshot(&snapshot);

        let changed = self
            .network_throughput
            .as_ref()
            .is_none_or(|current| current != &snapshot);
        if changed {
            self.network_throughput = Some(snapshot);
        }

        changed
    }

    fn container_cache_key(&self, container_id: impl Into<String>) -> ContainerCacheKey {
        ContainerCacheKey {
            connection_id: self.active_connection.id.clone(),
            container_id: container_id.into(),
        }
    }

    fn restore_selected_container_detail_tab(&mut self) {
        let Some(container_id) = self.selected_container_id.clone() else {
            self.container_detail_tab = ContainerDetailTab::Logs;
            return;
        };

        let key = self.container_cache_key(container_id);
        self.container_detail_tab = self
            .container_detail_tabs
            .get(&key)
            .copied()
            .unwrap_or(ContainerDetailTab::Logs);
    }

    fn restore_cached_container_detail(&mut self, container_id: &str) -> bool {
        let key = self.container_cache_key(container_id.to_string());
        let Some(cached) = self.container_detail_cache.get(&key) else {
            return false;
        };

        self.container_detail = Some(cached);
        true
    }

    fn cache_current_container_detail(&mut self, snapshot: ContainerDetailSnapshot) {
        if snapshot.status == ContainerDetailStatus::Loading {
            return;
        }

        let key = self.container_cache_key(snapshot.container_id.clone());
        self.container_detail_cache.insert(key, snapshot);
    }

    fn restore_cached_container_logs(&mut self, container_id: &str) -> bool {
        let key = self.container_cache_key(container_id.to_string());
        let Some(cached) = self.container_logs_cache.get(&key) else {
            return false;
        };

        self.container_logs = Some(cached.snapshot);
        self.container_logs_revision = cached.revision;
        true
    }

    fn cache_current_container_logs(&mut self, snapshot: ContainerLogsSnapshot) {
        if snapshot.status == ContainerLogsStatus::Loading {
            return;
        }

        let key = self.container_cache_key(snapshot.container_id.clone());
        self.container_logs_cache.insert(
            key,
            CachedContainerLogs {
                snapshot,
                revision: self.container_logs_revision,
            },
        );
    }

    fn apply_container_logs_snapshot(&mut self, snapshot: ContainerLogsSnapshot) -> bool {
        if self.selected_container_id.as_deref() != Some(&snapshot.container_id) {
            return false;
        }

        if should_ignore_warmup_logs_snapshot(self.container_logs.as_ref(), &snapshot) {
            return false;
        }

        let previous = self.container_logs.as_ref();
        let changed = previous.is_none_or(|logs| {
            logs.status != snapshot.status
                || logs.error != snapshot.error
                || logs.lines.len() != snapshot.lines.len()
                || logs.lines != snapshot.lines
        });

        if changed {
            self.container_logs_revision = self.container_logs_revision.wrapping_add(1);
            self.container_logs = Some(snapshot.clone());
            self.cache_current_container_logs(snapshot);
        }
        changed
    }

    fn apply_container_detail_snapshot(&mut self, snapshot: ContainerDetailSnapshot) -> bool {
        if self.selected_container_id.as_deref() != Some(&snapshot.container_id) {
            return false;
        }

        if should_ignore_detail_loading_snapshot(self.container_detail.as_ref(), &snapshot) {
            return false;
        }

        let snapshot = merge_container_detail_history(self.container_detail.as_ref(), snapshot);
        let previous = self.container_detail.as_ref();
        let changed = previous.is_none_or(|detail| {
            detail.status != snapshot.status
                || detail.error != snapshot.error
                || detail.detail != snapshot.detail
                || detail.latest != snapshot.latest
                || detail.history != snapshot.history
        });

        if changed {
            self.container_detail = Some(snapshot.clone());
            self.cache_current_container_detail(snapshot);
        }
        changed
    }

    fn apply_container_shell_snapshot(&mut self, snapshot: ContainerShellSnapshot) -> bool {
        if self.selected_container_id.as_deref() != Some(&snapshot.container_id) {
            return false;
        }

        let previous = self.container_shell.as_ref();
        let changed = previous
            .is_none_or(|shell| shell.status != snapshot.status || shell.error != snapshot.error);

        if changed {
            self.container_shell = Some(snapshot);
        }
        changed
    }

    fn start_selected_container_detail_if_needed(
        &mut self,
        selected_changed: bool,
        cx: &mut Context<Self>,
    ) {
        if !selected_changed && self._container_detail_task.is_some() {
            return;
        }

        let Some(container_id) = self.selected_container_id.clone() else {
            self.container_detail = None;
            self._container_detail_task = None;
            return;
        };

        if !self
            .containers
            .iter()
            .any(|container| container.id == container_id)
        {
            self.container_detail = None;
            self._container_detail_task = None;
            return;
        }

        let bridge = self.bridge.clone();
        let target = self.active_connection.target.clone();
        let target_id = self.active_connection.id.clone();
        let current_target_id = target_id.clone();
        if self
            .container_detail
            .as_ref()
            .is_none_or(|detail| detail.container_id != container_id)
            && !self.restore_cached_container_detail(&container_id)
        {
            self.container_detail = Some(ContainerDetailSnapshot::loading(container_id.clone()));
        }

        self._container_detail_task = Some(cx.spawn(async move |this, cx| {
            let mut snapshots = bridge.subscribe_container_detail(target, container_id.clone());

            let snapshot = snapshots.borrow_and_update().clone();
            let _ = this.update(cx, |model, cx| {
                if model.active_connection.id != current_target_id {
                    return;
                }
                if model.apply_container_detail_snapshot(snapshot) {
                    cx.notify();
                }
            });

            loop {
                if snapshots.changed().await.is_err() {
                    break;
                }

                let snapshot = snapshots.borrow_and_update().clone();
                let _ = this.update(cx, |model, cx| {
                    if model.active_connection.id != target_id {
                        return;
                    }
                    if model.apply_container_detail_snapshot(snapshot) {
                        cx.notify();
                    }
                });
            }
        }));
    }

    fn start_selected_container_logs_if_needed(
        &mut self,
        selected_changed: bool,
        cx: &mut Context<Self>,
    ) {
        if !should_stream_selected_container_logs(self.container_detail_tab) {
            self.container_logs = None;
            self._container_logs_task = None;
            return;
        }

        let should_retry_finished_stream = self
            .container_logs
            .as_ref()
            .is_some_and(|logs| logs.status == ContainerLogsStatus::Error);
        if !selected_changed && self._container_logs_task.is_some() && !should_retry_finished_stream
        {
            return;
        }

        let Some(container_id) = self.selected_container_id.clone() else {
            self.container_logs = None;
            self._container_logs_task = None;
            return;
        };

        if !self
            .containers
            .iter()
            .any(|container| container.id == container_id)
        {
            self.container_logs = None;
            self._container_logs_task = None;
            return;
        }

        let bridge = self.bridge.clone();
        let target = self.active_connection.target.clone();
        let target_id = self.active_connection.id.clone();
        let current_target_id = target_id.clone();
        if self
            .container_logs
            .as_ref()
            .is_none_or(|logs| logs.container_id != container_id)
            && !self.restore_cached_container_logs(&container_id)
        {
            self.container_logs = Some(ContainerLogsSnapshot::loading(container_id.clone()));
        }

        self._container_logs_task = Some(cx.spawn(async move |this, cx| {
            let mut snapshots = bridge.subscribe_container_logs(target, container_id.clone());

            let snapshot = snapshots.borrow_and_update().clone();
            let _ = this.update(cx, |model, cx| {
                if model.active_connection.id != current_target_id {
                    return;
                }
                if model.apply_container_logs_snapshot(snapshot) {
                    cx.notify();
                }
            });

            loop {
                if snapshots.changed().await.is_err() {
                    break;
                }

                let snapshot = snapshots.borrow_and_update().clone();
                let _ = this.update(cx, |model, cx| {
                    if model.active_connection.id != target_id {
                        return;
                    }
                    if model.apply_container_logs_snapshot(snapshot) {
                        cx.notify();
                    }
                });
            }
        }));
    }

    fn start_selected_container_shell_if_needed(
        &mut self,
        selected_changed: bool,
        cx: &mut Context<Self>,
    ) {
        if self.container_detail_tab != ContainerDetailTab::Shell {
            return;
        }

        if !selected_changed && self._container_shell_task.is_some() {
            return;
        }

        let Some(container_id) = self.selected_container_id.clone() else {
            self.container_shell = None;
            self._container_shell_task = None;
            return;
        };

        if !self
            .containers
            .iter()
            .any(|container| container.id == container_id)
        {
            self.container_shell = None;
            self._container_shell_task = None;
            return;
        }

        if !self
            .containers
            .iter()
            .find(|container| container.id == container_id)
            .is_some_and(model_container_is_running)
        {
            self.container_shell = Some(ContainerShellSnapshot {
                container_id,
                status: ContainerShellStatus::Stopped,
                error: None,
                last_updated: Some(SystemTime::now()),
            });
            self._container_shell_task = None;
            return;
        }

        let bridge = self.bridge.clone();
        let target = self.active_connection.target.clone();
        let target_id = self.active_connection.id.clone();
        let current_target_id = target_id.clone();
        self.container_shell = Some(ContainerShellSnapshot::loading(container_id.clone()));

        self._container_shell_task = Some(cx.spawn(async move |this, cx| {
            let mut snapshots = bridge.subscribe_container_shell(target, container_id.clone());

            let snapshot = snapshots.borrow_and_update().clone();
            let _ = this.update(cx, |model, cx| {
                if model.active_connection.id != current_target_id {
                    return;
                }
                if model.apply_container_shell_snapshot(snapshot) {
                    cx.notify();
                }
            });

            loop {
                if snapshots.changed().await.is_err() {
                    break;
                }

                let snapshot = snapshots.borrow_and_update().clone();
                let _ = this.update(cx, |model, cx| {
                    if model.active_connection.id != target_id {
                        return;
                    }
                    if model.apply_container_shell_snapshot(snapshot) {
                        cx.notify();
                    }
                });
            }
        }));
    }
}

impl ContainerDetailCache {
    fn get(&mut self, key: &ContainerCacheKey) -> Option<ContainerDetailSnapshot> {
        let cached = self.entries.get(key).cloned()?;
        self.touch(key.clone());
        Some(cached)
    }

    fn insert(&mut self, key: ContainerCacheKey, value: ContainerDetailSnapshot) {
        self.entries.insert(key.clone(), value);
        self.touch(key);
        self.evict_over_limit();
    }

    fn touch(&mut self, key: ContainerCacheKey) {
        self.lru.retain(|existing| existing != &key);
        self.lru.push_back(key);
    }

    fn evict_over_limit(&mut self) {
        while self.entries.len() > CONTAINER_DETAIL_CACHE_LIMIT {
            let Some(key) = self.lru.pop_front() else {
                break;
            };
            self.entries.remove(&key);
        }
    }
}

impl ContainerLogsCache {
    fn get(&mut self, key: &ContainerCacheKey) -> Option<CachedContainerLogs> {
        let cached = self.entries.get(key).cloned()?;
        self.touch(key.clone());
        Some(cached)
    }

    fn insert(&mut self, key: ContainerCacheKey, value: CachedContainerLogs) {
        self.entries.insert(key.clone(), value);
        self.touch(key);
        self.evict_over_limit();
    }

    fn remove(&mut self, key: &ContainerCacheKey) {
        self.entries.remove(key);
        self.lru.retain(|existing| existing != key);
    }

    fn touch(&mut self, key: ContainerCacheKey) {
        self.lru.retain(|existing| existing != &key);
        self.lru.push_back(key);
    }

    fn evict_over_limit(&mut self) {
        while self.entries.len() > CONTAINER_LOGS_CACHE_LIMIT {
            let Some(key) = self.lru.pop_front() else {
                break;
            };
            self.entries.remove(&key);
        }
    }
}

fn should_stream_selected_container_logs(tab: ContainerDetailTab) -> bool {
    tab == ContainerDetailTab::Logs
}

fn model_container_is_running(container: &ContainerSummary) -> bool {
    container
        .state
        .as_deref()
        .is_some_and(|state| state.eq_ignore_ascii_case("running"))
}

fn should_ignore_warmup_logs_snapshot(
    current: Option<&ContainerLogsSnapshot>,
    incoming: &ContainerLogsSnapshot,
) -> bool {
    current.is_some_and(|logs| {
        logs.container_id == incoming.container_id
            && logs.status != ContainerLogsStatus::Loading
            && incoming.lines.is_empty()
            && matches!(
                incoming.status,
                ContainerLogsStatus::Loading | ContainerLogsStatus::Live
            )
    })
}

fn should_ignore_detail_loading_snapshot(
    current: Option<&ContainerDetailSnapshot>,
    incoming: &ContainerDetailSnapshot,
) -> bool {
    incoming.status == ContainerDetailStatus::Loading
        && current.is_some_and(|detail| {
            detail.container_id == incoming.container_id
                && detail.status != ContainerDetailStatus::Loading
        })
}

fn should_ignore_network_throughput_warmup_snapshot(
    current: Option<&NetworkThroughputSnapshot>,
    incoming: &NetworkThroughputSnapshot,
) -> bool {
    incoming.status == NetworkThroughputStatus::Loading
        && current.is_some_and(|throughput| {
            network_throughput_targets_match_selection(&throughput.target, &incoming.target)
                && throughput.status != NetworkThroughputStatus::Loading
                && !throughput.history.is_empty()
        })
}

fn merge_container_detail_history(
    current: Option<&ContainerDetailSnapshot>,
    mut incoming: ContainerDetailSnapshot,
) -> ContainerDetailSnapshot {
    let Some(current) = current else {
        return incoming;
    };

    if current.container_id != incoming.container_id
        || current.history.is_empty()
        || incoming.history.len() >= current.history.len()
    {
        return incoming;
    }

    if incoming.history.is_empty() {
        incoming.history = current.history.clone();
        return incoming;
    }

    let limit = current.history.len();
    let retained = limit.saturating_sub(incoming.history.len());
    let mut history = current
        .history
        .iter()
        .skip(current.history.len().saturating_sub(retained))
        .cloned()
        .collect::<Vec<_>>();

    let mut next_sequence = history
        .last()
        .map(|point| point.sequence.saturating_add(1))
        .unwrap_or_default();
    for mut point in incoming.history {
        point.sequence = next_sequence;
        next_sequence = next_sequence.saturating_add(1);
        history.push(point);
    }

    incoming.history = history;
    incoming
}

impl From<ConnectionStatus> for SyncStatus {
    fn from(status: ConnectionStatus) -> Self {
        match status {
            ConnectionStatus::Connecting => SyncStatus::Loading,
            ConnectionStatus::Live => SyncStatus::Live,
            ConnectionStatus::Polling => SyncStatus::Polling,
            ConnectionStatus::Reconnecting | ConnectionStatus::Error => SyncStatus::Reconnecting,
        }
    }
}

async fn refresh_with_bridge(
    this: &WeakEntity<WorkspaceModel>,
    cx: &mut AsyncApp,
    bridge: Arc<Bridge>,
    target_id: String,
    initial_load: bool,
) {
    let target = match this.read_with(cx, |model, _| {
        if model.active_connection.id == target_id {
            Some(model.active_connection.target.clone())
        } else {
            None
        }
    }) {
        Ok(Some(target)) => target,
        Ok(None) => return,
        Err(_) => return,
    };

    let result = cx
        .background_spawn(async move { bridge.refresh_containers(target) })
        .await;

    let _ = this.update(cx, |model, cx| {
        if model.active_connection.id != target_id {
            return;
        }
        model.apply_container_result(result, initial_load, cx);
        if model.snapshot_should_probe_connection() {
            model.probe_active_connection(cx);
        }
        cx.notify();
    });
}

async fn refresh_images_with_bridge(
    this: &WeakEntity<WorkspaceModel>,
    cx: &mut AsyncApp,
    bridge: Arc<Bridge>,
    target_id: String,
) {
    let target = match this.read_with(cx, |model, _| {
        if model.active_connection.id == target_id {
            Some(model.active_connection.target.clone())
        } else {
            None
        }
    }) {
        Ok(Some(target)) => target,
        Ok(None) => return,
        Err(_) => return,
    };

    let result = cx
        .background_spawn(async move { bridge.refresh_images(target) })
        .await;

    let _ = this.update(cx, |model, cx| {
        if model.active_connection.id != target_id {
            return;
        }
        model.apply_image_result(result);
        cx.notify();
    });
}

async fn refresh_volumes_with_bridge(
    this: &WeakEntity<WorkspaceModel>,
    cx: &mut AsyncApp,
    bridge: Arc<Bridge>,
    target_id: String,
) {
    let target = match this.read_with(cx, |model, _| {
        if model.active_connection.id == target_id {
            Some(model.active_connection.target.clone())
        } else {
            None
        }
    }) {
        Ok(Some(target)) => target,
        Ok(None) => return,
        Err(_) => return,
    };

    let result = cx
        .background_spawn(async move { bridge.refresh_volumes(target) })
        .await;

    let _ = this.update(cx, |model, cx| {
        if model.active_connection.id != target_id {
            return;
        }
        model.apply_volume_result(result);
        cx.notify();
    });
}

async fn refresh_networks_with_bridge(
    this: &WeakEntity<WorkspaceModel>,
    cx: &mut AsyncApp,
    bridge: Arc<Bridge>,
    target_id: String,
) {
    let target = match this.read_with(cx, |model, _| {
        if model.active_connection.id == target_id {
            Some(model.active_connection.target.clone())
        } else {
            None
        }
    }) {
        Ok(Some(target)) => target,
        Ok(None) => return,
        Err(_) => return,
    };

    let result = cx
        .background_spawn(async move { bridge.refresh_networks(target) })
        .await;

    let _ = this.update(cx, |model, cx| {
        if model.active_connection.id != target_id {
            return;
        }
        model.apply_network_result(result);
        model.start_selected_network_throughput_if_needed(cx);
        cx.notify();
    });
}

fn should_ignore_network_loading_snapshot(snapshot: &NetworkSnapshot) -> bool {
    snapshot.networks.is_empty() && snapshot.error.is_none() && snapshot.last_updated.is_none()
}

fn should_ignore_image_loading_snapshot(snapshot: &ImageSnapshot) -> bool {
    snapshot.images.is_empty() && snapshot.error.is_none() && snapshot.last_updated.is_none()
}

fn should_ignore_volume_loading_snapshot(snapshot: &VolumeSnapshot) -> bool {
    snapshot.volumes.is_empty() && snapshot.error.is_none() && snapshot.last_updated.is_none()
}

fn network_selection_exists(networks: &[NetworkSummary], selection: &NetworkNodeSelection) -> bool {
    match selection {
        NetworkNodeSelection::Network { network_id } => {
            networks.iter().any(|network| &network.id == network_id)
        }
        NetworkNodeSelection::Container {
            network_id,
            container_id,
        } => networks
            .iter()
            .find(|network| &network.id == network_id)
            .is_some_and(|network| {
                network
                    .endpoints
                    .iter()
                    .any(|endpoint| &endpoint.container_id == container_id)
            }),
    }
}

fn network_throughput_target_for_selection(
    selection: Option<&NetworkNodeSelection>,
    networks: &[NetworkSummary],
    containers: &[ContainerSummary],
) -> Option<NetworkThroughputTarget> {
    let selection = selection.cloned().or_else(|| {
        networks
            .first()
            .map(|network| NetworkNodeSelection::Network {
                network_id: network.id.clone(),
            })
    })?;

    match &selection {
        NetworkNodeSelection::Network { network_id } => {
            let network = networks.iter().find(|network| &network.id == network_id)?;
            let mut container_ids = network
                .endpoints
                .iter()
                .filter(|endpoint| {
                    containers
                        .iter()
                        .find(|container| container.id == endpoint.container_id)
                        .is_some_and(model_container_is_running)
                })
                .map(|endpoint| endpoint.container_id.clone())
                .collect::<Vec<_>>();
            container_ids.sort();
            container_ids.dedup();

            Some(NetworkThroughputTarget::Network {
                network_id: network_id.clone(),
                container_ids,
            })
        }
        NetworkNodeSelection::Container {
            network_id,
            container_id,
        } => {
            if !network_selection_exists(networks, &selection) {
                return None;
            }

            let is_running = containers
                .iter()
                .find(|container| &container.id == container_id)
                .is_some_and(model_container_is_running);

            Some(NetworkThroughputTarget::Container {
                network_id: network_id.clone(),
                container_id: container_id.clone(),
                is_running,
            })
        }
    }
}

fn network_selection_for_throughput_target(
    target: &NetworkThroughputTarget,
) -> NetworkNodeSelection {
    match target {
        NetworkThroughputTarget::Network { network_id, .. } => NetworkNodeSelection::Network {
            network_id: network_id.clone(),
        },
        NetworkThroughputTarget::Container {
            network_id,
            container_id,
            ..
        } => NetworkNodeSelection::Container {
            network_id: network_id.clone(),
            container_id: container_id.clone(),
        },
    }
}

fn network_throughput_targets_match_selection(
    left: &NetworkThroughputTarget,
    right: &NetworkThroughputTarget,
) -> bool {
    network_selection_for_throughput_target(left) == network_selection_for_throughput_target(right)
}

fn merge_network_throughput_history(
    current: Option<&NetworkThroughputSnapshot>,
    mut incoming: NetworkThroughputSnapshot,
) -> NetworkThroughputSnapshot {
    let Some(current) = current else {
        return incoming;
    };

    if !network_throughput_targets_match_selection(&current.target, &incoming.target)
        || current.history.is_empty()
        || incoming.history.is_empty()
        || incoming.history.len() >= current.history.len()
    {
        return incoming;
    }

    let limit = current.history.len();
    let retained = limit.saturating_sub(incoming.history.len());
    let mut history = current
        .history
        .iter()
        .skip(current.history.len().saturating_sub(retained))
        .cloned()
        .collect::<Vec<_>>();

    let mut next_sequence = history
        .last()
        .map(|point| point.sequence.saturating_add(1))
        .unwrap_or_default();
    for mut point in incoming.history {
        point.sequence = next_sequence;
        next_sequence = next_sequence.saturating_add(1);
        history.push(point);
    }

    incoming.history = history;
    incoming
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        bridge::{
            ContainerDetailSnapshot, ContainerDetailStatus, ContainerLogsSnapshot,
            ContainerLogsStatus, ImageSnapshot, NetworkThroughputSnapshot, NetworkThroughputStatus,
            VolumeSnapshot,
        },
        domain::{
            ContainerLogLine, ContainerLogStreamKind, ContainerMetricPoint, ContainerPortSummary,
            ContainerSummary, ImageSummary, NetworkEndpointSummary, NetworkSummary,
            NetworkThroughputPoint, NetworkThroughputStats, NetworkThroughputTarget, VolumeSummary,
        },
    };

    use super::{
        CONTAINER_DETAIL_CACHE_LIMIT, CONTAINER_LOGS_CACHE_LIMIT, CachedContainerLogs,
        ContainerCacheKey, ContainerDetailCache, ContainerDetailTab, ContainerLogsCache,
        NetworkNodeSelection, merge_container_detail_history, merge_network_throughput_history,
        network_selection_exists, network_throughput_target_for_selection,
        should_ignore_detail_loading_snapshot, should_ignore_image_loading_snapshot,
        should_ignore_network_throughput_warmup_snapshot, should_ignore_volume_loading_snapshot,
        should_ignore_warmup_logs_snapshot, should_stream_selected_container_logs,
    };

    #[test]
    fn streams_logs_only_for_logs_tab() {
        assert!(should_stream_selected_container_logs(
            ContainerDetailTab::Logs
        ));
        assert!(!should_stream_selected_container_logs(
            ContainerDetailTab::Shell
        ));
    }

    #[test]
    fn container_detail_cache_evicts_least_recently_used_entry() {
        let mut cache = ContainerDetailCache::default();

        for index in 0..CONTAINER_DETAIL_CACHE_LIMIT {
            let key = cache_key("connection", &format!("container-{index}"));
            cache.insert(key, detail_snapshot(&format!("container-{index}")));
        }

        let first_key = cache_key("connection", "container-0");
        assert!(cache.get(&first_key).is_some());

        cache.insert(
            cache_key("connection", "container-overflow"),
            detail_snapshot("container-overflow"),
        );

        assert!(cache.get(&cache_key("connection", "container-1")).is_none());
        assert!(cache.get(&first_key).is_some());
    }

    #[test]
    fn container_logs_cache_returns_cached_snapshot_and_revision() {
        let mut cache = ContainerLogsCache::default();
        let key = cache_key("connection", "container");
        let snapshot = logs_snapshot("container", &["line one"]);

        cache.insert(
            key.clone(),
            CachedContainerLogs {
                snapshot: snapshot.clone(),
                revision: 7,
            },
        );

        let cached = cache.get(&key).unwrap();

        assert_eq!(cached.revision, 7);
        assert_eq!(cached.snapshot.container_id, "container");
        assert_eq!(cached.snapshot.lines.len(), 1);
    }

    #[test]
    fn container_logs_cache_evicts_least_recently_used_entry() {
        let mut cache = ContainerLogsCache::default();

        for index in 0..CONTAINER_LOGS_CACHE_LIMIT {
            let key = cache_key("connection", &format!("container-{index}"));
            cache.insert(
                key,
                CachedContainerLogs {
                    snapshot: logs_snapshot(&format!("container-{index}"), &["cached"]),
                    revision: index as u64,
                },
            );
        }

        let first_key = cache_key("connection", "container-0");
        assert!(cache.get(&first_key).is_some());

        let overflow_key = cache_key("connection", "container-overflow");
        cache.insert(
            overflow_key,
            CachedContainerLogs {
                snapshot: logs_snapshot("container-overflow", &["new"]),
                revision: 99,
            },
        );

        assert!(cache.get(&cache_key("connection", "container-1")).is_none());
        assert!(cache.get(&first_key).is_some());
    }

    #[test]
    fn container_logs_cache_remove_drops_entry_and_lru_record() {
        let mut cache = ContainerLogsCache::default();
        let key = cache_key("connection", "container");
        cache.insert(
            key.clone(),
            CachedContainerLogs {
                snapshot: logs_snapshot("container", &["cached"]),
                revision: 1,
            },
        );

        cache.remove(&key);

        assert!(cache.get(&key).is_none());
        assert!(cache.lru.is_empty());
    }

    #[test]
    fn cached_logs_are_not_overwritten_by_warmup_snapshot() {
        let existing = logs_snapshot("container", &["cached"]);
        let loading = ContainerLogsSnapshot::loading("container".to_string());
        let empty_live = logs_snapshot("container", &[]);
        let refreshed = logs_snapshot("container", &["new"]);

        assert!(should_ignore_warmup_logs_snapshot(
            Some(&existing),
            &loading
        ));
        assert!(should_ignore_warmup_logs_snapshot(
            Some(&existing),
            &empty_live
        ));
        assert!(!should_ignore_warmup_logs_snapshot(None, &loading));
        assert!(!should_ignore_warmup_logs_snapshot(
            Some(&existing),
            &refreshed
        ));
    }

    #[test]
    fn live_detail_is_not_overwritten_by_loading_snapshot() {
        let existing = ContainerDetailSnapshot {
            container_id: "container".to_string(),
            detail: None,
            latest: None,
            history: Vec::new(),
            status: ContainerDetailStatus::Stopped,
            error: None,
            last_updated: None,
        };
        let loading = ContainerDetailSnapshot::loading("container".to_string());

        assert!(should_ignore_detail_loading_snapshot(
            Some(&existing),
            &loading
        ));
        assert!(!should_ignore_detail_loading_snapshot(None, &loading));
    }

    #[test]
    fn empty_resource_loading_snapshots_are_ignored() {
        assert!(should_ignore_image_loading_snapshot(
            &ImageSnapshot::loading()
        ));
        assert!(should_ignore_volume_loading_snapshot(
            &VolumeSnapshot::loading()
        ));

        assert!(!should_ignore_image_loading_snapshot(&ImageSnapshot {
            images: vec![ImageSummary::new(
                "sha256:123".to_string(),
                vec!["redis:7".to_string()],
                Vec::new(),
                1024,
                None,
                None,
            )],
            error: None,
            last_updated: Some(std::time::UNIX_EPOCH),
        }));
        assert!(!should_ignore_volume_loading_snapshot(&VolumeSnapshot {
            volumes: vec![VolumeSummary::new(
                "db-data".to_string(),
                "local".to_string(),
                "/var/lib/docker/volumes/db-data/_data".to_string(),
                None,
                None,
                None,
            )],
            error: None,
            last_updated: Some(std::time::UNIX_EPOCH),
        }));
    }

    #[test]
    fn resumed_container_detail_keeps_existing_canvas_history() {
        let current = detail_snapshot_with_history("container", &[0., 1., 2., 3.]);
        let incoming = detail_snapshot_with_history("container", &[99.]);

        let merged = merge_container_detail_history(Some(&current), incoming);

        assert_eq!(merged.history.len(), 4);
        assert_eq!(merged.history[0].cpu_percent, 1.);
        assert_eq!(merged.history[3].cpu_percent, 99.);
        assert_eq!(merged.history[3].sequence, 4);
    }

    #[test]
    fn empty_container_detail_snapshot_preserves_existing_canvas_history() {
        let current = detail_snapshot_with_history("container", &[1., 2.]);
        let incoming = detail_snapshot("container");

        let merged = merge_container_detail_history(Some(&current), incoming);

        assert_eq!(merged.history.len(), 2);
        assert_eq!(merged.history[0].cpu_percent, 1.);
        assert_eq!(merged.history[1].cpu_percent, 2.);
    }

    #[test]
    fn network_throughput_history_is_not_overwritten_by_loading_snapshot() {
        let target = NetworkThroughputTarget::Network {
            network_id: "network-a".to_string(),
            container_ids: vec!["container-a".to_string()],
        };
        let existing = NetworkThroughputSnapshot {
            target: target.clone(),
            latest: Some(NetworkThroughputStats::zero(std::time::UNIX_EPOCH)),
            history: vec![NetworkThroughputPoint {
                sequence: 7,
                sample_time: std::time::UNIX_EPOCH,
                rx_bytes_per_sec: 1.,
                tx_bytes_per_sec: 2.,
            }],
            status: NetworkThroughputStatus::Live,
            error: None,
            last_updated: Some(std::time::UNIX_EPOCH),
        };
        let loading = NetworkThroughputSnapshot::loading(target);

        assert!(should_ignore_network_throughput_warmup_snapshot(
            Some(&existing),
            &loading
        ));
        assert!(!should_ignore_network_throughput_warmup_snapshot(
            None, &loading
        ));

        let changed_runtime_target =
            NetworkThroughputSnapshot::loading(NetworkThroughputTarget::Network {
                network_id: "network-a".to_string(),
                container_ids: Vec::new(),
            });
        assert!(should_ignore_network_throughput_warmup_snapshot(
            Some(&existing),
            &changed_runtime_target
        ));
    }

    #[test]
    fn resumed_network_throughput_keeps_existing_canvas_history() {
        let target = NetworkThroughputTarget::Network {
            network_id: "network-a".to_string(),
            container_ids: vec!["container-a".to_string()],
        };
        let current = NetworkThroughputSnapshot {
            target: target.clone(),
            latest: Some(NetworkThroughputStats::zero(std::time::UNIX_EPOCH)),
            history: (0..4)
                .map(|sequence| NetworkThroughputPoint {
                    sequence,
                    sample_time: std::time::UNIX_EPOCH,
                    rx_bytes_per_sec: sequence as f64,
                    tx_bytes_per_sec: 0.,
                })
                .collect(),
            status: NetworkThroughputStatus::Live,
            error: None,
            last_updated: Some(std::time::UNIX_EPOCH),
        };
        let incoming = NetworkThroughputSnapshot {
            target,
            latest: Some(NetworkThroughputStats::zero(std::time::UNIX_EPOCH)),
            history: vec![NetworkThroughputPoint {
                sequence: 0,
                sample_time: std::time::UNIX_EPOCH,
                rx_bytes_per_sec: 99.,
                tx_bytes_per_sec: 0.,
            }],
            status: NetworkThroughputStatus::Live,
            error: None,
            last_updated: Some(std::time::UNIX_EPOCH),
        };

        let merged = merge_network_throughput_history(Some(&current), incoming);

        assert_eq!(merged.history.len(), 4);
        assert_eq!(merged.history[0].rx_bytes_per_sec, 1.);
        assert_eq!(merged.history[3].rx_bytes_per_sec, 99.);
        assert_eq!(merged.history[3].sequence, 4);
    }

    #[test]
    fn network_selection_matches_network_and_container_nodes() {
        let networks = vec![network_summary("network-a", &["container-a"])];

        assert!(network_selection_exists(
            &networks,
            &NetworkNodeSelection::Network {
                network_id: "network-a".to_string()
            }
        ));
        assert!(network_selection_exists(
            &networks,
            &NetworkNodeSelection::Container {
                network_id: "network-a".to_string(),
                container_id: "container-a".to_string(),
            }
        ));
        assert!(!network_selection_exists(
            &networks,
            &NetworkNodeSelection::Container {
                network_id: "network-a".to_string(),
                container_id: "missing".to_string(),
            }
        ));
    }

    #[test]
    fn network_throughput_target_includes_running_endpoint_containers() {
        let networks = vec![network_summary(
            "network-a",
            &["running-container", "stopped-container"],
        )];
        let containers = vec![
            container_summary("running-container", Some("running")),
            container_summary("stopped-container", Some("exited")),
        ];

        let target = network_throughput_target_for_selection(
            Some(&NetworkNodeSelection::Network {
                network_id: "network-a".to_string(),
            }),
            &networks,
            &containers,
        );

        assert_eq!(
            target,
            Some(NetworkThroughputTarget::Network {
                network_id: "network-a".to_string(),
                container_ids: vec!["running-container".to_string()],
            })
        );
    }

    #[test]
    fn network_throughput_target_tracks_container_runtime_state() {
        let networks = vec![network_summary("network-a", &["container-a"])];
        let selection = NetworkNodeSelection::Container {
            network_id: "network-a".to_string(),
            container_id: "container-a".to_string(),
        };

        assert_eq!(
            network_throughput_target_for_selection(
                Some(&selection),
                &networks,
                &[container_summary("container-a", Some("running"))],
            ),
            Some(NetworkThroughputTarget::Container {
                network_id: "network-a".to_string(),
                container_id: "container-a".to_string(),
                is_running: true,
            })
        );
        assert_eq!(
            network_throughput_target_for_selection(
                Some(&selection),
                &networks,
                &[container_summary("container-a", Some("exited"))],
            ),
            Some(NetworkThroughputTarget::Container {
                network_id: "network-a".to_string(),
                container_id: "container-a".to_string(),
                is_running: false,
            })
        );
    }

    fn cache_key(connection_id: &str, container_id: &str) -> ContainerCacheKey {
        ContainerCacheKey {
            connection_id: connection_id.to_string(),
            container_id: container_id.to_string(),
        }
    }

    fn detail_snapshot(container_id: &str) -> ContainerDetailSnapshot {
        ContainerDetailSnapshot {
            container_id: container_id.to_string(),
            detail: None,
            latest: None,
            history: Vec::new(),
            status: ContainerDetailStatus::Live,
            error: None,
            last_updated: None,
        }
    }

    fn detail_snapshot_with_history(
        container_id: &str,
        cpu_values: &[f64],
    ) -> ContainerDetailSnapshot {
        let mut snapshot = detail_snapshot(container_id);
        snapshot.history = cpu_values
            .iter()
            .enumerate()
            .map(|(sequence, cpu_percent)| ContainerMetricPoint {
                sequence: sequence as u64,
                sample_time: std::time::UNIX_EPOCH,
                cpu_percent: *cpu_percent,
                memory_bytes: 0.,
                network_bytes_per_sec: 0.,
                disk_bytes_per_sec: 0.,
            })
            .collect();
        snapshot
    }

    fn logs_snapshot(container_id: &str, lines: &[&str]) -> ContainerLogsSnapshot {
        ContainerLogsSnapshot {
            container_id: container_id.to_string(),
            lines: Arc::new(
                lines
                    .iter()
                    .map(|line| {
                        ContainerLogLine::new(
                            None,
                            ContainerLogStreamKind::Stdout,
                            (*line).to_string(),
                        )
                    })
                    .collect(),
            ),
            status: ContainerLogsStatus::Live,
            error: None,
            last_updated: None,
        }
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

    fn network_summary(id: &str, container_ids: &[&str]) -> NetworkSummary {
        NetworkSummary::new(
            id.to_string(),
            id.to_string(),
            "bridge".to_string(),
            Some("local".to_string()),
            Some("172.17.0.0/16".to_string()),
            Some("172.17.0.1".to_string()),
            false,
            None,
            Vec::new(),
            container_ids
                .iter()
                .map(|container_id| {
                    NetworkEndpointSummary::new(
                        (*container_id).to_string(),
                        (*container_id).to_string(),
                        None,
                        None,
                        None,
                        None,
                    )
                })
                .collect(),
        )
    }
}

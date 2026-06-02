use std::collections::BTreeSet;

use gpui_component::ThemeMode;

use crate::{
    app::{
        AppFontFamily, ContainerDetailTab, NavSection, NetworkNodeSelection,
        PendingContainerAction, PendingImageAction, PendingNetworkAction, PendingVolumeAction,
        UpdateStatus, WorkspaceModel,
    },
    bridge::{
        ContainerDetailSnapshot, ContainerLogsSnapshot, ContainerShellSnapshot,
        NetworkThroughputSnapshot,
    },
    domain::{ContainerSummary, ImageSummary, NetworkSummary, VolumeSummary},
    i18n::AppLocale,
};

#[derive(Clone)]
pub(super) struct WorkspaceSnapshot {
    pub(super) filtered_containers: Vec<ContainerSummary>,
    pub(super) containers: Vec<ContainerSummary>,
    pub(super) selected_container_id: Option<String>,
    pub(super) selected_container: Option<ContainerSummary>,
    pub(super) active_connection_name: String,
    pub(super) active_connection_endpoint: String,
    pub(super) error: Option<String>,
    pub(super) refresh_error: Option<String>,
    pub(super) is_loading: bool,
    pub(super) docker_unavailable: bool,
    pub(super) reconnect_seconds_remaining: Option<u64>,
    pub(super) locale: AppLocale,
    pub(super) theme_mode: ThemeMode,
    pub(super) font_family: AppFontFamily,
    pub(super) auto_check_updates: bool,
    pub(super) notify_new_version: bool,
    pub(super) update_status: UpdateStatus,
    pub(super) container_list_width: u16,
    pub(super) search_text: String,
    pub(super) expanded_compose_projects: BTreeSet<String>,
    pub(super) active_nav: NavSection,
    pub(super) pending_container_action: Option<PendingContainerAction>,
    pub(super) container_detail: Option<ContainerDetailSnapshot>,
    pub(super) container_logs: Option<ContainerLogsSnapshot>,
    pub(super) container_shell: Option<ContainerShellSnapshot>,
    pub(super) container_detail_tab: ContainerDetailTab,
    pub(super) container_bottom_maximized: bool,
    pub(super) container_log_filter: String,
    pub(super) container_logs_revision: u64,
    pub(super) filtered_images: Vec<ImageSummary>,
    pub(super) image_error: Option<String>,
    pub(super) is_images_loading: bool,
    pub(super) is_image_importing: bool,
    pub(super) pending_image_action: Option<PendingImageAction>,
    pub(super) filtered_volumes: Vec<VolumeSummary>,
    pub(super) volume_error: Option<String>,
    pub(super) is_volumes_loading: bool,
    pub(super) is_volume_importing: bool,
    pub(super) pending_volume_action: Option<PendingVolumeAction>,
    pub(super) networks: Vec<NetworkSummary>,
    pub(super) network_error: Option<String>,
    pub(super) is_networks_loading: bool,
    pub(super) pending_network_action: Option<PendingNetworkAction>,
    pub(super) selected_network_node: Option<NetworkNodeSelection>,
    pub(super) network_throughput: Option<NetworkThroughputSnapshot>,
}

impl From<&WorkspaceModel> for WorkspaceSnapshot {
    fn from(model: &WorkspaceModel) -> Self {
        let filtered_containers = filter_containers(&model.containers, &model.search_text);
        let images = images_with_container_usage(&model.images, &model.containers);
        let filtered_images = filter_images(&images, &model.image_search_text);
        let filtered_volumes = filter_volumes(&model.volumes, &model.volume_search_text);
        let selected_container = model
            .selected_container_id
            .as_ref()
            .and_then(|id| {
                model
                    .containers
                    .iter()
                    .find(|container| &container.id == id)
            })
            .cloned();

        Self {
            filtered_containers,
            containers: model.containers.clone(),
            selected_container_id: model.selected_container_id.clone(),
            selected_container,
            active_connection_name: model.active_connection.name.clone(),
            active_connection_endpoint: model.active_connection.endpoint(),
            error: model.error.clone(),
            refresh_error: model.refresh_error.clone(),
            is_loading: model.is_loading,
            docker_unavailable: model.docker_unavailable(),
            reconnect_seconds_remaining: model.reconnect_seconds_remaining,
            locale: model.locale,
            theme_mode: model.theme_mode,
            font_family: model.font_family,
            auto_check_updates: model.auto_check_updates,
            notify_new_version: model.notify_new_version,
            update_status: model.update_status.clone(),
            container_list_width: model.container_list_width,
            search_text: model.search_text.clone(),
            expanded_compose_projects: model.expanded_compose_projects.clone(),
            active_nav: model.active_nav,
            pending_container_action: model.pending_container_action.clone(),
            container_detail: model.container_detail.clone(),
            container_logs: model.container_logs.clone(),
            container_shell: model.container_shell.clone(),
            container_detail_tab: model.container_detail_tab,
            container_bottom_maximized: model.container_bottom_maximized,
            container_log_filter: model.container_log_filter.clone(),
            container_logs_revision: model.container_logs_revision,
            filtered_images,
            image_error: model.image_error.clone(),
            is_images_loading: model.is_images_loading,
            is_image_importing: model.is_image_importing,
            pending_image_action: model.pending_image_action.clone(),
            filtered_volumes,
            volume_error: model.volume_error.clone(),
            is_volumes_loading: model.is_volumes_loading,
            is_volume_importing: model.is_volume_importing,
            pending_volume_action: model.pending_volume_action.clone(),
            networks: model.networks.clone(),
            network_error: model.network_error.clone(),
            is_networks_loading: model.is_networks_loading,
            pending_network_action: model.pending_network_action.clone(),
            selected_network_node: model.selected_network_node.clone(),
            network_throughput: model.network_throughput.clone(),
        }
    }
}

fn filter_containers(containers: &[ContainerSummary], query: &str) -> Vec<ContainerSummary> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return containers.to_vec();
    }

    containers
        .iter()
        .filter(|container| {
            contains(&container.name, &query)
                || contains(&container.image, &query)
                || contains(&container.id, &query)
                || contains(&container.status, &query)
                || container
                    .state
                    .as_deref()
                    .is_some_and(|state| contains(state, &query))
        })
        .cloned()
        .collect()
}

fn contains(value: &str, query: &str) -> bool {
    value.to_lowercase().contains(query)
}

fn filter_images(images: &[ImageSummary], query: &str) -> Vec<ImageSummary> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return images.to_vec();
    }

    images
        .iter()
        .filter(|image| {
            contains(&image.id, &query)
                || image.repo_tags.iter().any(|tag| contains(tag, &query))
                || image
                    .repo_digests
                    .iter()
                    .any(|digest| contains(digest, &query))
        })
        .cloned()
        .collect()
}

fn images_with_container_usage(
    images: &[ImageSummary],
    containers: &[ContainerSummary],
) -> Vec<ImageSummary> {
    images
        .iter()
        .map(|image| {
            let matched_count = containers
                .iter()
                .filter(|container| image_matches_container(image, container))
                .count() as u64;
            let mut image = image.clone();
            image.containers = Some(image.containers.unwrap_or_default().max(matched_count));
            image
        })
        .collect()
}

fn image_matches_container(image: &ImageSummary, container: &ContainerSummary) -> bool {
    let reference = container.image.as_str();
    let image_id = image.id.strip_prefix("sha256:").unwrap_or(&image.id);

    image.repo_tags.iter().any(|tag| tag == reference)
        || image.repo_digests.iter().any(|digest| digest == reference)
        || reference == image.id
        || reference == image_id
        || (reference.len() >= 12 && image_id.starts_with(reference))
}

fn filter_volumes(volumes: &[VolumeSummary], query: &str) -> Vec<VolumeSummary> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return volumes.to_vec();
    }

    volumes
        .iter()
        .filter(|volume| {
            contains(&volume.name, &query)
                || contains(&volume.driver, &query)
                || contains(&volume.mountpoint, &query)
        })
        .cloned()
        .collect()
}

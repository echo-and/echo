use std::{
    collections::HashMap,
    env,
    io::{Read, Write},
    path::{Path, PathBuf},
    pin::Pin,
    task::{Context as TaskContext, Poll},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::{os::unix::net::UnixStream, process::Command};

use anyhow::{Context, Result};
use bollard::{
    Docker, body_try_stream,
    container::LogOutput,
    exec::{StartExecOptions, StartExecResults},
    models::{
        ContainerCreateBody, ContainerInspectResponse, ContainerStatsResponse, EndpointResource,
        HostConfig, Ipam, IpamConfig, Mount, MountType, Network, NetworkCreateRequest,
        NetworkInspect, PortBinding, Volume, VolumeCreateRequest,
    },
    query_parameters::{
        CreateContainerOptionsBuilder, DataUsageOptions, ImportImageOptionsBuilder,
        InspectContainerOptionsBuilder, ListContainersOptionsBuilder, ListImagesOptionsBuilder,
        ListNetworksOptionsBuilder, ListVolumesOptionsBuilder, LogsOptionsBuilder,
        RemoveContainerOptionsBuilder, RemoveImageOptionsBuilder, RemoveVolumeOptionsBuilder,
        RestartContainerOptionsBuilder, StatsOptionsBuilder, StopContainerOptionsBuilder,
        UploadToContainerOptionsBuilder,
    },
};
use flate2::read::GzDecoder;
use futures_util::{Stream, StreamExt};
use serde_json::Value;
use tokio::{fs::File, io::AsyncWrite};
use tokio_util::io::ReaderStream;

use crate::bridge::NetworkCreateConfig;
use crate::{
    bridge::ContainerAction,
    domain::{
        ComposeMetadata, ConnectionTarget, ContainerDetail, ContainerLabelSummary,
        ContainerLogLine, ContainerLogStreamKind, ContainerMountSummary, ContainerPortSummary,
        ContainerRuntimeStats, ContainerSummary, ImageSummary, NetworkEndpointSummary,
        NetworkLabelSummary, NetworkSummary, VolumeSummary,
    },
};

pub struct ContainerStatsStream {
    inner:
        Pin<Box<dyn Stream<Item = Result<ContainerStatsResponse, bollard::errors::Error>> + Send>>,
    previous: Option<StatsCounters>,
}

pub struct ContainerLogsStream {
    inner: Pin<Box<dyn Stream<Item = Result<LogOutput, bollard::errors::Error>> + Send>>,
}

pub struct ContainerShellExec {
    pub exec_id: String,
    pub output: Pin<Box<dyn Stream<Item = Result<LogOutput, bollard::errors::Error>> + Send>>,
    pub input: Pin<Box<dyn AsyncWrite + Send>>,
}

pub async fn list_containers(target: ConnectionTarget) -> Result<Vec<ContainerSummary>> {
    let docker = connect(target)?;
    let options = ListContainersOptionsBuilder::default().all(true).build();

    let containers = docker
        .list_containers(Some(options))
        .await
        .context("failed to list Docker containers")?;

    Ok(containers
        .into_iter()
        .map(|container| {
            let id = container.id.unwrap_or_default();
            let name = container
                .names
                .and_then(|names| names.into_iter().next())
                .map(|name| name.trim_start_matches('/').to_string())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| short_id(&id));
            let compose = container.labels.as_ref().and_then(compose_metadata);

            ContainerSummary::new_with_compose(
                id,
                name,
                container.image.unwrap_or_else(|| "<none>".to_string()),
                container.status.unwrap_or_else(|| "unknown".to_string()),
                container.state.map(|state| state.to_string()),
                container
                    .health
                    .and_then(|health| health.status)
                    .map(|health| health.to_string())
                    .filter(|health| !health.is_empty() && health != "none"),
                container
                    .ports
                    .unwrap_or_default()
                    .into_iter()
                    .map(|port| {
                        ContainerPortSummary::new(
                            port.private_port,
                            port.public_port,
                            port.typ.map(|typ| typ.to_string()),
                        )
                    })
                    .collect(),
                compose,
                false,
            )
        })
        .collect())
}

fn compose_metadata(labels: &HashMap<String, String>) -> Option<ComposeMetadata> {
    let project = labels
        .get("com.docker.compose.project")
        .filter(|project| !project.is_empty())?
        .clone();
    let service = labels
        .get("com.docker.compose.service")
        .filter(|service| !service.is_empty())
        .cloned();

    Some(ComposeMetadata::new(project, service))
}

pub async fn inspect_container(
    target: ConnectionTarget,
    container_id: &str,
) -> Result<ContainerDetail> {
    let docker = connect(target)?;
    let options = InspectContainerOptionsBuilder::default().size(true).build();
    let response = docker
        .inspect_container(container_id, Some(options))
        .await
        .with_context(|| format!("failed to inspect container {}", short_id(container_id)))?;

    Ok(container_detail_from_inspect(response))
}

pub async fn list_images(target: ConnectionTarget) -> Result<Vec<ImageSummary>> {
    let docker = connect(target)?;
    let options = ListImagesOptionsBuilder::default().all(true).build();

    let images = docker
        .list_images(Some(options))
        .await
        .context("failed to list Docker images")?;

    Ok(images
        .into_iter()
        .map(|image| {
            ImageSummary::new(
                image.id,
                image
                    .repo_tags
                    .into_iter()
                    .filter(|tag| tag != "<none>:<none>")
                    .collect(),
                image.repo_digests,
                image.size.max(0) as u64,
                (image.created > 0).then(|| UNIX_EPOCH + Duration::from_secs(image.created as u64)),
                (image.containers >= 0).then_some(image.containers as u64),
            )
        })
        .collect())
}

pub async fn remove_image(target: ConnectionTarget, image_id: &str) -> Result<()> {
    let docker = connect(target)?;
    let options = RemoveImageOptionsBuilder::default()
        .force(false)
        .noprune(false)
        .build();

    docker
        .remove_image(image_id, Some(options), None)
        .await
        .with_context(|| format!("failed to remove image {}", short_id(image_id)))?;

    Ok(())
}

pub async fn import_image(target: ConnectionTarget, archive_path: PathBuf) -> Result<()> {
    if !archive_path.is_file() {
        anyhow::bail!(
            "Docker image archive does not exist: {}",
            archive_path.display()
        );
    }

    let docker = connect(target)?;
    let file = File::open(&archive_path)
        .await
        .with_context(|| format!("failed to open image archive {}", archive_path.display()))?;
    let stream = ReaderStream::new(file);
    let mut response =
        docker.import_image_stream(ImportImageOptionsBuilder::default().build(), stream, None);

    while let Some(update) = response.next().await {
        update.with_context(|| {
            format!(
                "failed to import Docker image archive {}",
                archive_path.display()
            )
        })?;
    }

    Ok(())
}

pub async fn list_volumes(target: ConnectionTarget) -> Result<Vec<VolumeSummary>> {
    let docker = connect(target.clone())?;
    let options = ListVolumesOptionsBuilder::default().build();

    let response = docker
        .list_volumes(Some(options))
        .await
        .context("failed to list Docker volumes")?;
    let disk_usage = match raw_volume_disk_usage(&target) {
        Some(usage) => usage,
        None => volume_disk_usage(&docker).await.unwrap_or_default(),
    };
    let link_counts = volume_link_counts(&docker).await.unwrap_or_default();

    Ok(response
        .volumes
        .unwrap_or_default()
        .into_iter()
        .map(|volume| {
            let usage_data = volume
                .usage_data
                .as_ref()
                .and_then(|usage| volume_usage_from_fields(usage.size, usage.ref_count))
                .or_else(|| disk_usage.get(&volume.name).copied());
            let ref_count = usage_data
                .and_then(|usage| usage.ref_count)
                .or_else(|| link_counts.get(&volume.name).copied());
            VolumeSummary::new(
                volume.name,
                volume.driver,
                volume.mountpoint,
                usage_data.and_then(|usage| usage.size_bytes),
                volume.created_at.map(system_time_from_offset_datetime),
                ref_count,
            )
        })
        .collect())
}

pub async fn list_networks(target: ConnectionTarget) -> Result<Vec<NetworkSummary>> {
    let docker = connect(target)?;
    let options = ListNetworksOptionsBuilder::default().build();

    let networks = docker
        .list_networks(Some(options))
        .await
        .context("failed to list Docker networks")?;

    let mut summaries = Vec::with_capacity(networks.len());
    for network in networks {
        let inspect = match network.id.as_deref().or(network.name.as_deref()) {
            Some(id) if !id.is_empty() => docker.inspect_network(id, None).await.ok(),
            _ => None,
        };
        summaries.push(network_summary_from_network(network, inspect));
    }

    summaries.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    Ok(summaries)
}

pub async fn create_network(
    target: ConnectionTarget,
    config: NetworkCreateConfig,
) -> Result<String> {
    let docker = connect(target)?;
    let request = network_create_request(config)?;
    let name = request.name.clone();

    let response = docker
        .create_network(request)
        .await
        .with_context(|| format!("failed to create Docker network {}", name))?;

    Ok(response.id)
}

pub async fn remove_network(target: ConnectionTarget, network_id: &str) -> Result<()> {
    let docker = connect(target)?;

    docker
        .remove_network(network_id)
        .await
        .with_context(|| format!("failed to delete Docker network {}", short_id(network_id)))?;

    Ok(())
}

fn network_create_request(config: NetworkCreateConfig) -> Result<NetworkCreateRequest> {
    let name = config.name.trim().to_string();
    if name.is_empty() {
        anyhow::bail!("network name is required");
    }

    let driver = trim_string_to_option(config.driver).unwrap_or_else(|| "bridge".to_string());
    let subnet = trim_option_to_option(config.subnet);
    let gateway = trim_option_to_option(config.gateway);
    let ipam = (subnet.is_some() || gateway.is_some()).then(|| Ipam {
        config: Some(vec![IpamConfig {
            subnet,
            gateway,
            ..Default::default()
        }]),
        ..Default::default()
    });

    Ok(NetworkCreateRequest {
        name,
        driver: Some(driver),
        internal: config.internal.then_some(true),
        ipam,
        enable_ipv6: config.enable_ipv6.then_some(true),
        ..Default::default()
    })
}

fn trim_string_to_option(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn trim_option_to_option(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub async fn remove_volume(target: ConnectionTarget, volume_name: &str) -> Result<()> {
    let docker = connect(target)?;
    let options = RemoveVolumeOptionsBuilder::default().force(false).build();

    docker
        .remove_volume(volume_name, Some(options))
        .await
        .with_context(|| format!("failed to remove volume {}", volume_name))?;

    Ok(())
}

pub async fn import_volume_archive(
    target: ConnectionTarget,
    archive_path: PathBuf,
    volume_name: String,
) -> Result<()> {
    if !archive_path.is_file() {
        anyhow::bail!(
            "Docker volume archive does not exist: {}",
            archive_path.display()
        );
    }
    if volume_name.trim().is_empty() {
        anyhow::bail!("volume name is required");
    }

    let docker = connect(target)?;
    if docker.inspect_volume(&volume_name).await.is_ok() {
        anyhow::bail!("volume {} already exists", volume_name);
    }

    docker
        .create_volume(VolumeCreateRequest {
            name: Some(volume_name.clone()),
            ..Default::default()
        })
        .await
        .with_context(|| format!("failed to create Docker volume {}", volume_name))?;

    let helper_image = match select_volume_import_helper_image(&docker).await {
        Ok(helper_image) => helper_image,
        Err(error) => {
            cleanup_import_volume(&docker, &volume_name).await;
            return Err(error);
        }
    };

    let container_name = import_helper_container_name(&volume_name);
    let container = match docker
        .create_container(
            Some(
                CreateContainerOptionsBuilder::default()
                    .name(&container_name)
                    .build(),
            ),
            ContainerCreateBody {
                image: Some(helper_image),
                cmd: Some(vec!["/bin/true".to_string()]),
                host_config: Some(HostConfig {
                    mounts: Some(vec![Mount {
                        typ: Some(MountType::VOLUME),
                        source: Some(volume_name.clone()),
                        target: Some(VOLUME_IMPORT_MOUNT_PATH.to_string()),
                        ..Default::default()
                    }]),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
    {
        Ok(container) => container,
        Err(error) => {
            cleanup_import_volume(&docker, &volume_name).await;
            return Err(error).with_context(|| {
                format!("failed to create volume import helper for {}", volume_name)
            });
        }
    };

    let upload_result =
        upload_volume_archive_to_container(&docker, &container.id, &archive_path).await;
    cleanup_import_container(&docker, &container.id).await;

    if let Err(error) = upload_result {
        cleanup_import_volume(&docker, &volume_name).await;
        return Err(error);
    }

    Ok(())
}

const VOLUME_IMPORT_MOUNT_PATH: &str = "/echo-volume-import";

async fn upload_volume_archive_to_container(
    docker: &Docker,
    container_id: &str,
    archive_path: &Path,
) -> Result<()> {
    let prepared_archive = prepare_volume_archive(archive_path).await?;
    let file = File::open(&prepared_archive.path)
        .await
        .with_context(|| format!("failed to open volume archive {}", archive_path.display()))?;
    let stream = ReaderStream::new(file);
    let options = UploadToContainerOptionsBuilder::default()
        .path(VOLUME_IMPORT_MOUNT_PATH)
        .no_overwrite_dir_non_dir("true")
        .build();

    let result = docker
        .upload_to_container(container_id, Some(options), body_try_stream(stream))
        .await
        .with_context(|| format!("failed to import volume archive {}", archive_path.display()));

    prepared_archive.cleanup();
    result
}

struct PreparedArchive {
    path: PathBuf,
    remove_after_upload: bool,
}

impl PreparedArchive {
    fn cleanup(&self) {
        if self.remove_after_upload {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

async fn prepare_volume_archive(path: &Path) -> Result<PreparedArchive> {
    let source = path.to_path_buf();
    let target = temp_volume_archive_path();
    let target_for_task = target.clone();

    tokio::task::spawn_blocking(move || -> Result<()> {
        let input_file = std::fs::File::open(&source)
            .with_context(|| format!("failed to open volume archive {}", source.display()))?;
        let mut input: Box<dyn Read> = if is_gzip_archive(&source) {
            Box::new(GzDecoder::new(input_file))
        } else {
            Box::new(input_file)
        };
        let mut output = std::fs::File::create(&target_for_task).with_context(|| {
            format!(
                "failed to create temporary volume archive {}",
                target_for_task.display()
            )
        })?;
        copy_tar_without_pax_extensions(&mut input, &mut output)
            .with_context(|| format!("failed to prepare volume archive {}", source.display()))?;
        Ok(())
    })
    .await
    .context("failed to prepare volume archive")??;

    Ok(PreparedArchive {
        path: target,
        remove_after_upload: true,
    })
}

fn copy_tar_without_pax_extensions(input: &mut dyn Read, output: &mut dyn Write) -> Result<()> {
    loop {
        let mut header = [0u8; 512];
        input
            .read_exact(&mut header)
            .context("failed to read tar header")?;

        if header.iter().all(|byte| *byte == 0) {
            output.write_all(&[0; 1024])?;
            return Ok(());
        }

        let size = tar_header_size(&header)?;
        let padded_size = size.div_ceil(512) * 512;
        let is_pax_extension = matches!(header[156], b'x' | b'g');

        if !is_pax_extension {
            output.write_all(&header)?;
        }
        copy_tar_entry_bytes(input, output, padded_size, is_pax_extension)?;
    }
}

fn copy_tar_entry_bytes(
    input: &mut dyn Read,
    output: &mut dyn Write,
    mut remaining: u64,
    discard: bool,
) -> Result<()> {
    let mut buffer = [0u8; 8192];
    while remaining > 0 {
        let length = buffer.len().min(remaining as usize);
        input
            .read_exact(&mut buffer[..length])
            .context("failed to read tar entry")?;
        if !discard {
            output.write_all(&buffer[..length])?;
        }
        remaining -= length as u64;
    }
    Ok(())
}

fn tar_header_size(header: &[u8; 512]) -> Result<u64> {
    let field = &header[124..136];
    let text = field
        .iter()
        .copied()
        .take_while(|byte| *byte != 0 && *byte != b' ')
        .map(char::from)
        .collect::<String>();
    if text.trim().is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(text.trim(), 8).context("failed to parse tar entry size")
}

fn temp_volume_archive_path() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    env::temp_dir().join(format!(
        "echo-volume-import-{}-{nanos}.tar",
        std::process::id()
    ))
}

fn is_gzip_archive(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let name = name.to_lowercase();
    name.ends_with(".tar.gz") || name.ends_with(".tgz")
}

async fn select_volume_import_helper_image(docker: &Docker) -> Result<String> {
    let options = ListImagesOptionsBuilder::default().all(false).build();
    let images = docker
        .list_images(Some(options))
        .await
        .context("failed to list Docker images for volume import helper")?;

    let mut first_tag = None;
    for image in images {
        for tag in image.repo_tags {
            if tag == "<none>:<none>" {
                continue;
            }
            if tag == "alpine:latest" {
                return Ok(tag);
            }
            if first_tag.is_none() {
                first_tag = Some(tag);
            }
        }
    }

    first_tag.ok_or_else(|| {
        anyhow::anyhow!("volume import requires at least one local Docker image for staging")
    })
}

fn import_helper_container_name(volume_name: &str) -> String {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!(
        "echo-volume-import-{}-{suffix}",
        sanitize_docker_name(volume_name)
    )
}

async fn cleanup_import_container(docker: &Docker, container_id: &str) {
    let options = RemoveContainerOptionsBuilder::default()
        .force(true)
        .v(false)
        .build();
    let _ = docker.remove_container(container_id, Some(options)).await;
}

async fn cleanup_import_volume(docker: &Docker, volume_name: &str) {
    let options = RemoveVolumeOptionsBuilder::default().force(true).build();
    let _ = docker.remove_volume(volume_name, Some(options)).await;
}

pub(crate) fn volume_name_from_archive_path(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("imported-volume");
    let lower = name.to_lowercase();
    let stem = if lower.ends_with(".tar.gz") {
        &name[..name.len().saturating_sub(7)]
    } else if lower.ends_with(".tgz") || lower.ends_with(".tar") {
        &name[..name.len().saturating_sub(4)]
    } else {
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(name)
    };

    sanitize_docker_name(stem)
}

fn sanitize_docker_name(value: &str) -> String {
    let mut sanitized = String::new();
    let mut previous_was_separator = false;

    for ch in value.chars() {
        let valid = ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '-';
        let next = if valid { ch } else { '_' };
        if next == '_' && previous_was_separator {
            continue;
        }
        previous_was_separator = next == '_';
        sanitized.push(next);
    }

    let sanitized = sanitized.trim_matches(['_', '.', '-']).to_string();
    if sanitized.is_empty() {
        "imported-volume".to_string()
    } else if sanitized
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphanumeric())
    {
        sanitized
    } else {
        format!("volume_{sanitized}")
    }
}

fn network_summary_from_network(
    network: Network,
    inspect: Option<NetworkInspect>,
) -> NetworkSummary {
    let id = inspect
        .as_ref()
        .and_then(|network| network.id.clone())
        .or(network.id)
        .unwrap_or_default();
    let name = inspect
        .as_ref()
        .and_then(|network| network.name.clone())
        .or(network.name)
        .unwrap_or_else(|| short_id(&id));
    let driver = inspect
        .as_ref()
        .and_then(|network| network.driver.clone())
        .or(network.driver)
        .unwrap_or_else(|| "unknown".to_string());
    let scope = inspect
        .as_ref()
        .and_then(|network| network.scope.clone())
        .or(network.scope);
    let ipam = inspect
        .as_ref()
        .and_then(|network| network.ipam.as_ref())
        .or(network.ipam.as_ref());
    let (subnet, gateway) = network_ipam_summary(ipam);
    let ipv6_enabled = inspect
        .as_ref()
        .and_then(|network| network.enable_ipv6)
        .or(network.enable_ipv6)
        .unwrap_or(false);
    let created_at = inspect
        .as_ref()
        .and_then(|network| network.created)
        .or(network.created)
        .map(system_time_from_offset_datetime);
    let labels = inspect
        .as_ref()
        .and_then(|network| network.labels.clone())
        .or(network.labels)
        .map(network_labels_from_map)
        .unwrap_or_default();
    let endpoints = inspect
        .and_then(|network| network.containers)
        .map(network_endpoints_from_map)
        .unwrap_or_default();

    NetworkSummary::new(
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
    )
}

fn network_ipam_summary(ipam: Option<&Ipam>) -> (Option<String>, Option<String>) {
    let Some(config) = ipam.and_then(|ipam| ipam.config.as_ref()) else {
        return (None, None);
    };
    let subnet = config.iter().find_map(|config| {
        config
            .subnet
            .as_deref()
            .filter(|subnet| !subnet.is_empty())
            .map(str::to_string)
    });
    let gateway = config.iter().find_map(|config| {
        config
            .gateway
            .as_deref()
            .filter(|gateway| !gateway.is_empty())
            .map(str::to_string)
    });

    (subnet, gateway)
}

fn network_labels_from_map(labels: HashMap<String, String>) -> Vec<NetworkLabelSummary> {
    let mut labels = labels
        .into_iter()
        .filter(|(key, _)| !key.is_empty())
        .map(|(key, value)| NetworkLabelSummary::new(key, value))
        .collect::<Vec<_>>();
    labels.sort_by(|a, b| a.key.cmp(&b.key));
    labels
}

fn network_endpoints_from_map(
    endpoints: HashMap<String, EndpointResource>,
) -> Vec<NetworkEndpointSummary> {
    let mut endpoints = endpoints
        .into_iter()
        .map(|(container_id, endpoint)| {
            let name = endpoint
                .name
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| short_id(&container_id));
            NetworkEndpointSummary::new(
                container_id,
                name,
                endpoint.endpoint_id.filter(|id| !id.is_empty()),
                endpoint.mac_address.filter(|mac| !mac.is_empty()),
                endpoint.ipv4_address.filter(|ip| !ip.is_empty()),
                endpoint.ipv6_address.filter(|ip| !ip.is_empty()),
            )
        })
        .collect::<Vec<_>>();
    endpoints.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.container_id.cmp(&b.container_id))
    });
    endpoints
}

#[derive(Clone, Copy, Debug, Default)]
struct VolumeUsage {
    size_bytes: Option<u64>,
    ref_count: Option<u64>,
}

async fn volume_link_counts(docker: &Docker) -> Result<HashMap<String, u64>> {
    let options = ListContainersOptionsBuilder::default().all(true).build();
    let containers = docker
        .list_containers(Some(options))
        .await
        .context("failed to list Docker containers for volume usage")?;

    let mut counts = HashMap::new();
    for mount in containers
        .into_iter()
        .flat_map(|container| container.mounts.unwrap_or_default())
    {
        if mount.typ.as_deref() != Some("volume") {
            continue;
        }
        let Some(name) = mount.name.filter(|name| !name.is_empty()) else {
            continue;
        };
        *counts.entry(name).or_insert(0) += 1;
    }

    Ok(counts)
}

async fn volume_disk_usage(docker: &Docker) -> Result<HashMap<String, VolumeUsage>> {
    let response = docker
        .df(None::<DataUsageOptions>)
        .await
        .context("failed to fetch Docker disk usage")?;

    Ok(response
        .volume_usage
        .and_then(|usage| usage.items)
        .unwrap_or_default()
        .into_iter()
        .filter_map(volume_usage_from_value)
        .collect())
}

#[cfg(unix)]
fn raw_volume_disk_usage(target: &ConnectionTarget) -> Option<HashMap<String, VolumeUsage>> {
    let socket_path = match target {
        ConnectionTarget::DockerHost(host) => host.strip_prefix("unix://")?.to_string(),
        ConnectionTarget::LocalSocket(path) => path.to_str()?.to_string(),
        ConnectionTarget::DefaultContext => default_unix_socket_path()?,
        ConnectionTarget::Ssh { .. } | ConnectionTarget::BoxLite => return None,
    };

    let mut stream = UnixStream::connect(&socket_path).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
    stream
        .write_all(b"GET /system/df HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .ok()?;

    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    let (headers, body) = response.split_once("\r\n\r\n")?;
    let body = if headers
        .lines()
        .any(|line| line.eq_ignore_ascii_case("Transfer-Encoding: chunked"))
    {
        decode_chunked_body(body)?
    } else {
        body.to_string()
    };
    let value = serde_json::from_str::<Value>(&body).ok()?;

    volume_usage_from_system_df_value(value)
}

#[cfg(not(unix))]
fn raw_volume_disk_usage(_target: &ConnectionTarget) -> Option<HashMap<String, VolumeUsage>> {
    None
}

#[cfg(unix)]
fn default_unix_socket_path() -> Option<String> {
    if let Ok(host) = env::var("DOCKER_HOST")
        && let Some(path) = host.strip_prefix("unix://")
    {
        return Some(path.to_string());
    }

    let context_host = Command::new("docker")
        .args([
            "context",
            "inspect",
            "--format",
            "{{.Endpoints.docker.Host}}",
        ])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|output| output.trim().to_string())
        .filter(|host| !host.is_empty() && host != "<no value>");

    if let Some(context_host) = context_host
        && let Some(path) = context_host.strip_prefix("unix://")
    {
        return Some(path.to_string());
    }

    Path::new("/var/run/docker.sock")
        .exists()
        .then(|| "/var/run/docker.sock".to_string())
}

fn volume_usage_from_value(value: Value) -> Option<(String, VolumeUsage)> {
    if let Ok(volume) = serde_json::from_value::<Volume>(value.clone()) {
        let usage_data = volume.usage_data?;
        return volume_usage_from_fields(usage_data.size, usage_data.ref_count)
            .map(|usage| (volume.name, usage));
    }

    let name = value
        .get("Name")
        .or_else(|| value.get("name"))
        .and_then(Value::as_str)?
        .to_string();
    let usage_data = value.get("UsageData").or_else(|| value.get("usage_data"))?;
    let size = usage_data
        .get("Size")
        .or_else(|| usage_data.get("size"))
        .and_then(Value::as_i64)
        .unwrap_or(-1);
    let ref_count = usage_data
        .get("RefCount")
        .or_else(|| usage_data.get("ref_count"))
        .and_then(Value::as_i64)
        .unwrap_or(-1);

    volume_usage_from_fields(size, ref_count).map(|usage| (name, usage))
}

fn volume_usage_from_system_df_value(value: Value) -> Option<HashMap<String, VolumeUsage>> {
    value
        .get("Volumes")
        .and_then(Value::as_array)
        .or_else(|| {
            value
                .get("VolumeUsage")
                .and_then(|usage| usage.get("Items"))
                .and_then(Value::as_array)
        })
        .map(|volumes| {
            volumes
                .iter()
                .cloned()
                .filter_map(volume_usage_from_value)
                .collect()
        })
}

fn decode_chunked_body(body: &str) -> Option<String> {
    let mut decoded = String::new();
    let mut rest = body;

    loop {
        let (size_hex, after_size) = rest.split_once("\r\n")?;
        let size = usize::from_str_radix(size_hex.trim(), 16).ok()?;
        if size == 0 {
            break;
        }
        if after_size.len() < size + 2 {
            return None;
        }
        decoded.push_str(&after_size[..size]);
        rest = after_size.get(size + 2..)?;
    }

    Some(decoded)
}

fn volume_usage_from_fields(size: i64, ref_count: i64) -> Option<VolumeUsage> {
    let usage = VolumeUsage {
        size_bytes: non_negative_i64(size),
        ref_count: non_negative_i64(ref_count),
    };
    (usage.size_bytes.is_some() || usage.ref_count.is_some()).then_some(usage)
}

pub fn stream_container_stats(
    target: ConnectionTarget,
    container_id: &str,
) -> Result<ContainerStatsStream> {
    let docker = connect(target)?;
    let options = StatsOptionsBuilder::default().stream(true).build();
    let stream = docker.stats(container_id, Some(options));

    Ok(ContainerStatsStream {
        inner: Box::pin(stream),
        previous: None,
    })
}

pub fn stream_container_logs(
    target: ConnectionTarget,
    container_id: &str,
) -> Result<ContainerLogsStream> {
    let docker = connect(target)?;
    let options = LogsOptionsBuilder::default()
        .follow(true)
        .stdout(true)
        .stderr(true)
        .timestamps(false)
        .tail("200")
        .build();
    let stream = docker.logs(container_id, Some(options));

    Ok(ContainerLogsStream {
        inner: Box::pin(stream),
    })
}

pub async fn start_container_shell(
    target: ConnectionTarget,
    container_id: &str,
    cols: u16,
    rows: u16,
) -> Result<ContainerShellExec> {
    let docker = connect(target)?;
    let shell = detect_container_shell(&docker, container_id).await?;
    let config = bollard::models::ExecConfig {
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        attach_stdin: Some(true),
        tty: Some(true),
        env: Some(vec![
            "TERM=xterm-256color".to_string(),
            "COLORTERM=truecolor".to_string(),
        ]),
        cmd: Some(vec![shell]),
        ..Default::default()
    };
    let exec_id = docker
        .create_exec(container_id, config)
        .await
        .with_context(|| {
            format!(
                "failed to create shell for container {}",
                short_id(container_id)
            )
        })?
        .id;

    let result = docker
        .start_exec(
            &exec_id,
            Some(StartExecOptions {
                tty: true,
                output_capacity: Some(32 * 1024),
                ..Default::default()
            }),
        )
        .await
        .with_context(|| {
            format!(
                "failed to start shell for container {}",
                short_id(container_id)
            )
        })?;

    docker
        .resize_exec(
            &exec_id,
            bollard::query_parameters::ResizeExecOptionsBuilder::default()
                .w(cols.max(1) as i32)
                .h(rows.max(1) as i32)
                .build(),
        )
        .await
        .ok();

    match result {
        StartExecResults::Attached { output, input } => Ok(ContainerShellExec {
            exec_id,
            output,
            input,
        }),
        StartExecResults::Detached => anyhow::bail!("Docker shell exec detached unexpectedly"),
    }
}

pub async fn resize_container_shell(
    target: ConnectionTarget,
    exec_id: &str,
    cols: u16,
    rows: u16,
) -> Result<()> {
    let docker = connect(target)?;
    docker
        .resize_exec(
            exec_id,
            bollard::query_parameters::ResizeExecOptionsBuilder::default()
                .w(cols.max(1) as i32)
                .h(rows.max(1) as i32)
                .build(),
        )
        .await
        .context("failed to resize container shell")
}

async fn detect_container_shell(docker: &Docker, container_id: &str) -> Result<String> {
    const SHELL_CANDIDATES: &[&str] = &[
        "/bin/bash",
        "/usr/bin/bash",
        "/bin/sh",
        "/usr/bin/sh",
        "/bin/ash",
        "/usr/bin/ash",
        "bash",
        "sh",
        "ash",
    ];

    for shell in SHELL_CANDIDATES {
        let config = bollard::models::ExecConfig {
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            cmd: Some(vec![
                shell.to_string(),
                "-c".to_string(),
                "exit 0".to_string(),
            ]),
            ..Default::default()
        };
        let Ok(exec) = docker.create_exec(container_id, config).await else {
            continue;
        };
        let exec_id = exec.id;

        let Ok(result) = docker.start_exec(&exec_id, None::<StartExecOptions>).await else {
            continue;
        };

        match result {
            StartExecResults::Attached { mut output, .. } => {
                while let Some(_chunk) = output.next().await {}
            }
            StartExecResults::Detached => {}
        }

        let Ok(inspect) = docker.inspect_exec(&exec_id).await else {
            continue;
        };

        if inspect.exit_code == Some(0) {
            return Ok(shell.to_string());
        }
    }

    anyhow::bail!("container does not provide bash, sh, or ash")
}

pub async fn control_container(
    target: ConnectionTarget,
    container_id: &str,
    action: ContainerAction,
) -> Result<()> {
    let docker = connect(target)?;

    match action {
        ContainerAction::Start => docker
            .start_container(container_id, None)
            .await
            .with_context(|| format!("failed to start container {}", short_id(container_id))),
        ContainerAction::Stop => {
            let options = StopContainerOptionsBuilder::default().build();
            docker
                .stop_container(container_id, Some(options))
                .await
                .with_context(|| format!("failed to stop container {}", short_id(container_id)))
        }
        ContainerAction::Restart => {
            let options = RestartContainerOptionsBuilder::default().build();
            docker
                .restart_container(container_id, Some(options))
                .await
                .with_context(|| format!("failed to restart container {}", short_id(container_id)))
        }
        ContainerAction::Pause => docker
            .pause_container(container_id)
            .await
            .with_context(|| format!("failed to pause container {}", short_id(container_id))),
        ContainerAction::Unpause => docker
            .unpause_container(container_id)
            .await
            .with_context(|| format!("failed to resume container {}", short_id(container_id))),
        ContainerAction::Remove => {
            let options = RemoveContainerOptionsBuilder::default()
                .v(false)
                .force(false)
                .build();
            docker
                .remove_container(container_id, Some(options))
                .await
                .with_context(|| format!("failed to remove container {}", short_id(container_id)))
        }
    }
}

pub(crate) fn connect(target: ConnectionTarget) -> Result<Docker> {
    match target {
        ConnectionTarget::DefaultContext => {
            Docker::connect_with_defaults().context("failed to connect to the default Docker host")
        }
        ConnectionTarget::DockerHost(host) => Docker::connect_with_host(&host)
            .with_context(|| format!("failed to connect to Docker host {}", host)),
        ConnectionTarget::LocalSocket(path) => {
            let host = format!("unix://{}", path.display());
            Docker::connect_with_host(&host)
                .with_context(|| format!("failed to connect to Docker socket {}", path.display()))
        }
        ConnectionTarget::Ssh { .. } | ConnectionTarget::BoxLite => {
            anyhow::bail!("unsupported local Docker connection target")
        }
    }
}

fn short_id(id: &str) -> String {
    id.chars().take(12).collect()
}

impl Stream for ContainerStatsStream {
    type Item = Result<ContainerRuntimeStats>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(raw))) => {
                let counters = StatsCounters::from_response(&raw);
                let stats = container_runtime_stats_from_response(&raw, counters, self.previous);
                self.previous = Some(counters);
                Poll::Ready(Some(stats))
            }
            Poll::Ready(Some(Err(error))) => Poll::Ready(Some(Err(error.into()))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Stream for ContainerLogsStream {
    type Item = Result<ContainerLogLine>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(raw))) => {
                Poll::Ready(Some(Ok(container_log_line_from_output(raw))))
            }
            Poll::Ready(Some(Err(error))) => Poll::Ready(Some(Err(error.into()))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[derive(Clone, Copy)]
struct StatsCounters {
    sample_time: SystemTime,
    cpu_total: u64,
    system_cpu_total: u64,
    online_cpus: Option<u32>,
    memory_usage: Option<u64>,
    memory_limit: Option<u64>,
    network_rx: u64,
    network_tx: u64,
    disk_read: u64,
    disk_write: u64,
}

fn container_detail_from_inspect(response: ContainerInspectResponse) -> ContainerDetail {
    let config = response.config;
    let host_config = response.host_config;
    let network_settings = response.network_settings;

    let ports = network_settings
        .as_ref()
        .and_then(|settings| settings.ports.as_ref())
        .map(port_summaries_from_map)
        .unwrap_or_default();

    let mounts = response
        .mounts
        .unwrap_or_default()
        .into_iter()
        .filter_map(|mount| {
            let destination = mount.destination?;
            Some(ContainerMountSummary::new(
                mount.name.or(mount.source),
                destination,
                mount.typ,
                !mount.rw.unwrap_or(true),
            ))
        })
        .collect();

    let environment = config
        .as_ref()
        .and_then(|config| config.env.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| !entry.is_empty())
        .collect();

    let mut labels = config
        .as_ref()
        .and_then(|config| config.labels.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|(key, value)| ContainerLabelSummary::new(key, value))
        .collect::<Vec<_>>();
    labels.sort_by(|left, right| left.key.cmp(&right.key));

    let restart_policy = host_config
        .as_ref()
        .and_then(|host_config| host_config.restart_policy.as_ref())
        .and_then(|policy| policy.name.as_ref())
        .map(ToString::to_string)
        .filter(|policy| !policy.is_empty());

    ContainerDetail {
        id: response.id.unwrap_or_default(),
        image: config
            .as_ref()
            .and_then(|config| config.image.clone())
            .or(response.image)
            .unwrap_or_else(|| "<none>".to_string()),
        created_at: response.created.map(|date| {
            date.format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_else(|_| date.to_string())
        }),
        started_at: response.state.and_then(|state| state.started_at),
        restart_count: response.restart_count.unwrap_or_default().max(0) as u64,
        restart_policy,
        user: config
            .as_ref()
            .and_then(|config| non_empty(config.user.clone())),
        working_dir: config
            .as_ref()
            .and_then(|config| non_empty(config.working_dir.clone())),
        entrypoint: config
            .as_ref()
            .and_then(|config| config.entrypoint.clone())
            .unwrap_or_default(),
        command: config
            .as_ref()
            .and_then(|config| config.cmd.clone())
            .unwrap_or_default(),
        ports,
        mounts,
        environment,
        labels,
        size_rw_bytes: response.size_rw.and_then(non_negative_i64),
        size_root_fs_bytes: response.size_root_fs.and_then(non_negative_i64),
    }
}

fn container_runtime_stats_from_response(
    _response: &ContainerStatsResponse,
    counters: StatsCounters,
    previous: Option<StatsCounters>,
) -> Result<ContainerRuntimeStats> {
    let (
        network_rx_bytes_per_sec,
        network_tx_bytes_per_sec,
        disk_read_bytes_per_sec,
        disk_write_bytes_per_sec,
    ) = previous.map_or((0., 0., 0., 0.), |previous| {
        let elapsed_secs = counters
            .sample_time
            .duration_since(previous.sample_time)
            .map(|elapsed| elapsed.as_secs_f64())
            .unwrap_or_default();
        (
            rate(previous.network_rx, counters.network_rx, elapsed_secs),
            rate(previous.network_tx, counters.network_tx, elapsed_secs),
            rate(previous.disk_read, counters.disk_read, elapsed_secs),
            rate(previous.disk_write, counters.disk_write, elapsed_secs),
        )
    });

    Ok(ContainerRuntimeStats {
        sample_time: counters.sample_time,
        cpu_percent: cpu_percent(counters, previous),
        online_cpus: counters.online_cpus,
        memory_usage_bytes: counters.memory_usage,
        memory_limit_bytes: counters.memory_limit,
        network_rx_bytes_per_sec,
        network_tx_bytes_per_sec,
        disk_read_bytes_per_sec,
        disk_write_bytes_per_sec,
    })
}

impl StatsCounters {
    fn from_response(response: &ContainerStatsResponse) -> Self {
        let sample_time = response
            .read
            .map(system_time_from_offset_datetime)
            .unwrap_or_else(SystemTime::now);
        let cpu_stats = response.cpu_stats.as_ref();
        let cpu_total = cpu_stats
            .and_then(|stats| stats.cpu_usage.as_ref())
            .and_then(|usage| usage.total_usage)
            .unwrap_or_default();
        let system_cpu_total = cpu_stats
            .and_then(|stats| stats.system_cpu_usage)
            .unwrap_or_default();
        let online_cpus = cpu_stats
            .and_then(|stats| stats.online_cpus)
            .filter(|cpus| *cpus > 0)
            .or_else(|| {
                cpu_stats
                    .and_then(|stats| stats.cpu_usage.as_ref())
                    .and_then(|usage| usage.percpu_usage.as_ref())
                    .map(|cpus| cpus.len() as u32)
                    .filter(|cpus| *cpus > 0)
            });
        let memory_stats = response.memory_stats.as_ref();
        let memory_usage = memory_stats.map(memory_usage_bytes);
        let memory_limit = memory_stats
            .and_then(|stats| stats.limit)
            .filter(|limit| *limit > 0);
        let (network_rx, network_tx) = response
            .networks
            .as_ref()
            .map(|networks| {
                networks.values().fold((0, 0), |(rx, tx), stats| {
                    (
                        rx + stats.rx_bytes.unwrap_or_default(),
                        tx + stats.tx_bytes.unwrap_or_default(),
                    )
                })
            })
            .unwrap_or_default();
        let (disk_read, disk_write) = response
            .blkio_stats
            .as_ref()
            .and_then(|stats| stats.io_service_bytes_recursive.as_ref())
            .map(|entries| {
                entries.iter().fold((0, 0), |(read, write), entry| {
                    match entry.op.as_deref().map(str::to_ascii_lowercase).as_deref() {
                        Some("read") => (read + entry.value.unwrap_or_default(), write),
                        Some("write") => (read, write + entry.value.unwrap_or_default()),
                        _ => (read, write),
                    }
                })
            })
            .unwrap_or_default();

        Self {
            sample_time,
            cpu_total,
            system_cpu_total,
            online_cpus,
            memory_usage,
            memory_limit,
            network_rx,
            network_tx,
            disk_read,
            disk_write,
        }
    }
}

fn system_time_from_offset_datetime(date: time::OffsetDateTime) -> SystemTime {
    let seconds = date.unix_timestamp();
    let nanos = date.nanosecond();

    if seconds >= 0 {
        UNIX_EPOCH + Duration::new(seconds as u64, nanos)
    } else {
        UNIX_EPOCH - Duration::new(seconds.unsigned_abs(), nanos)
    }
}

fn container_log_line_from_output(output: LogOutput) -> ContainerLogLine {
    let stream = match &output {
        LogOutput::StdOut { .. } => ContainerLogStreamKind::Stdout,
        LogOutput::StdErr { .. } => ContainerLogStreamKind::Stderr,
        LogOutput::StdIn { .. } => ContainerLogStreamKind::Stdin,
        LogOutput::Console { .. } => ContainerLogStreamKind::Console,
    };
    let raw = sanitize_terminal_log_text(&String::from_utf8_lossy(output.as_ref()))
        .trim_end()
        .to_string();

    ContainerLogLine::new(None, stream, raw)
}

fn sanitize_terminal_log_text(raw: &str) -> String {
    let normalized = normalize_terminal_line_endings(raw);
    remove_non_text_controls(&strip_ansi_escapes::strip_str(&normalized))
}

fn normalize_terminal_line_endings(raw: &str) -> String {
    let mut text = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    continue;
                }
                text.push('\n');
            }
            ch => text.push(ch),
        }
    }

    text
}

fn remove_non_text_controls(raw: &str) -> String {
    let mut text = String::with_capacity(raw.len());

    for ch in raw.chars() {
        match ch {
            '\n' | '\t' => text.push(ch),
            ch if ch.is_control() => {}
            ch => text.push(ch),
        }
    }

    text
}

fn cpu_percent(counters: StatsCounters, previous: Option<StatsCounters>) -> f64 {
    let Some(previous) = previous else {
        return 0.;
    };

    let cpu_delta = counters.cpu_total.saturating_sub(previous.cpu_total) as f64;
    let system_delta = counters
        .system_cpu_total
        .saturating_sub(previous.system_cpu_total) as f64;
    let online_cpus = counters.online_cpus.unwrap_or(1).max(1) as f64;

    if cpu_delta <= 0. || system_delta <= 0. {
        return 0.;
    }

    (cpu_delta / system_delta) * online_cpus * 100.
}

fn memory_usage_bytes(stats: &bollard::models::ContainerMemoryStats) -> u64 {
    let usage = stats.usage.unwrap_or_default();
    let inactive_file = stats
        .stats
        .as_ref()
        .and_then(|stats| {
            stats
                .get("inactive_file")
                .or_else(|| stats.get("total_inactive_file"))
                .or_else(|| stats.get("cache"))
        })
        .copied()
        .unwrap_or_default();

    usage.saturating_sub(inactive_file)
}

fn rate(previous: u64, current: u64, elapsed_secs: f64) -> f64 {
    if elapsed_secs <= 0. {
        return 0.;
    }

    current.saturating_sub(previous) as f64 / elapsed_secs
}

fn port_summaries_from_map(
    ports: &HashMap<String, Option<Vec<PortBinding>>>,
) -> Vec<ContainerPortSummary> {
    let mut summaries = Vec::new();

    for (container_port, bindings) in ports {
        let Some((private_port, protocol)) = parse_container_port(container_port) else {
            continue;
        };

        if let Some(bindings) = bindings {
            for binding in bindings {
                summaries.push(ContainerPortSummary::new(
                    private_port,
                    binding
                        .host_port
                        .as_deref()
                        .and_then(|port| port.parse::<u16>().ok()),
                    Some(protocol.clone()),
                ));
            }
        } else {
            summaries.push(ContainerPortSummary::new(
                private_port,
                None,
                Some(protocol.clone()),
            ));
        }
    }

    summaries.sort_by_key(|port| (port.private_port, port.public_port));
    summaries
}

fn parse_container_port(port: &str) -> Option<(u16, String)> {
    let (private_port, protocol) = port.split_once('/')?;
    Some((private_port.parse().ok()?, protocol.to_string()))
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        if value.is_empty() { None } else { Some(value) }
    })
}

fn non_negative_i64(value: i64) -> Option<u64> {
    u64::try_from(value).ok()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        process::{Command, Output},
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use bollard::models::{
        ContainerConfig, ContainerInspectResponse, ContainerMemoryStats, EndpointResource, Ipam,
        IpamConfig, Network, NetworkInspect,
    };

    use bollard::container::LogOutput;

    use crate::{
        bridge::{NetworkCreateConfig, resolver::resolve_current_target},
        domain::ContainerLogStreamKind,
    };

    use super::{
        StatsCounters, container_detail_from_inspect, container_log_line_from_output, cpu_percent,
        decode_chunked_body, import_image, import_volume_archive, is_gzip_archive,
        memory_usage_bytes, network_create_request, network_summary_from_network, rate,
        sanitize_terminal_log_text, short_id, volume_name_from_archive_path,
        volume_usage_from_system_df_value, volume_usage_from_value,
    };

    #[test]
    fn shortens_container_ids() {
        assert_eq!(short_id("1234567890abcdef"), "1234567890ab");
    }

    #[test]
    fn preserves_full_container_environment_from_inspect() {
        let detail = container_detail_from_inspect(ContainerInspectResponse {
            config: Some(ContainerConfig {
                env: Some(vec![
                    "DATABASE_URL=postgres://localhost/echo".to_string(),
                    "EMPTY=".to_string(),
                    String::new(),
                ]),
                ..Default::default()
            }),
            ..Default::default()
        });

        assert_eq!(
            detail.environment,
            vec![
                "DATABASE_URL=postgres://localhost/echo".to_string(),
                "EMPTY=".to_string()
            ]
        );
    }

    #[test]
    fn derives_volume_name_from_archive_path() {
        assert_eq!(
            volume_name_from_archive_path(Path::new("/tmp/db-data.tar.gz")),
            "db-data"
        );
        assert_eq!(
            volume_name_from_archive_path(Path::new("/tmp/my volume.tgz")),
            "my_volume"
        );
        assert_eq!(
            volume_name_from_archive_path(Path::new("/tmp/---.tar")),
            "imported-volume"
        );
    }

    #[test]
    fn detects_gzip_volume_archives() {
        assert!(is_gzip_archive(Path::new("volume.tar.gz")));
        assert!(is_gzip_archive(Path::new("volume.tgz")));
        assert!(!is_gzip_archive(Path::new("volume.tar")));
    }

    fn unique_test_suffix() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string()
    }

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    fn run_docker<const N: usize>(args: [&str; N]) -> Output {
        run_command("docker", args)
    }

    fn run_command<const N: usize>(program: &str, args: [&str; N]) -> Output {
        let output = Command::new(program).args(args).output().unwrap();
        assert!(
            output.status.success(),
            "{} failed: {}\n{}",
            program,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    #[tokio::test]
    #[ignore = "requires a local Docker daemon and mutates test images"]
    async fn imports_image_archive_from_local_file() {
        let tag = format!("echo-import-test:{}", unique_test_suffix());
        let archive = temp_path(&format!("{}-image.tar", tag.replace(':', "-")));

        run_docker(["tag", "hello-world:latest", &tag]);
        run_docker(["save", "-o", archive.to_str().unwrap(), &tag]);
        run_docker(["image", "rm", "-f", &tag]);

        import_image(resolve_current_target(), archive.clone())
            .await
            .unwrap();

        let output = run_docker(["image", "ls", "--format", "{{.Repository}}:{{.Tag}}"]);
        let images = String::from_utf8_lossy(&output.stdout);
        assert!(images.lines().any(|line| line == tag));

        run_docker(["image", "rm", "-f", &tag]);
        let _ = fs::remove_file(archive);
    }

    #[tokio::test]
    #[ignore = "requires a local Docker daemon and mutates test volumes"]
    async fn imports_volume_archive_from_local_file() {
        let suffix = unique_test_suffix();
        let volume_name = format!("echo-import-volume-{suffix}");
        let dir = temp_path(&format!("echo-volume-src-{suffix}"));
        let archive = temp_path(&format!("echo-volume-src-{suffix}.tar.gz"));

        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("hello.txt"), "hello from echo\n").unwrap();
        run_command(
            "tar",
            [
                "-czf",
                archive.to_str().unwrap(),
                "-C",
                dir.to_str().unwrap(),
                ".",
            ],
        );

        import_volume_archive(
            resolve_current_target(),
            archive.clone(),
            volume_name.clone(),
        )
        .await
        .unwrap();

        let output = run_docker([
            "run",
            "--rm",
            "-v",
            &format!("{volume_name}:/data"),
            "alpine:latest",
            "cat",
            "/data/hello.txt",
        ]);
        assert_eq!(String::from_utf8_lossy(&output.stdout), "hello from echo\n");

        run_docker(["volume", "rm", "-f", &volume_name]);
        let _ = fs::remove_dir_all(dir);
        let _ = fs::remove_file(archive);
    }

    #[test]
    fn calculates_cpu_percent_from_counter_deltas() {
        let previous = StatsCounters {
            sample_time: UNIX_EPOCH,
            cpu_total: 100,
            system_cpu_total: 1_000,
            online_cpus: Some(4),
            memory_usage: None,
            memory_limit: None,
            network_rx: 0,
            network_tx: 0,
            disk_read: 0,
            disk_write: 0,
        };
        let current = StatsCounters {
            cpu_total: 150,
            system_cpu_total: 2_000,
            ..previous
        };

        assert_eq!(cpu_percent(current, Some(previous)), 20.);
    }

    #[test]
    fn clamps_rates_when_counters_reset_or_elapsed_is_zero() {
        assert_eq!(rate(1_000, 3_000, 2.), 1_000.);
        assert_eq!(rate(3_000, 1_000, 2.), 0.);
        assert_eq!(rate(1_000, 3_000, 0.), 0.);
    }

    #[test]
    fn subtracts_memory_cache_from_usage() {
        let stats = ContainerMemoryStats {
            usage: Some(10_000),
            stats: Some([("inactive_file".to_string(), 2_500)].into_iter().collect()),
            ..Default::default()
        };

        assert_eq!(memory_usage_bytes(&stats), 7_500);
    }

    #[test]
    fn converts_offset_datetime_to_system_time() {
        let date = time::OffsetDateTime::from_unix_timestamp(1_700_000_000)
            .unwrap()
            .replace_nanosecond(123_000_000)
            .unwrap();

        assert_eq!(
            super::system_time_from_offset_datetime(date),
            UNIX_EPOCH + Duration::new(1_700_000_000, 123_000_000)
        );
    }

    #[test]
    fn parses_volume_usage_from_system_df_volume_item() {
        let value = serde_json::json!({
            "Name": "echo-data",
            "UsageData": {
                "RefCount": 2,
                "Size": 4096
            }
        });

        let (name, usage) = volume_usage_from_value(value).unwrap();

        assert_eq!(name, "echo-data");
        assert_eq!(usage.ref_count, Some(2));
        assert_eq!(usage.size_bytes, Some(4096));
    }

    #[test]
    fn parses_volume_usage_from_top_level_system_df_value() {
        let value = serde_json::json!({
            "Volumes": [
                {
                    "Name": "echo-data",
                    "UsageData": {
                        "RefCount": 1,
                        "Size": 8192
                    }
                }
            ]
        });

        let usage = volume_usage_from_system_df_value(value).unwrap();

        assert_eq!(usage["echo-data"].ref_count, Some(1));
        assert_eq!(usage["echo-data"].size_bytes, Some(8192));
    }

    #[test]
    fn maps_network_inspect_to_domain_summary() {
        let network = Network {
            id: Some("network-1234567890abcdef".to_string()),
            name: Some("bridge".to_string()),
            driver: Some("bridge".to_string()),
            enable_ipv6: Some(false),
            ipam: Some(Ipam {
                config: Some(vec![IpamConfig {
                    subnet: Some("172.17.0.0/16".to_string()),
                    gateway: Some("172.17.0.1".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let inspect = NetworkInspect {
            id: Some("network-1234567890abcdef".to_string()),
            name: Some("bridge".to_string()),
            driver: Some("bridge".to_string()),
            enable_ipv6: Some(false),
            ipam: network.ipam.clone(),
            containers: Some(
                [(
                    "container-1".to_string(),
                    EndpointResource {
                        name: Some("web".to_string()),
                        endpoint_id: Some("endpoint-1".to_string()),
                        mac_address: Some("02:42:ac:11:00:02".to_string()),
                        ipv4_address: Some("172.17.0.2/16".to_string()),
                        ..Default::default()
                    },
                )]
                .into_iter()
                .collect(),
            ),
            ..Default::default()
        };

        let summary = network_summary_from_network(network, Some(inspect));

        assert_eq!(summary.name, "bridge");
        assert_eq!(summary.driver, "bridge");
        assert_eq!(summary.subnet.as_deref(), Some("172.17.0.0/16"));
        assert_eq!(summary.gateway.as_deref(), Some("172.17.0.1"));
        assert!(!summary.ipv6_enabled);
        assert_eq!(summary.endpoints.len(), 1);
        assert_eq!(summary.endpoints[0].name, "web");
        assert_eq!(
            summary.endpoints[0].ipv4_address.as_deref(),
            Some("172.17.0.2/16")
        );
    }

    #[test]
    fn builds_minimal_network_create_request() {
        let request = network_create_request(NetworkCreateConfig {
            name: " echo-net ".to_string(),
            driver: String::new(),
            subnet: None,
            gateway: None,
            enable_ipv6: false,
            internal: false,
        })
        .unwrap();

        assert_eq!(request.name, "echo-net");
        assert_eq!(request.driver.as_deref(), Some("bridge"));
        assert!(request.ipam.is_none());
        assert_eq!(request.enable_ipv6, None);
        assert_eq!(request.internal, None);
    }

    #[test]
    fn builds_network_create_request_with_ipam_and_flags() {
        let request = network_create_request(NetworkCreateConfig {
            name: "echo-net".to_string(),
            driver: "bridge".to_string(),
            subnet: Some(" 10.44.0.0/24 ".to_string()),
            gateway: Some(" 10.44.0.1 ".to_string()),
            enable_ipv6: true,
            internal: true,
        })
        .unwrap();

        let ipam_config = request.ipam.unwrap().config.unwrap();
        assert_eq!(ipam_config[0].subnet.as_deref(), Some("10.44.0.0/24"));
        assert_eq!(ipam_config[0].gateway.as_deref(), Some("10.44.0.1"));
        assert_eq!(request.enable_ipv6, Some(true));
        assert_eq!(request.internal, Some(true));
    }

    #[test]
    fn rejects_empty_network_names() {
        let result = network_create_request(NetworkCreateConfig {
            name: "   ".to_string(),
            driver: "bridge".to_string(),
            subnet: None,
            gateway: None,
            enable_ipv6: false,
            internal: false,
        });

        assert!(result.is_err());
    }

    #[test]
    fn decodes_chunked_http_body() {
        let body = "7\r\n{\"a\":1}\r\n0\r\n\r\n";

        assert_eq!(decode_chunked_body(body).as_deref(), Some("{\"a\":1}"));
    }

    #[test]
    fn maps_stdout_log_output_to_domain_line() {
        let line = container_log_line_from_output(LogOutput::StdOut {
            message: "2026-05-19T10:20:30.123456Z service ready\n".into(),
        });

        assert_eq!(line.stream, ContainerLogStreamKind::Stdout);
        assert!(line.timestamp.is_none());
        assert_eq!(line.message, "2026-05-19T10:20:30.123456Z service ready");
    }

    #[test]
    fn preserves_raw_application_log_line() {
        let line = container_log_line_from_output(LogOutput::StdErr {
            message: "service ready\n".into(),
        });

        assert_eq!(line.stream, ContainerLogStreamKind::Stderr);
        assert!(line.timestamp.is_none());
        assert_eq!(line.message, "service ready");
    }

    #[test]
    fn strips_terminal_control_sequences_from_tty_logs() {
        let line = container_log_line_from_output(LogOutput::Console {
            message: "\u{1b}[6n/ # ls\r\n\u{1b}[1;34mbin\u{1b}[m  dev\n".into(),
        });

        assert_eq!(line.stream, ContainerLogStreamKind::Console);
        assert_eq!(line.message, "/ # ls\nbin  dev");
    }

    #[test]
    fn normalizes_carriage_returns_in_terminal_logs() {
        assert_eq!(
            sanitize_terminal_log_text("first\r\nsecond\rthird"),
            "first\nsecond\nthird"
        );
    }
}

use std::{collections::HashMap, time::Duration};

use anyhow::{Context, Result, anyhow};
use bollard::{
    models::{EventMessage, EventMessageTypeEnum},
    query_parameters::EventsOptionsBuilder,
};
use futures_util::StreamExt;
use tokio::{
    sync::mpsc,
    time::{Instant, MissedTickBehavior},
};

use crate::{
    bridge::{
        backends::local,
        types::{ContainerEventKind, ContainerSyncSignal},
        types::{NetworkSyncSignal, ResourceSyncSignal},
    },
    domain::ConnectionTarget,
};

pub async fn watch_container_events(
    target: ConnectionTarget,
    poll_interval: Duration,
    sender: mpsc::Sender<ContainerSyncSignal>,
) -> Result<()> {
    let docker = local::connect(target)?;
    let mut filters = HashMap::new();
    filters.insert("type", vec!["container"]);

    let since = docker_event_since_now();
    let options = EventsOptionsBuilder::default()
        .since(&since)
        .filters(&filters)
        .build();
    let events = docker.events(Some(options));
    tokio::pin!(events);

    let mut poll = tokio::time::interval_at(Instant::now() + poll_interval, poll_interval);
    poll.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            event = events.next() => {
                match event {
                    Some(Ok(event)) => {
                        send_signal(&sender, ContainerSyncSignal::Changed(event_kind(event.action.as_deref()))).await?;
                    }
                    Some(Err(error)) => return Err(error).context("Docker event stream failed"),
                    None => return Err(anyhow!("Docker event stream ended")),
                }
            }
            _ = poll.tick() => {
                send_signal(&sender, ContainerSyncSignal::Poll).await?;
            }
        }
    }
}

pub async fn watch_network_events(
    target: ConnectionTarget,
    poll_interval: Duration,
    sender: mpsc::Sender<NetworkSyncSignal>,
) -> Result<()> {
    let docker = local::connect(target)?;
    let mut filters = HashMap::new();
    filters.insert("type", vec!["network"]);

    let since = docker_event_since_now();
    let options = EventsOptionsBuilder::default()
        .since(&since)
        .filters(&filters)
        .build();
    let events = docker.events(Some(options));
    tokio::pin!(events);

    let mut poll = tokio::time::interval_at(Instant::now() + poll_interval, poll_interval);
    poll.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            event = events.next() => {
                match event {
                    Some(Ok(_)) => {
                        send_network_signal(&sender, NetworkSyncSignal::Changed).await?;
                    }
                    Some(Err(error)) => return Err(error).context("Docker network event stream failed"),
                    None => return Err(anyhow!("Docker network event stream ended")),
                }
            }
            _ = poll.tick() => {
                send_network_signal(&sender, NetworkSyncSignal::Poll).await?;
            }
        }
    }
}

pub async fn watch_image_events(
    target: ConnectionTarget,
    poll_interval: Duration,
    sender: mpsc::Sender<ResourceSyncSignal>,
) -> Result<()> {
    watch_resource_events(
        target,
        poll_interval,
        sender,
        vec!["image", "container"],
        image_event_should_refresh,
        "Docker image event stream failed",
    )
    .await
}

pub async fn watch_volume_events(
    target: ConnectionTarget,
    poll_interval: Duration,
    sender: mpsc::Sender<ResourceSyncSignal>,
) -> Result<()> {
    watch_resource_events(
        target,
        poll_interval,
        sender,
        vec!["volume", "container"],
        volume_event_should_refresh,
        "Docker volume event stream failed",
    )
    .await
}

async fn watch_resource_events(
    target: ConnectionTarget,
    poll_interval: Duration,
    sender: mpsc::Sender<ResourceSyncSignal>,
    event_types: Vec<&'static str>,
    should_refresh: fn(&EventMessage) -> bool,
    stream_error: &'static str,
) -> Result<()> {
    let docker = local::connect(target)?;
    let mut filters = HashMap::new();
    filters.insert("type", event_types);

    let since = docker_event_since_now();
    let options = EventsOptionsBuilder::default()
        .since(&since)
        .filters(&filters)
        .build();
    let events = docker.events(Some(options));
    tokio::pin!(events);

    let mut poll = tokio::time::interval_at(Instant::now() + poll_interval, poll_interval);
    poll.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            event = events.next() => {
                match event {
                    Some(Ok(event)) => {
                        if should_refresh(&event) {
                            send_resource_signal(&sender, ResourceSyncSignal::Changed).await?;
                        }
                    }
                    Some(Err(error)) => return Err(error).context(stream_error),
                    None => return Err(anyhow!("Docker resource event stream ended")),
                }
            }
            _ = poll.tick() => {
                send_resource_signal(&sender, ResourceSyncSignal::Poll).await?;
            }
        }
    }
}

fn event_kind(action: Option<&str>) -> ContainerEventKind {
    match action {
        Some("destroy") | Some("remove") | Some("delete") => ContainerEventKind::Removed,
        Some(
            "start" | "stop" | "restart" | "pause" | "unpause" | "die" | "kill" | "create"
            | "rename" | "health_status",
        ) => ContainerEventKind::StateChanged,
        _ => ContainerEventKind::Other,
    }
}

fn image_event_should_refresh(event: &EventMessage) -> bool {
    match event.typ {
        Some(EventMessageTypeEnum::IMAGE) => true,
        Some(EventMessageTypeEnum::CONTAINER) => {
            container_resource_event_should_refresh(event.action.as_deref())
        }
        _ => false,
    }
}

fn volume_event_should_refresh(event: &EventMessage) -> bool {
    match event.typ {
        Some(EventMessageTypeEnum::VOLUME) => true,
        Some(EventMessageTypeEnum::CONTAINER) => {
            container_resource_event_should_refresh(event.action.as_deref())
        }
        _ => false,
    }
}

fn container_resource_event_should_refresh(action: Option<&str>) -> bool {
    matches!(action, Some("create" | "destroy" | "remove" | "delete"))
}

async fn send_signal(
    sender: &mpsc::Sender<ContainerSyncSignal>,
    signal: ContainerSyncSignal,
) -> Result<()> {
    sender
        .send(signal)
        .await
        .map_err(|_| anyhow!("container sync receiver closed"))
}

async fn send_network_signal(
    sender: &mpsc::Sender<NetworkSyncSignal>,
    signal: NetworkSyncSignal,
) -> Result<()> {
    sender
        .send(signal)
        .await
        .map_err(|_| anyhow!("network sync receiver closed"))
}

async fn send_resource_signal(
    sender: &mpsc::Sender<ResourceSyncSignal>,
    signal: ResourceSyncSignal,
) -> Result<()> {
    sender
        .send(signal)
        .await
        .map_err(|_| anyhow!("resource sync receiver closed"))
}

fn docker_event_since_now() -> String {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => format!("{}.{}", duration.as_secs(), duration.subsec_nanos()),
        Err(_) => "0".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use bollard::models::{EventMessage, EventMessageTypeEnum};

    use crate::bridge::types::ContainerEventKind;

    use super::{
        container_resource_event_should_refresh, docker_event_since_now, event_kind,
        image_event_should_refresh, volume_event_should_refresh,
    };

    #[test]
    fn event_since_timestamp_is_docker_compatible() {
        let timestamp = docker_event_since_now();
        let (seconds, nanos) = timestamp
            .split_once('.')
            .expect("timestamp should include nanoseconds");

        assert!(seconds.parse::<u64>().is_ok());
        assert!(nanos.parse::<u32>().is_ok());
    }

    #[test]
    fn classifies_container_remove_events() {
        assert_eq!(event_kind(Some("destroy")), ContainerEventKind::Removed);
        assert_eq!(event_kind(Some("remove")), ContainerEventKind::Removed);
        assert_eq!(event_kind(Some("stop")), ContainerEventKind::StateChanged);
        assert_eq!(event_kind(Some("exec_create")), ContainerEventKind::Other);
    }

    #[test]
    fn classifies_container_resource_events() {
        assert!(container_resource_event_should_refresh(Some("create")));
        assert!(container_resource_event_should_refresh(Some("destroy")));
        assert!(container_resource_event_should_refresh(Some("remove")));
        assert!(container_resource_event_should_refresh(Some("delete")));
        assert!(!container_resource_event_should_refresh(Some("start")));
        assert!(!container_resource_event_should_refresh(Some("stop")));
        assert!(!container_resource_event_should_refresh(None));
    }

    #[test]
    fn image_refreshes_for_image_and_container_usage_events() {
        assert!(image_event_should_refresh(&event(
            Some(EventMessageTypeEnum::IMAGE),
            Some("pull"),
        )));
        assert!(image_event_should_refresh(&event(
            Some(EventMessageTypeEnum::CONTAINER),
            Some("create"),
        )));
        assert!(!image_event_should_refresh(&event(
            Some(EventMessageTypeEnum::CONTAINER),
            Some("start"),
        )));
        assert!(!image_event_should_refresh(&event(
            Some(EventMessageTypeEnum::VOLUME),
            Some("create"),
        )));
    }

    #[test]
    fn volume_refreshes_for_volume_and_container_usage_events() {
        assert!(volume_event_should_refresh(&event(
            Some(EventMessageTypeEnum::VOLUME),
            Some("create"),
        )));
        assert!(volume_event_should_refresh(&event(
            Some(EventMessageTypeEnum::CONTAINER),
            Some("destroy"),
        )));
        assert!(!volume_event_should_refresh(&event(
            Some(EventMessageTypeEnum::CONTAINER),
            Some("stop"),
        )));
        assert!(!volume_event_should_refresh(&event(
            Some(EventMessageTypeEnum::IMAGE),
            Some("pull"),
        )));
    }

    fn event(typ: Option<EventMessageTypeEnum>, action: Option<&str>) -> EventMessage {
        EventMessage {
            typ,
            action: action.map(ToString::to_string),
            actor: None,
            scope: None,
            time: None,
            time_nano: None,
        }
    }
}

use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use serde::Deserialize;

use crate::domain::{ConnectionTarget, DockerBackendSummary};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn resolve_current_target() -> ConnectionTarget {
    if let Ok(host) = env::var("DOCKER_HOST")
        && !host.trim().is_empty()
    {
        return ConnectionTarget::DockerHost(host.trim().to_string());
    }

    docker_config_context_host()
        .or_else(docker_context_host)
        .map(ConnectionTarget::DockerHost)
        .unwrap_or(ConnectionTarget::DefaultContext)
}

pub fn discover_backend_candidates() -> Vec<DockerBackendSummary> {
    let mut backends = Vec::new();
    let mut seen = HashSet::new();

    add_backend(
        &mut backends,
        &mut seen,
        "Docker defaults".to_string(),
        ConnectionTarget::DefaultContext,
    );

    for context in docker_config_contexts() {
        let target = ConnectionTarget::DockerHost(context.host.clone());
        let name = backend_display_name(&context.name, &context.host);
        add_backend(&mut backends, &mut seen, name, target);
    }

    for (name, path) in known_socket_candidates() {
        if path.exists() {
            let target = ConnectionTarget::DockerHost(format!("unix://{}", path.display()));
            add_backend(&mut backends, &mut seen, name, target);
        }
    }

    let target = resolve_current_target();
    let name = backend_display_name(&target.display_name(), &target.endpoint());
    add_backend(&mut backends, &mut seen, name, target);

    backends
}

fn add_backend(
    backends: &mut Vec<DockerBackendSummary>,
    seen: &mut HashSet<String>,
    name: String,
    target: ConnectionTarget,
) {
    let id = target.stable_id();
    if !seen.insert(id) {
        return;
    }

    backends.push(DockerBackendSummary::new(name, target));
}

fn docker_context_host() -> Option<String> {
    for candidate in docker_cli_candidates() {
        let mut command = Command::new(&candidate);
        hide_command_window(&mut command);

        let output = command
            .args([
                "context",
                "inspect",
                "--format",
                "{{.Endpoints.docker.Host}}",
            ])
            .output()
            .ok();

        let Some(output) = output else {
            continue;
        };

        if !output.status.success() {
            continue;
        }

        if let Some(host) = parse_context_host(&String::from_utf8(output.stdout).ok()?) {
            return Some(host);
        }
    }

    None
}

fn docker_config_context_host() -> Option<String> {
    let context_name = env::var("DOCKER_CONTEXT")
        .ok()
        .and_then(|value| non_empty(value.trim()))
        .or_else(|| docker_config_dir().and_then(|dir| read_current_context(&dir)));

    let context_name = context_name?;
    if context_name == "default" {
        return None;
    }

    docker_config_dir().and_then(|dir| read_context_host(&dir, &context_name))
}

fn docker_config_contexts() -> Vec<DockerContextEndpoint> {
    docker_config_dir()
        .map(|dir| read_contexts(&dir))
        .unwrap_or_default()
}

fn docker_config_dir() -> Option<PathBuf> {
    if let Ok(path) = env::var("DOCKER_CONFIG")
        && !path.trim().is_empty()
    {
        return Some(PathBuf::from(path));
    }

    dirs::home_dir().map(|home| home.join(".docker"))
}

fn read_current_context(config_dir: &Path) -> Option<String> {
    let content = fs::read_to_string(config_dir.join("config.json")).ok()?;
    parse_current_context(&content)
}

fn read_context_host(config_dir: &Path, context_name: &str) -> Option<String> {
    read_contexts(config_dir)
        .into_iter()
        .find(|context| context.name == context_name)
        .map(|context| context.host)
}

fn read_contexts(config_dir: &Path) -> Vec<DockerContextEndpoint> {
    let meta_root = config_dir.join("contexts").join("meta");
    let Ok(entries) = fs::read_dir(meta_root) else {
        return Vec::new();
    };

    let mut contexts = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let content = fs::read_to_string(entry.path().join("meta.json")).ok()?;
            parse_context_meta(&content)
        })
        .collect::<Vec<_>>();

    contexts.sort_by(|left, right| left.name.cmp(&right.name));
    contexts
}

fn parse_context_host(output: &str) -> Option<String> {
    let host = output.trim();

    if host.is_empty() || host == "<no value>" {
        None
    } else {
        Some(host.to_string())
    }
}

fn parse_current_context(content: &str) -> Option<String> {
    let config = serde_json::from_str::<DockerConfig>(content).ok()?;
    config
        .current_context
        .as_deref()
        .and_then(|context| non_empty(context.trim()))
}

fn parse_context_meta(content: &str) -> Option<DockerContextEndpoint> {
    let meta = serde_json::from_str::<DockerContextMeta>(content).ok()?;
    let host = meta
        .endpoints
        .docker
        .host
        .as_deref()
        .and_then(|host| non_empty(host.trim()))?;

    if host == "<no value>" {
        return None;
    }

    Some(DockerContextEndpoint {
        name: meta.name,
        host,
    })
}

fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

fn docker_cli_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    push_candidate(&mut candidates, &mut seen, PathBuf::from("docker"));
    push_candidate(
        &mut candidates,
        &mut seen,
        PathBuf::from("/usr/local/bin/docker"),
    );
    push_candidate(
        &mut candidates,
        &mut seen,
        PathBuf::from("/opt/homebrew/bin/docker"),
    );
    push_candidate(
        &mut candidates,
        &mut seen,
        PathBuf::from("/Applications/Docker.app/Contents/Resources/bin/docker"),
    );

    if let Some(home) = dirs::home_dir() {
        push_candidate(&mut candidates, &mut seen, home.join(".docker/bin/docker"));
        push_candidate(
            &mut candidates,
            &mut seen,
            home.join(".orbstack/bin/docker"),
        );
    }

    candidates
}

fn push_candidate(candidates: &mut Vec<PathBuf>, seen: &mut HashSet<String>, path: PathBuf) {
    let key = path.to_string_lossy().to_string();
    if seen.insert(key) {
        candidates.push(path);
    }
}

fn known_socket_candidates() -> Vec<(String, PathBuf)> {
    let mut candidates = Vec::new();
    let Some(home) = dirs::home_dir() else {
        return candidates;
    };

    candidates.push((
        "OrbStack".to_string(),
        home.join(".orbstack/run/docker.sock"),
    ));
    candidates.push((
        "Docker Desktop".to_string(),
        home.join(".docker/run/docker.sock"),
    ));
    candidates.push((
        "Colima".to_string(),
        home.join(".colima/default/docker.sock"),
    ));
    candidates.extend(profile_socket_candidates(
        &home.join(".colima"),
        "Colima",
        "docker.sock",
    ));
    candidates.push((
        "Lima".to_string(),
        home.join(".lima/docker/sock/docker.sock"),
    ));
    candidates.extend(profile_socket_candidates(
        &home.join(".lima"),
        "Lima",
        "sock/docker.sock",
    ));

    candidates
}

fn profile_socket_candidates(
    root: &Path,
    provider: &str,
    relative_socket: &str,
) -> Vec<(String, PathBuf)> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let profile = entry.file_name().to_string_lossy().to_string();
            if profile == "default" {
                return None;
            }

            let socket = entry.path().join(relative_socket);
            Some((format!("{provider} ({profile})"), socket))
        })
        .collect()
}

fn backend_display_name(name: &str, endpoint: &str) -> String {
    let normalized = endpoint.to_lowercase();

    if normalized.contains("orbstack") {
        return "OrbStack".to_string();
    }
    if normalized.contains("colima") {
        return "Colima".to_string();
    }
    if normalized.contains("docker/run/docker.sock") {
        return "Docker Desktop".to_string();
    }
    if normalized.contains("lima") {
        return "Lima".to_string();
    }

    let trimmed = name.trim();
    if trimmed.is_empty() {
        "Docker".to_string()
    } else {
        trimmed.to_string()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DockerContextEndpoint {
    name: String,
    host: String,
}

#[derive(Deserialize)]
struct DockerConfig {
    #[serde(rename = "currentContext")]
    current_context: Option<String>,
}

#[derive(Deserialize)]
struct DockerContextMeta {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Endpoints")]
    endpoints: DockerContextMetaEndpoints,
}

#[derive(Deserialize)]
struct DockerContextMetaEndpoints {
    docker: DockerContextMetaDockerEndpoint,
}

#[derive(Deserialize)]
struct DockerContextMetaDockerEndpoint {
    #[serde(rename = "Host")]
    host: Option<String>,
}

#[cfg(windows)]
fn hide_command_window(command: &mut Command) {
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_command_window(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::{parse_context_host, parse_context_meta, parse_current_context};

    #[test]
    fn parses_context_host() {
        assert_eq!(
            parse_context_host("unix:///var/run/docker.sock\n"),
            Some("unix:///var/run/docker.sock".to_string())
        );
    }

    #[test]
    fn ignores_empty_context_host() {
        assert_eq!(parse_context_host("\n"), None);
        assert_eq!(parse_context_host("<no value>\n"), None);
    }

    #[test]
    fn parses_current_context_from_config() {
        let content = r#"{"currentContext":"colima"}"#;
        assert_eq!(parse_current_context(content), Some("colima".to_string()));
    }

    #[test]
    fn parses_context_meta_host() {
        let content = r#"{"Name":"colima","Endpoints":{"docker":{"Host":"unix:///Users/me/.colima/default/docker.sock","SkipTLSVerify":false}}}"#;
        let context = parse_context_meta(content).unwrap();
        assert_eq!(context.name, "colima");
        assert_eq!(context.host, "unix:///Users/me/.colima/default/docker.sock");
    }

    #[test]
    fn ignores_context_meta_without_host() {
        let content = r#"{"Name":"broken","Endpoints":{"docker":{"Host":"<no value>"}}}"#;
        assert_eq!(parse_context_meta(content), None);
    }
}

use std::{env, process::Command};

use crate::domain::ConnectionTarget;

pub fn resolve_current_target() -> ConnectionTarget {
    if let Ok(host) = env::var("DOCKER_HOST")
        && !host.trim().is_empty()
    {
        return ConnectionTarget::DockerHost(host.trim().to_string());
    }

    docker_context_host()
        .map(ConnectionTarget::DockerHost)
        .unwrap_or(ConnectionTarget::DefaultContext)
}

fn docker_context_host() -> Option<String> {
    let output = Command::new("docker")
        .args([
            "context",
            "inspect",
            "--format",
            "{{.Endpoints.docker.Host}}",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_context_host(&String::from_utf8(output.stdout).ok()?)
}

fn parse_context_host(output: &str) -> Option<String> {
    let host = output.trim();

    if host.is_empty() || host == "<no value>" {
        None
    } else {
        Some(host.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::parse_context_host;

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
}

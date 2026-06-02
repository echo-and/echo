use gpui::{Hsla, hsla};
use gpui_component::ThemeMode;

use crate::assets::{DOCKER_ICON_ASSETS, DOCKER_ICON_COLORS, DOCKER_ICON_NAMES};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DockerIconStyle {
    pub(super) path: &'static str,
    pub(super) background: Option<Hsla>,
    pub(super) grayscale: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DockerIconState {
    Normal,
    Stopped,
}

pub(super) fn docker_icon_style_for_reference(
    reference: &str,
    fallback: &'static str,
    theme_mode: ThemeMode,
    state: DockerIconState,
) -> DockerIconStyle {
    let Some(index) = docker_icon_index_for_reference(reference) else {
        return DockerIconStyle {
            path: fallback,
            background: None,
            grayscale: false,
        };
    };

    DockerIconStyle {
        path: DOCKER_ICON_ASSETS[index],
        background: docker_icon_background(DOCKER_ICON_NAMES[index], theme_mode, state),
        grayscale: state == DockerIconState::Stopped,
    }
}

fn docker_icon_index_for_reference(reference: &str) -> Option<usize> {
    let icon_name = official_image_name(reference)?;
    DOCKER_ICON_NAMES
        .binary_search_by(|probe| probe.cmp(&icon_name.as_str()))
        .ok()
}

fn docker_icon_background(
    name: &str,
    theme_mode: ThemeMode,
    state: DockerIconState,
) -> Option<Hsla> {
    if state == DockerIconState::Stopped {
        return Some(stopped_icon_background(theme_mode));
    }

    let color = DOCKER_ICON_COLORS
        .binary_search_by(|(probe, _)| probe.cmp(&name))
        .ok()
        .map(|index| DOCKER_ICON_COLORS[index].1)?;
    let (h, s, l) = rgb_to_hsl(color);

    if theme_mode.is_dark() {
        Some(hsla(
            h,
            (s * 0.30).clamp(0.08, 0.34),
            (0.84 + l * 0.08).clamp(0.84, 0.92),
            1.0,
        ))
    } else {
        Some(hsla(
            h,
            (s * 0.34).clamp(0.08, 0.38),
            (0.90 + l * 0.06).clamp(0.90, 0.96),
            1.0,
        ))
    }
}

fn stopped_icon_background(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        hsla(0.0, 0.0, 0.86, 1.0)
    } else {
        hsla(0.0, 0.0, 0.92, 1.0)
    }
}

fn rgb_to_hsl(color: u32) -> (f32, f32, f32) {
    let r = ((color >> 16) & 0xff) as f32 / 255.;
    let g = ((color >> 8) & 0xff) as f32 / 255.;
    let b = (color & 0xff) as f32 / 255.;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.;

    if (max - min).abs() < f32::EPSILON {
        return (0., 0., l);
    }

    let delta = max - min;
    let s = if l > 0.5 {
        delta / (2. - max - min)
    } else {
        delta / (max + min)
    };
    let h = if (max - r).abs() < f32::EPSILON {
        ((g - b) / delta + if g < b { 6. } else { 0. }) / 6.
    } else if (max - g).abs() < f32::EPSILON {
        ((b - r) / delta + 2.) / 6.
    } else {
        ((r - g) / delta + 4.) / 6.
    };

    (h, s, l)
}

fn official_image_name(reference: &str) -> Option<String> {
    let reference = reference.trim();
    if reference.is_empty() || reference == "<none>:<none>" {
        return None;
    }

    let without_digest = reference
        .split_once('@')
        .map_or(reference, |(name, _)| name);
    let normalized = strip_tag(without_digest).to_ascii_lowercase();
    let parts = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    match parts.as_slice() {
        [name] => Some((*name).to_string()),
        ["library", name] => Some((*name).to_string()),
        ["docker.io", name] => Some((*name).to_string()),
        ["docker.io", "library", name] => Some((*name).to_string()),
        ["registry-1.docker.io", "library", name] => Some((*name).to_string()),
        ["index.docker.io", "library", name] => Some((*name).to_string()),
        _ => None,
    }
}

fn strip_tag(reference: &str) -> &str {
    let Some(last_slash) = reference.rfind('/') else {
        return reference
            .split_once(':')
            .map_or(reference, |(name, _)| name);
    };

    let last_segment = &reference[last_slash + 1..];
    let Some(tag_start) = last_segment.rfind(':') else {
        return reference;
    };

    if last_segment[tag_start + 1..].contains(':') {
        reference
    } else {
        &reference[..last_slash + 1 + tag_start]
    }
}

#[cfg(test)]
mod tests {
    use gpui_component::ThemeMode;

    use super::{DockerIconState, docker_icon_style_for_reference, official_image_name};

    const FALLBACK_ICON: &str = "fallback.svg";

    #[test]
    fn matches_official_image_references() {
        assert_eq!(
            docker_icon_style_for_reference(
                "postgres:16",
                FALLBACK_ICON,
                ThemeMode::Light,
                DockerIconState::Normal
            )
            .path,
            "assets/images/docker-icons/postgres.png"
        );
        assert_eq!(
            docker_icon_style_for_reference(
                "postgres@sha256:abc",
                FALLBACK_ICON,
                ThemeMode::Light,
                DockerIconState::Normal
            )
            .path,
            "assets/images/docker-icons/postgres.png"
        );
        assert_eq!(
            docker_icon_style_for_reference(
                "library/postgres:16",
                FALLBACK_ICON,
                ThemeMode::Light,
                DockerIconState::Normal
            )
            .path,
            "assets/images/docker-icons/postgres.png"
        );
        assert_eq!(
            docker_icon_style_for_reference(
                "docker.io/postgres:16",
                FALLBACK_ICON,
                ThemeMode::Light,
                DockerIconState::Normal
            )
            .path,
            "assets/images/docker-icons/postgres.png"
        );
        assert_eq!(
            docker_icon_style_for_reference(
                "docker.io/library/postgres:16",
                FALLBACK_ICON,
                ThemeMode::Light,
                DockerIconState::Normal
            )
            .path,
            "assets/images/docker-icons/postgres.png"
        );
    }

    #[test]
    fn rejects_non_official_image_references() {
        assert_eq!(
            docker_icon_style_for_reference(
                "bitnami/postgres:16",
                FALLBACK_ICON,
                ThemeMode::Light,
                DockerIconState::Normal
            )
            .path,
            FALLBACK_ICON
        );
        assert_eq!(
            docker_icon_style_for_reference(
                "localhost:5000/postgres:16",
                FALLBACK_ICON,
                ThemeMode::Light,
                DockerIconState::Normal
            )
            .path,
            FALLBACK_ICON
        );
        assert_eq!(
            docker_icon_style_for_reference(
                "ghcr.io/org/postgres:16",
                FALLBACK_ICON,
                ThemeMode::Light,
                DockerIconState::Normal
            )
            .path,
            FALLBACK_ICON
        );
    }

    #[test]
    fn returns_background_for_known_official_icon() {
        let style = docker_icon_style_for_reference(
            "postgres:16",
            FALLBACK_ICON,
            ThemeMode::Light,
            DockerIconState::Normal,
        );

        assert!(style.background.is_some());
        assert!(!style.grayscale);
    }

    #[test]
    fn lightens_dark_mode_icon_background() {
        let style = docker_icon_style_for_reference(
            "postgres:16",
            FALLBACK_ICON,
            ThemeMode::Dark,
            DockerIconState::Normal,
        );
        let background = style.background.expect("expected postgres background");

        assert!(background.l >= 0.84);
    }

    #[test]
    fn grayscales_stopped_official_icon() {
        let style = docker_icon_style_for_reference(
            "postgres:16",
            FALLBACK_ICON,
            ThemeMode::Light,
            DockerIconState::Stopped,
        );
        let background = style.background.expect("expected stopped background");

        assert_eq!(style.path, "assets/images/docker-icons/postgres.png");
        assert!(style.grayscale);
        assert_eq!(background.s, 0.0);
        assert!((background.l - 0.92).abs() < f32::EPSILON);
    }

    #[test]
    fn stopped_non_official_icon_keeps_fallback_style() {
        let style = docker_icon_style_for_reference(
            "private/postgres:16",
            FALLBACK_ICON,
            ThemeMode::Light,
            DockerIconState::Stopped,
        );

        assert_eq!(style.path, FALLBACK_ICON);
        assert_eq!(style.background, None);
        assert!(!style.grayscale);
    }

    #[test]
    fn strips_only_last_segment_tag() {
        assert_eq!(
            official_image_name("localhost:5000/postgres:16").as_deref(),
            None
        );
        assert_eq!(
            official_image_name("docker.io/library/postgres:16").as_deref(),
            Some("postgres")
        );
    }
}

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};
use gpui_component_assets::Assets as ComponentAssets;

pub struct EchoAssets;

include!(concat!(env!("OUT_DIR"), "/docker_icons.rs"));

impl AssetSource for EchoAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = load_echo_asset(path) {
            return Ok(Some(bytes));
        }

        ComponentAssets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut assets = ComponentAssets.list(path)?;

        for asset in [
            "assets/icons/box.svg",
            "assets/icons/clock.svg",
            "assets/icons/disc-album.svg",
            "assets/icons/cpu.svg",
            "assets/icons/hard-drive.svg",
            "assets/icons/microchip.svg",
            "assets/icons/network.svg",
            "assets/icons/chevrons-left-right.svg",
            "assets/icons/copy.svg",
            "assets/icons/ellipsis.svg",
            "assets/icons/heart.svg",
            "assets/icons/pause.svg",
            "assets/icons/play.svg",
            "assets/icons/refresh-cw.svg",
            "assets/icons/rotate-cw.svg",
            "assets/icons/sliders-horizontal.svg",
            "assets/icons/square.svg",
            "assets/icons/trash-2.svg",
            "assets/images/Logo.svg",
            "assets/images/logo-gray-placeholder.svg",
            "assets/images/list-icons/List-Container-Icon.svg",
            "assets/images/list-icons/List-Container-Icon-Inactive.svg",
            "assets/images/list-icons/List-Image-Icon.svg",
            "assets/images/list-icons/List-Volume-Icon.svg",
            "assets/images/list-icons/List-Compose-Icon.svg",
            "assets/images/list-icons/List-Compose-Icon-Inactive.svg",
            "assets/images/list-icons/List-Child-Icon.svg",
        ]
        .into_iter()
        .chain(DOCKER_ICON_ASSETS.iter().copied())
        {
            if asset.starts_with(path)
                || asset
                    .strip_prefix("assets/")
                    .is_some_and(|p| p.starts_with(path))
            {
                assets.push(asset.into());
            }
        }

        Ok(assets)
    }
}

fn load_echo_asset(path: &str) -> Option<Cow<'static, [u8]>> {
    let path = path.strip_prefix("assets/").unwrap_or(path);

    if let Some(bytes) = load_docker_icon_asset(path) {
        return Some(bytes);
    }

    match path {
        "icons/box.svg" => Some(Cow::Borrowed(include_bytes!("../assets/icons/box.svg"))),
        "icons/clock.svg" => Some(Cow::Borrowed(include_bytes!("../assets/icons/clock.svg"))),
        "icons/cpu.svg" => Some(Cow::Borrowed(include_bytes!("../assets/icons/cpu.svg"))),
        "icons/disc-album.svg" => Some(Cow::Borrowed(include_bytes!(
            "../assets/icons/disc-album.svg"
        ))),
        "icons/hard-drive.svg" => Some(Cow::Borrowed(include_bytes!(
            "../assets/icons/hard-drive.svg"
        ))),
        "icons/microchip.svg" => Some(Cow::Borrowed(include_bytes!(
            "../assets/icons/microchip.svg"
        ))),
        "icons/network.svg" => Some(Cow::Borrowed(include_bytes!("../assets/icons/network.svg"))),
        "icons/chevrons-left-right.svg" => Some(Cow::Borrowed(include_bytes!(
            "../assets/icons/chevrons-left-right.svg"
        ))),
        "icons/copy.svg" => Some(Cow::Borrowed(include_bytes!("../assets/icons/copy.svg"))),
        "icons/ellipsis.svg" => Some(Cow::Borrowed(include_bytes!(
            "../assets/icons/ellipsis.svg"
        ))),
        "icons/heart.svg" => Some(Cow::Borrowed(include_bytes!("../assets/icons/heart.svg"))),
        "icons/pause.svg" => Some(Cow::Borrowed(include_bytes!("../assets/icons/pause.svg"))),
        "icons/play.svg" => Some(Cow::Borrowed(include_bytes!("../assets/icons/play.svg"))),
        "icons/refresh-cw.svg" => Some(Cow::Borrowed(include_bytes!(
            "../assets/icons/refresh-cw.svg"
        ))),
        "icons/rotate-cw.svg" => Some(Cow::Borrowed(include_bytes!(
            "../assets/icons/rotate-cw.svg"
        ))),
        "icons/sliders-horizontal.svg" => Some(Cow::Borrowed(include_bytes!(
            "../assets/icons/sliders-horizontal.svg"
        ))),
        "icons/square.svg" => Some(Cow::Borrowed(include_bytes!("../assets/icons/square.svg"))),
        "icons/trash-2.svg" => Some(Cow::Borrowed(include_bytes!("../assets/icons/trash-2.svg"))),
        "images/Logo.svg" => Some(Cow::Borrowed(include_bytes!("../assets/images/Logo.svg"))),
        "images/logo-gray-placeholder.svg" => Some(Cow::Borrowed(include_bytes!(
            "../assets/images/logo-gray-placeholder.svg"
        ))),
        "images/list-icons/List-Container-Icon.svg" => Some(Cow::Borrowed(include_bytes!(
            "../assets/images/list-icons/List-Container-Icon.svg"
        ))),
        "images/list-icons/List-Container-Icon-Inactive.svg" => Some(Cow::Borrowed(
            include_bytes!("../assets/images/list-icons/List-Container-Icon-Inactive.svg"),
        )),
        "images/list-icons/List-Image-Icon.svg" => Some(Cow::Borrowed(include_bytes!(
            "../assets/images/list-icons/List-Image-Icon.svg"
        ))),
        "images/list-icons/List-Volume-Icon.svg" => Some(Cow::Borrowed(include_bytes!(
            "../assets/images/list-icons/List-Volume-Icon.svg"
        ))),
        "images/list-icons/List-Compose-Icon.svg" => Some(Cow::Borrowed(include_bytes!(
            "../assets/images/list-icons/List-Compose-Icon.svg"
        ))),
        "images/list-icons/List-Compose-Icon-Inactive.svg" => Some(Cow::Borrowed(include_bytes!(
            "../assets/images/list-icons/List-Compose-Icon-Inactive.svg"
        ))),
        "images/list-icons/List-Child-Icon.svg" => Some(Cow::Borrowed(include_bytes!(
            "../assets/images/list-icons/List-Child-Icon.svg"
        ))),
        _ => None,
    }
}

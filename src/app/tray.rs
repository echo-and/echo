use std::time::Duration;

use anyhow::{Context as _, Result};
use gpui::*;
use gpui_component::Root;
use image::ImageFormat;
use rust_i18n::t;
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
};

use crate::app::{AppServices, EchoApp};
use crate::ui::{apply_echo_font_preference, apply_echo_theme_overrides};

const TRAY_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(100);
#[cfg(target_os = "macos")]
const MACOS_TRAY_ICON: &[u8] =
    include_bytes!("../../assets/images/tray-icons/tray-macos-template.png");
#[cfg(not(target_os = "macos"))]
const COLORED_TRAY_ICON: &[u8] =
    include_bytes!("../../assets/images/tray-icons/tray-windows-color.ico");

pub fn install_tray(cx: &mut App) -> Result<()> {
    let menu = Menu::new();
    let show_item = MenuItem::with_id("echo.show", &t!("tray.show"), true, None);
    let quit_item = MenuItem::with_id("echo.quit", &t!("tray.quit"), true, None);
    let separator = PredefinedMenuItem::separator();

    menu.append_items(&[&show_item, &separator, &quit_item])?;

    let tray_icon = tray_icon()?;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(&t!("tray.tooltip"))
        .with_icon(tray_icon.icon)
        .with_icon_as_template(tray_icon.is_template)
        .with_menu_on_left_click(true)
        .with_menu_on_right_click(false)
        .build()?;

    let show_item_id = show_item.id().clone();
    let quit_item_id = quit_item.id().clone();
    let tray_task = cx.spawn(async move |cx| {
        loop {
            handle_tray_events(&show_item_id, &quit_item_id, cx);
            cx.background_executor()
                .timer(TRAY_EVENT_POLL_INTERVAL)
                .await;
        }
    });

    cx.update_global::<AppServices, _>(|services, _| {
        services.tray = Some(tray);
        services.tray_show_item_id = Some(show_item.id().clone());
        services.tray_quit_item_id = Some(quit_item.id().clone());
        services._tray_task = Some(tray_task);
    });

    Ok(())
}

pub fn open_echo_window(cx: &mut App) -> Result<WindowHandle<Root>> {
    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(1080.), px(722.)), cx)),
        titlebar: Some(TitlebarOptions {
            title: Some(t!("app.title").to_string().into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(8.), px(8.))),
        }),
        ..Default::default()
    };

    let window = cx.open_window(window_options, |window, cx| {
        window.on_window_should_close(cx, |_, cx| {
            hide_echo_window(cx);
            false
        });

        let view = cx.new(|cx| EchoApp::new(window, cx));
        let (theme_mode, font_family) = view
            .read(cx)
            .model
            .read_with(cx, |model, _| (model.theme_mode, model.font_family));
        gpui_component::Theme::change(theme_mode, Some(window), cx);
        apply_echo_theme_overrides(cx);
        apply_echo_font_preference(font_family, cx);
        view.update(cx, |app, cx| app.start_container_sync(cx));

        cx.new(|cx| Root::new(view, window, cx))
    })?;

    cx.update_global::<AppServices, _>(|services, _| {
        services.window = Some(window);
    });
    activate_echo_window(window, cx);

    Ok(window)
}

pub fn hide_echo_window(cx: &mut App) {
    set_app_hidden(true, cx);
    cx.hide();
}

pub fn show_echo_window(cx: &mut App) {
    set_app_hidden(false, cx);
    let window = cx.global::<AppServices>().window;

    if let Some(window) = window {
        activate_echo_window(window, cx);
    }
}

fn activate_echo_window(window: WindowHandle<Root>, cx: &mut App) {
    cx.activate(true);
    let _ = window.update(cx, |_, window, _| window.activate_window());
}

fn handle_tray_left_click(cx: &mut App) {
    if cx.global::<AppServices>().app_hidden {
        show_echo_window(cx);
    }
}

fn set_app_hidden(hidden: bool, cx: &mut App) {
    cx.update_global::<AppServices, _>(|services, _| {
        services.app_hidden = hidden;
        if let Some(tray) = services.tray.as_ref() {
            tray.set_show_menu_on_left_click(!hidden);
        }
    });
}

fn handle_tray_events(show_item_id: &MenuId, quit_item_id: &MenuId, cx: &mut AsyncApp) {
    while let Ok(event) = TrayIconEvent::receiver().try_recv() {
        if should_show_for_tray_event(&event) {
            cx.update(handle_tray_left_click);
        }
    }

    while let Ok(event) = MenuEvent::receiver().try_recv() {
        if event.id == show_item_id {
            cx.update(show_echo_window);
        } else if event.id == quit_item_id {
            cx.update(|cx| cx.quit());
        }
    }
}

fn should_show_for_tray_event(event: &TrayIconEvent) -> bool {
    matches!(
        event,
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        }
    )
}

struct TrayIconAsset {
    bytes: &'static [u8],
    format: ImageFormat,
    is_template: bool,
}

struct LoadedTrayIcon {
    icon: Icon,
    is_template: bool,
}

fn tray_icon() -> Result<LoadedTrayIcon> {
    let asset = tray_icon_asset();
    let image = image::load_from_memory_with_format(asset.bytes, asset.format)
        .context("failed to decode tray icon asset")?
        .into_rgba8();
    let (width, height) = image.dimensions();
    let icon = Icon::from_rgba(image.into_raw(), width, height)
        .context("failed to create tray icon from RGBA data")?;

    Ok(LoadedTrayIcon {
        icon,
        is_template: asset.is_template,
    })
}

#[cfg(target_os = "macos")]
fn tray_icon_asset() -> TrayIconAsset {
    TrayIconAsset {
        bytes: MACOS_TRAY_ICON,
        format: ImageFormat::Png,
        is_template: true,
    }
}

#[cfg(not(target_os = "macos"))]
fn tray_icon_asset() -> TrayIconAsset {
    TrayIconAsset {
        bytes: COLORED_TRAY_ICON,
        format: ImageFormat::Ico,
        is_template: false,
    }
}

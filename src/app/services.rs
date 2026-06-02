use std::sync::Arc;

use gpui::{Global, Task, WindowHandle};
use tray_icon::{TrayIcon, menu::MenuId};

use crate::bridge::Bridge;

pub struct AppServices {
    pub bridge: Arc<Bridge>,
    pub tray: Option<TrayIcon>,
    pub tray_show_item_id: Option<MenuId>,
    pub tray_quit_item_id: Option<MenuId>,
    pub window: Option<WindowHandle<gpui_component::Root>>,
    pub app_hidden: bool,
    pub _tray_task: Option<Task<()>>,
}

impl AppServices {
    pub fn new(bridge: Bridge) -> Self {
        Self {
            bridge: Arc::new(bridge),
            tray: None,
            tray_show_item_id: None,
            tray_quit_item_id: None,
            window: None,
            app_hidden: false,
            _tray_task: None,
        }
    }
}

impl Global for AppServices {}

use std::path::PathBuf;

use gpui::*;
use gpui_component::{
    VirtualListScrollHandle,
    input::{InputEvent, InputState},
    resizable::ResizableState,
};
use rust_i18n::t;

use crate::{
    app::{AppPreferences, AppServices, WorkspaceModel},
    bridge::{ContainerAction, NetworkCreateConfig},
    i18n::AppLocale,
    ui::containers::{ContainerLogsPanel, ContainerShellPanel},
};

pub struct EchoApp {
    pub model: Entity<WorkspaceModel>,
    pub search_input: Entity<InputState>,
    pub image_search_input: Entity<InputState>,
    pub volume_search_input: Entity<InputState>,
    pub log_filter_input: Entity<InputState>,
    pub logs_panel: Entity<ContainerLogsPanel>,
    pub shell_panel: Entity<ContainerShellPanel>,
    pub container_layout: Entity<ResizableState>,
    pub container_scroll: VirtualListScrollHandle,
    pub focus_handle: FocusHandle,
    input_placeholder_locale: Option<AppLocale>,
    _model_subscription: Subscription,
    _search_subscription: Subscription,
    _image_search_subscription: Subscription,
    _volume_search_subscription: Subscription,
    _log_filter_subscription: Subscription,
}

impl EchoApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let bridge = cx.global::<AppServices>().bridge.clone();
        let preferences = AppPreferences::load();

        let model = cx.new(|cx| WorkspaceModel::new(bridge, preferences, cx));
        let _model_subscription = cx.observe(&model, |_, _, cx| cx.notify());
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("list.search_placeholder")));
        let image_search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("images.search_placeholder")));
        let volume_search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("volumes.search_placeholder")));
        let log_filter_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("detail.filter_logs")));
        let logs_panel = cx.new(|_| ContainerLogsPanel::new());
        let shell_panel = cx.new(|cx| ContainerShellPanel::new(model.clone(), cx));
        let container_layout = cx.new(|_| ResizableState::default());
        let container_scroll = VirtualListScrollHandle::new();
        let focus_handle = cx.focus_handle().tab_stop(false);
        let model_weak = model.downgrade();
        let _search_subscription = cx.subscribe_in(&search_input, window, {
            move |_, state, event: &InputEvent, _window, cx| match event {
                InputEvent::Change => {
                    let value = state.read(cx).value().to_string();
                    let _ = model_weak.update(cx, |model, cx| model.set_search_text(value, cx));
                }
                InputEvent::Focus | InputEvent::Blur | InputEvent::PressEnter { .. } => {}
            }
        });
        let model_weak = model.downgrade();
        let _image_search_subscription = cx.subscribe_in(&image_search_input, window, {
            move |_, state, event: &InputEvent, _window, cx| match event {
                InputEvent::Change => {
                    let value = state.read(cx).value().to_string();
                    let _ =
                        model_weak.update(cx, |model, cx| model.set_image_search_text(value, cx));
                }
                InputEvent::Focus | InputEvent::Blur | InputEvent::PressEnter { .. } => {}
            }
        });
        let model_weak = model.downgrade();
        let _volume_search_subscription = cx.subscribe_in(&volume_search_input, window, {
            move |_, state, event: &InputEvent, _window, cx| match event {
                InputEvent::Change => {
                    let value = state.read(cx).value().to_string();
                    let _ =
                        model_weak.update(cx, |model, cx| model.set_volume_search_text(value, cx));
                }
                InputEvent::Focus | InputEvent::Blur | InputEvent::PressEnter { .. } => {}
            }
        });
        let model_weak = model.downgrade();
        let _log_filter_subscription = cx.subscribe_in(&log_filter_input, window, {
            move |_, state, event: &InputEvent, _window, cx| match event {
                InputEvent::Change => {
                    let value = state.read(cx).value().to_string();
                    let _ = model_weak
                        .update(cx, |model, cx| model.set_container_log_filter(value, cx));
                }
                InputEvent::Focus | InputEvent::Blur | InputEvent::PressEnter { .. } => {}
            }
        });

        Self {
            model,
            search_input,
            image_search_input,
            volume_search_input,
            log_filter_input,
            logs_panel,
            shell_panel,
            container_layout,
            container_scroll,
            focus_handle,
            input_placeholder_locale: None,
            _model_subscription,
            _search_subscription,
            _image_search_subscription,
            _volume_search_subscription,
            _log_filter_subscription,
        }
    }

    pub fn start_container_sync(&mut self, cx: &mut Context<Self>) {
        self.model.update(cx, |model, cx| {
            model.start_connection_monitor(cx);
            model.start_container_sync(cx);
            model.start_resource_preload(cx);
        });
    }

    pub fn sync_input_placeholders(
        &mut self,
        locale: AppLocale,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.input_placeholder_locale == Some(locale) {
            return;
        }

        self.search_input.update(cx, |state, cx| {
            state.set_placeholder(t!("list.search_placeholder"), window, cx);
        });
        self.image_search_input.update(cx, |state, cx| {
            state.set_placeholder(t!("images.search_placeholder"), window, cx);
        });
        self.volume_search_input.update(cx, |state, cx| {
            state.set_placeholder(t!("volumes.search_placeholder"), window, cx);
        });
        self.log_filter_input.update(cx, |state, cx| {
            state.set_placeholder(t!("detail.filter_logs"), window, cx);
        });
        self.input_placeholder_locale = Some(locale);
    }

    pub fn control_container(
        &mut self,
        container_id: String,
        action: ContainerAction,
        cx: &mut Context<Self>,
    ) {
        self.model.update(cx, |model, cx| {
            model.control_container(container_id, action, cx);
        });
    }

    pub fn toggle_compose_project(&mut self, project: String, cx: &mut Context<Self>) {
        self.model.update(cx, |model, cx| {
            model.toggle_compose_project(project, cx);
        });
    }

    pub fn remove_image(&mut self, image_id: String, cx: &mut Context<Self>) {
        self.model.update(cx, |model, cx| {
            model.remove_image(image_id, cx);
        });
    }

    pub fn import_image(&mut self, archive_path: PathBuf, cx: &mut Context<Self>) {
        self.model.update(cx, |model, cx| {
            model.import_image(archive_path, cx);
        });
    }

    pub fn remove_volume(&mut self, volume_name: String, cx: &mut Context<Self>) {
        self.model.update(cx, |model, cx| {
            model.remove_volume(volume_name, cx);
        });
    }

    pub fn import_volume_archive(&mut self, archive_path: PathBuf, cx: &mut Context<Self>) {
        self.model.update(cx, |model, cx| {
            model.import_volume_archive(archive_path, cx);
        });
    }

    pub fn create_network(&mut self, config: NetworkCreateConfig, cx: &mut Context<Self>) {
        self.model.update(cx, |model, cx| {
            model.create_network(config, cx);
        });
    }

    pub fn remove_network(&mut self, network_id: String, cx: &mut Context<Self>) {
        self.model.update(cx, |model, cx| {
            model.remove_network(network_id, cx);
        });
    }
}

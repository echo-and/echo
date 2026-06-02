mod model;
mod preferences;
mod services;
mod tray;
mod workspace;

use gpui::actions;

actions!(echo, [HideEcho]);

pub use model::{ContainerDetailTab, WorkspaceModel};
pub use model::{
    NavSection, NetworkNodeSelection, PendingContainerAction, PendingImageAction,
    PendingNetworkAction, PendingVolumeAction,
};
pub use preferences::{AppFontFamily, AppPreferences};
pub use preferences::{
    MAX_CONTAINER_LIST_WIDTH, MIN_CONTAINER_LIST_WIDTH, clamp_container_list_width,
};
pub use services::AppServices;
pub use tray::{hide_echo_window, install_tray, open_echo_window, show_echo_window};
pub use workspace::EchoApp;

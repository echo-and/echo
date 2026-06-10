mod model;
mod preferences;
mod services;
mod tray;
mod updates;
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
#[cfg(target_os = "linux")]
pub use tray::hide_echo_window_from_window;
pub use tray::{hide_echo_window, install_tray, open_echo_window, show_echo_window};
pub use updates::{
    CURRENT_VERSION, GITHUB_LICENSE_URL, GITHUB_RELEASES_URL, GITHUB_REPOSITORY_URL, UpdateStatus,
    UpdateUnavailableReason,
};
pub use workspace::EchoApp;

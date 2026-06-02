rust_i18n::i18n!("locales", fallback = "en");

mod app;
mod assets;
mod bridge;
mod domain;
mod i18n;
mod ui;

use assets::EchoAssets;
use gpui::*;

fn main() {
    let application = gpui_platform::application()
        .with_assets(EchoAssets)
        .with_quit_mode(QuitMode::Explicit);
    application.on_reopen(app::show_echo_window);
    application.run(move |cx| {
        gpui_component::init(cx);
        cx.set_global(app::AppServices::new(
            bridge::Bridge::new().expect("failed to initialize Echo services"),
        ));

        if let Err(error) = app::install_tray(cx) {
            eprintln!("failed to install Echo tray icon: {error:#}");
        }
        cx.bind_keys([
            #[cfg(target_os = "macos")]
            KeyBinding::new("cmd-w", app::HideEcho, None),
            #[cfg(not(target_os = "macos"))]
            KeyBinding::new("ctrl-w", app::HideEcho, None),
        ]);
        cx.on_action(|_: &app::HideEcho, cx| app::hide_echo_window(cx));

        app::open_echo_window(cx).expect("failed to open Echo window");
    });
}

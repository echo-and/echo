# Changelog

## v0.1.4

- Fix Linux tray startup by initializing GTK before creating tray menus.
- Gracefully disable the Linux tray when appindicator setup fails so the main window can still open.
- Remove the unused Linux `libxdo` tray dependency and add Linux package smoke testing.
- Document Linux GTK and appindicator runtime dependencies.

## v0.1.3

- Fix Docker backend discovery in release builds and add manual backend selection in Settings.
- Refresh macOS release icon asset.

## v0.1.1

- Fix Windows release archives so Echo does not require a separately installed Visual C++ Runtime.

## v0.1.0 - MVP

- Initial MVP release of Echo.
- Browse local Docker containers, images, volumes, and networks.
- Inspect container metrics, status, logs, and shell sessions.
- Manage common container, image, volume, and network actions.
- Switch interface language, theme, and font preferences.

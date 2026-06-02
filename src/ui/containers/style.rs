use gpui::*;
use gpui_component::ThemeMode;

pub(super) fn metric_card_bg(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        hsla(0.0, 0.0, 0.184, 1.0)
    } else {
        hsla(0.0, 0.0, 0.965, 1.0)
    }
}

pub(super) fn tab_bar_bg(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        hsla(0.0, 0.0, 0.157, 1.0)
    } else {
        hsla(0.0, 0.0, 0.953, 1.0)
    }
}

pub(super) fn log_toolbar_bg(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        hsla(0.0, 0.0, 0.157, 1.0)
    } else {
        hsla(0.0, 0.0, 0.953, 1.0)
    }
}

pub(super) fn theme_log_text(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        hsla(0.0, 0.0, 0.867, 1.0)
    } else {
        hsla(0.0, 0.0, 0.145, 1.0)
    }
}

use gpui::*;
use gpui_component::{Theme, ThemeMode};

use crate::app::AppFontFamily;

pub(super) fn apply_echo_theme_overrides(cx: &mut App) {
    let mode = Theme::global(cx).mode;
    let border = theme_border(mode);
    let theme = Theme::global_mut(cx);
    if mode.is_dark() {
        let background = hsla(0.0, 0.0, 0.118, 1.0);
        theme.background = background;
        theme.popover = background;
    }
    theme.border = border;
    theme.input = border;
    theme.ring = theme_focus_border(mode);
    theme.tab_bar = if mode.is_dark() {
        hsla(0.0, 0.0, 0.192, 1.0)
    } else {
        hsla(0.0, 0.0, 0.953, 1.0)
    };
    theme.tab_active = if mode.is_dark() {
        hsla(0.0, 0.0, 0.157, 1.0)
    } else {
        hsla(0.0, 0.0, 1.0, 1.0)
    };
    theme.tab_foreground = theme_text(mode);
    theme.tab_active_foreground = theme_text(mode);
    if mode.is_dark() {
        theme.secondary = hsla(0.0, 0.0, 1.0, 0.125);
    }
    theme.secondary_hover = theme_sidebar_item_active_bg(mode);
    theme.secondary_active = theme_sidebar_item_active_bg(mode);
}

pub(super) fn apply_echo_font_preference(font_family: AppFontFamily, cx: &mut App) {
    Theme::global_mut(cx).font_family = font_family.family_name().into();
}

pub(super) fn theme_bg(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        hsla(0.0, 0.0, 0.157, 1.0)
    } else {
        hsla(0.0, 0.0, 0.961, 1.0)
    }
}

pub(super) fn theme_sidebar_bg(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        hsla(0.0, 0.0, 0.239, 1.0)
    } else {
        hsla(0.0, 0.0, 0.914, 1.0)
    }
}

pub(super) fn theme_sidebar_icon(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        hsla(0.0, 0.0, 0.557, 1.0)
    } else {
        hsla(0.0, 0.0, 0.427, 1.0)
    }
}

pub(super) fn theme_sidebar_icon_active(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        hsla(0.0, 0.0, 0.796, 1.0)
    } else {
        hsla(0.0, 0.0, 0.098, 1.0)
    }
}

pub(super) fn theme_sidebar_item_active_bg(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        hsla(0.0, 0.0, 1.0, 0.1)
    } else {
        hsla(0.0, 0.0, 0.0, 0.08)
    }
}

pub(super) fn theme_list_bg(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        hsla(0.0, 0.0, 0.184, 1.0)
    } else {
        hsla(0.0, 0.0, 0.953, 1.0)
    }
}

pub(super) fn theme_content_bg(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        hsla(0.0, 0.0, 0.157, 1.0)
    } else {
        hsla(0.0, 0.0, 1.0, 1.0)
    }
}

pub(super) fn theme_border(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        hsla(0.0, 0.0, 0.306, 1.0)
    } else {
        hsla(0.0, 0.0, 0.839, 1.0)
    }
}

fn theme_focus_border(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        hsla(0.0, 0.0, 0.420, 1.0)
    } else {
        hsla(0.0, 0.0, 0.700, 1.0)
    }
}

pub(super) fn theme_text(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        white()
    } else {
        hsla(0.0, 0.0, 0.098, 1.0)
    }
}

pub(super) fn theme_secondary(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        hsla(0.0, 0.0, 0.682, 1.0)
    } else {
        hsla(0.0, 0.0, 0.302, 1.0)
    }
}

pub(super) fn theme_muted(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        hsla(0.0, 0.0, 0.580, 1.0)
    } else {
        hsla(0.0, 0.0, 0.427, 1.0)
    }
}

pub(super) fn theme_error(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        hsla(0.0, 0.820, 0.620, 1.0)
    } else {
        hsla(0.0, 0.850, 0.590, 1.0)
    }
}

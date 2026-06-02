use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{StyledExt as _, ThemeMode, h_flex};

use crate::ui::{TOP_BAR_HEIGHT, theme::theme_border};

pub(in crate::ui) fn page_header(
    title: impl Into<SharedString>,
    actions: Option<AnyElement>,
    theme_mode: ThemeMode,
) -> impl IntoElement {
    h_flex()
        .h(TOP_BAR_HEIGHT)
        .flex_shrink_0()
        .items_center()
        .justify_between()
        .px_4()
        .border_b_1()
        .border_color(theme_border(theme_mode))
        .child(div().text_sm().font_medium().child(title.into()))
        .when_some(actions, |this, actions| this.child(actions))
}

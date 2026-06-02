use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{Icon, StyledExt as _, v_flex};
use rust_i18n::t;

use crate::{
    app::{EchoApp, NavSection, WorkspaceModel},
    ui::{
        ICON_SLIDERS_HORIZONTAL,
        snapshot::WorkspaceSnapshot,
        theme::{
            theme_border, theme_sidebar_bg, theme_sidebar_icon, theme_sidebar_icon_active,
            theme_sidebar_item_active_bg,
        },
    },
};

const APP_LOGO: &str = "assets/images/Logo.svg";
const ICON_BOX: &str = "assets/icons/box.svg";
const ICON_DISC_ALBUM: &str = "assets/icons/disc-album.svg";
const ICON_HARD_DRIVE: &str = "assets/icons/hard-drive.svg";
const ICON_NETWORK: &str = "assets/icons/network.svg";
const SIDEBAR_WIDTH: Pixels = px(68.);
const NAV_ICON_SIZE: Pixels = px(24.);
const NAV_ITEM_PADDING: Pixels = px(4.);

pub(super) fn sidebar(
    model: Entity<WorkspaceModel>,
    snapshot: &WorkspaceSnapshot,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    v_flex()
        .id("sidebar")
        .w(SIDEBAR_WIDTH)
        .h_full()
        .bg(theme_sidebar_bg(snapshot.theme_mode))
        .border_r_1()
        .border_color(theme_border(snapshot.theme_mode))
        .items_center()
        .justify_between()
        .child(
            v_flex()
                .items_center()
                .child(
                    div()
                        .mt(px(43.))
                        .size(px(32.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(img(APP_LOGO).size(px(32.)).object_fit(ObjectFit::Contain)),
                )
                .child(
                    v_flex()
                        .pt(px(30.))
                        .gap(px(20.))
                        .child(nav_menu_item(
                            model.clone(),
                            snapshot,
                            NavSection::Containers,
                        ))
                        .child(nav_menu_item(model.clone(), snapshot, NavSection::Images))
                        .child(nav_menu_item(model.clone(), snapshot, NavSection::Volumes))
                        .child(nav_menu_item(model.clone(), snapshot, NavSection::Networks)),
                ),
        )
        .child(settings_footer(model, snapshot, cx))
}

fn nav_menu_item(
    model: Entity<WorkspaceModel>,
    snapshot: &WorkspaceSnapshot,
    section: NavSection,
) -> impl IntoElement {
    nav_button(
        snapshot.active_nav == section,
        nav_button_id(section),
        nav_icon(section),
        theme_sidebar_icon(snapshot.theme_mode),
        theme_sidebar_icon_active(snapshot.theme_mode),
        theme_sidebar_item_active_bg(snapshot.theme_mode),
        move |cx| {
            model.update(cx, |model, cx| model.set_nav_section(section, cx));
        },
    )
}

fn nav_button(
    selected: bool,
    id: &'static str,
    icon: Icon,
    icon_color: Hsla,
    active_icon_color: Hsla,
    active_bg: Hsla,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let current_icon_color = if selected {
        active_icon_color
    } else {
        icon_color
    };

    div()
        .id(id)
        .p(NAV_ITEM_PADDING)
        .rounded(px(4.))
        .flex()
        .items_center()
        .justify_center()
        .text_color(current_icon_color)
        .cursor_pointer()
        .hover(move |this| this.text_color(active_icon_color))
        .when(selected, |this| {
            this.font_medium()
                .bg(active_bg)
                .text_color(active_icon_color)
        })
        .on_click(move |_, _, cx| {
            on_click(cx);
        })
        .child(icon.size(NAV_ICON_SIZE))
}

fn settings_footer(
    model: Entity<WorkspaceModel>,
    snapshot: &WorkspaceSnapshot,
    _cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let selected = snapshot.active_nav == NavSection::Settings;

    div()
        .w(SIDEBAR_WIDTH)
        .pb(px(20.))
        .flex()
        .items_center()
        .justify_center()
        .child(nav_button(
            selected,
            nav_button_id(NavSection::Settings),
            nav_icon(NavSection::Settings),
            theme_sidebar_icon(snapshot.theme_mode),
            theme_sidebar_icon_active(snapshot.theme_mode),
            theme_sidebar_item_active_bg(snapshot.theme_mode),
            move |cx| {
                model.update(cx, |model, cx| {
                    model.set_nav_section(NavSection::Settings, cx)
                });
            },
        ))
}

pub(super) fn nav_label(section: NavSection) -> SharedString {
    match section {
        NavSection::Containers => t!("nav.containers").to_string(),
        NavSection::Images => t!("nav.images").to_string(),
        NavSection::Volumes => t!("nav.volumes").to_string(),
        NavSection::Networks => t!("nav.networks").to_string(),
        NavSection::Settings => t!("nav.settings").to_string(),
    }
    .into()
}

fn nav_button_id(section: NavSection) -> &'static str {
    match section {
        NavSection::Containers => "nav-containers-hitbox",
        NavSection::Images => "nav-images-hitbox",
        NavSection::Volumes => "nav-volumes-hitbox",
        NavSection::Networks => "nav-networks-hitbox",
        NavSection::Settings => "nav-settings-hitbox",
    }
}

fn nav_icon(section: NavSection) -> Icon {
    match section {
        NavSection::Containers => Icon::new(Icon::empty()).path(ICON_BOX),
        NavSection::Images => Icon::new(Icon::empty()).path(ICON_DISC_ALBUM),
        NavSection::Volumes => Icon::new(Icon::empty()).path(ICON_HARD_DRIVE),
        NavSection::Networks => Icon::new(Icon::empty()).path(ICON_NETWORK),
        NavSection::Settings => Icon::new(Icon::empty()).path(ICON_SLIDERS_HORIZONTAL),
    }
}

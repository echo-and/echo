use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Sizable, Size, StyledExt, Theme, ThemeMode,
    button::Button,
    group_box::GroupBoxVariant,
    h_flex,
    label::Label,
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    v_flex,
};
use rust_i18n::t;

use crate::{
    app::{AppFontFamily, EchoApp, WorkspaceModel},
    i18n::AppLocale,
    ui::{
        ICON_SLIDERS_HORIZONTAL,
        header::page_header,
        sidebar::nav_label,
        snapshot::WorkspaceSnapshot,
        theme::{
            apply_echo_font_preference, apply_echo_theme_overrides, theme_content_bg, theme_list_bg,
        },
    },
};

const APP_LOGO: &str = "assets/images/Logo.svg";
const ICON_REFRESH_CW: &str = "assets/icons/refresh-cw.svg";
const DOCS_URL: &str = "https://echo.place/docs/getting-started";
const WEBSITE_URL: &str = "https://echo.place";
const LICENSE_URL: &str = "https://echo.place/license";

pub(super) fn settings_page(
    model: Entity<WorkspaceModel>,
    snapshot: &WorkspaceSnapshot,
    _cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let pages = settings_pages(model, snapshot);

    v_flex()
        .id("settings-page")
        .flex_1()
        .h_full()
        .overflow_hidden()
        .bg(theme_content_bg(snapshot.theme_mode))
        .child(page_header(
            nav_label(snapshot.active_nav),
            None,
            snapshot.theme_mode,
        ))
        .child(
            div().flex_1().overflow_hidden().child(
                Settings::new(format!("echo-settings-{}", snapshot.locale.code()))
                    .with_size(Size::Small)
                    .with_group_variant(GroupBoxVariant::Outline)
                    .sidebar_style(
                        &StyleRefinement::default().bg(theme_list_bg(snapshot.theme_mode)),
                    )
                    .pages(pages),
            ),
        )
}

fn settings_pages(model: Entity<WorkspaceModel>, snapshot: &WorkspaceSnapshot) -> Vec<SettingPage> {
    let theme_model = model.clone();
    let language_model = model.clone();
    let font_model = model.clone();
    let auto_check_model = model.clone();
    let notify_model = model;

    vec![
        SettingPage::new(t!("settings.general").to_string())
            .header_style(&StyleRefinement::default().hidden())
            .default_open(true)
            .resettable(false)
            .icon(Icon::new(Icon::empty()).path(ICON_SLIDERS_HORIZONTAL))
            .groups(vec![
                SettingGroup::new()
                    .title(t!("settings.appearance").to_string())
                    .items(vec![
                        SettingItem::new(
                            t!("settings.dark_mode").to_string(),
                            SettingField::switch(
                                {
                                    let theme_mode = snapshot.theme_mode;
                                    move |_| theme_mode.is_dark()
                                },
                                {
                                    let font_family = snapshot.font_family;
                                    move |enabled, cx| {
                                        let mode = if enabled {
                                            ThemeMode::Dark
                                        } else {
                                            ThemeMode::Light
                                        };
                                        theme_model.update(cx, |model, cx| {
                                            model.set_theme_mode(mode, cx);
                                        });
                                        Theme::change(mode, None, cx);
                                        apply_echo_theme_overrides(cx);
                                        apply_echo_font_preference(font_family, cx);
                                    }
                                },
                            )
                            .default_value(false),
                        )
                        .description(t!("settings.dark_mode_description").to_string()),
                        SettingItem::new(
                            t!("settings.app_language").to_string(),
                            SettingField::dropdown(
                                vec![
                                    (
                                        locale_setting_value(AppLocale::English),
                                        t!("settings.language_english").to_string().into(),
                                    ),
                                    (
                                        locale_setting_value(AppLocale::Chinese),
                                        t!("settings.language_chinese").to_string().into(),
                                    ),
                                ],
                                {
                                    let locale = snapshot.locale;
                                    move |_| locale_setting_value(locale)
                                },
                                move |value, cx| {
                                    let Some(locale) = locale_from_setting_value(value.as_ref())
                                    else {
                                        return;
                                    };
                                    language_model.update(cx, |model, cx| {
                                        model.set_locale(locale, cx);
                                    });
                                },
                            )
                            .default_value(locale_setting_value(AppLocale::English)),
                        )
                        .description(t!("settings.app_language_description").to_string()),
                        SettingItem::new(
                            t!("settings.font_family").to_string(),
                            SettingField::dropdown(
                                font_options(),
                                {
                                    let font_family = snapshot.font_family;
                                    move |_| font_setting_value(font_family)
                                },
                                move |value, cx| {
                                    let Some(font_family) =
                                        AppFontFamily::from_setting_value(value.as_ref())
                                    else {
                                        return;
                                    };
                                    font_model.update(cx, |model, cx| {
                                        model.set_font_family(font_family, cx);
                                    });
                                    apply_echo_font_preference(font_family, cx);
                                },
                            )
                            .default_value(font_setting_value(AppFontFamily::SystemDefault)),
                        )
                        .description(t!("settings.font_family_description").to_string()),
                    ]),
                SettingGroup::new()
                    .title(t!("settings.docker").to_string())
                    .items(vec![docker_socket_item(snapshot)]),
            ]),
        SettingPage::new(t!("settings.updates").to_string())
            .header_style(&StyleRefinement::default().hidden())
            .resettable(false)
            .icon(Icon::new(Icon::empty()).path(ICON_REFRESH_CW))
            .groups(vec![
                SettingGroup::new()
                    .title(t!("settings.updates").to_string())
                    .items(vec![
                        SettingItem::new(
                            t!("settings.auto_check_updates").to_string(),
                            SettingField::switch(
                                {
                                    let enabled = snapshot.auto_check_updates;
                                    move |_| enabled
                                },
                                move |enabled, cx| {
                                    auto_check_model.update(cx, |model, cx| {
                                        model.set_auto_check_updates(enabled, cx);
                                    });
                                },
                            )
                            .default_value(true),
                        )
                        .description(t!("settings.auto_check_updates_description").to_string()),
                        SettingItem::new(
                            t!("settings.notify_new_version").to_string(),
                            SettingField::switch(
                                {
                                    let enabled = snapshot.notify_new_version;
                                    move |_| enabled
                                },
                                move |enabled, cx| {
                                    notify_model.update(cx, |model, cx| {
                                        model.set_notify_new_version(enabled, cx);
                                    });
                                },
                            )
                            .default_value(true),
                        )
                        .description(t!("settings.notify_new_version_description").to_string()),
                    ]),
            ]),
        SettingPage::new(t!("settings.about").to_string())
            .header_style(&StyleRefinement::default().hidden())
            .resettable(false)
            .icon(Icon::new(IconName::Info))
            .groups(vec![
                SettingGroup::new().items(vec![about_intro_item()]),
                SettingGroup::new()
                    .title(t!("settings.links").to_string())
                    .items(vec![
                        link_setting_item(
                            t!("settings.documentation").to_string(),
                            t!("settings.documentation_description").to_string(),
                            DOCS_URL,
                            "settings-docs-link",
                        ),
                        link_setting_item(
                            t!("settings.website").to_string(),
                            t!("settings.website_description").to_string(),
                            WEBSITE_URL,
                            "settings-website-link",
                        ),
                        link_setting_item(
                            t!("settings.license").to_string(),
                            t!("settings.license_description").to_string(),
                            LICENSE_URL,
                            "settings-license-link",
                        ),
                    ]),
            ]),
    ]
}

fn about_intro_item() -> SettingItem {
    SettingItem::render(|_options, _window, cx| {
        v_flex()
            .w_full()
            .items_center()
            .justify_center()
            .gap_3()
            .py_4()
            .child(img(APP_LOGO).size(px(56.)).object_fit(ObjectFit::Contain))
            .child(Label::new("Echo").text_lg().font_semibold())
            .child(
                div().max_w(px(460.)).text_center().child(
                    Label::new(t!("settings.about_description").to_string())
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                ),
            )
    })
}

fn docker_socket_item(snapshot: &WorkspaceSnapshot) -> SettingItem {
    let name = docker_socket_display_name(
        &snapshot.active_connection_name,
        &snapshot.active_connection_endpoint,
    );
    let endpoint = docker_socket_address(&snapshot.active_connection_endpoint);
    let current = format!("Current: {endpoint}");

    SettingItem::render(move |options, _window, cx| {
        v_flex()
            .w_full()
            .gap_2()
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .justify_between()
                    .gap_3()
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(Label::new(t!("settings.docker_socket").to_string()).text_sm()),
                    )
                    .child(
                        Button::new("settings-docker-socket")
                            .label(name.clone())
                            .dropdown_caret(true)
                            .outline()
                            .with_size(options.size)
                            .disabled(true),
                    ),
            )
            .child(
                Label::new(current.clone())
                    .text_xs()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_color(cx.theme().muted_foreground),
            )
    })
}

fn link_setting_item(
    title: String,
    description: String,
    url: &'static str,
    button_id: &'static str,
) -> SettingItem {
    let button_label = t!("settings.open_link").to_string();

    SettingItem::new(
        title,
        SettingField::render(move |options, _window, _cx| {
            Button::new(button_id)
                .icon(IconName::ExternalLink)
                .label(button_label.clone())
                .outline()
                .with_size(options.size)
                .on_click(move |_, _window, cx| cx.open_url(url))
        }),
    )
    .description(description)
}

fn locale_setting_value(locale: AppLocale) -> SharedString {
    match locale {
        AppLocale::English => "en".into(),
        AppLocale::Chinese => "zh-CN".into(),
    }
}

fn locale_from_setting_value(value: &str) -> Option<AppLocale> {
    match value {
        "en" => Some(AppLocale::English),
        "zh-CN" => Some(AppLocale::Chinese),
        _ => None,
    }
}

fn font_options() -> Vec<(SharedString, SharedString)> {
    AppFontFamily::all()
        .iter()
        .copied()
        .map(|font_family| {
            (
                font_setting_value(font_family),
                font_family_label(font_family).into(),
            )
        })
        .collect()
}

fn font_setting_value(font_family: AppFontFamily) -> SharedString {
    font_family.setting_value().into()
}

fn font_family_label(font_family: AppFontFamily) -> String {
    match font_family {
        AppFontFamily::SystemDefault => t!("settings.font_system_default").to_string(),
        _ => font_family.display_name().to_string(),
    }
}

fn docker_socket_display_name(active_connection_name: &str, endpoint: &str) -> String {
    let normalized = endpoint.to_lowercase();

    if normalized.contains("orbstack") {
        return "OrbStack".to_string();
    }
    if normalized.contains("colima") {
        return "Colima".to_string();
    }
    if normalized.contains("lima") {
        return "Lima".to_string();
    }
    if normalized.ends_with("/var/run/docker.sock") || normalized.ends_with("/docker.sock") {
        return "Docker Engine".to_string();
    }

    active_connection_name.to_string()
}

fn docker_socket_address(endpoint: &str) -> String {
    endpoint
        .strip_prefix("unix://")
        .unwrap_or(endpoint)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{docker_socket_address, docker_socket_display_name};

    #[test]
    fn detects_known_socket_providers() {
        assert_eq!(
            docker_socket_display_name(
                "Docker host unix:///Users/me/.orbstack/run/docker.sock",
                "unix:///Users/me/.orbstack/run/docker.sock",
            ),
            "OrbStack"
        );
        assert_eq!(
            docker_socket_display_name(
                "Docker host unix:///Users/me/.colima/default/docker.sock",
                "unix:///Users/me/.colima/default/docker.sock",
            ),
            "Colima"
        );
        assert_eq!(
            docker_socket_display_name(
                "Docker host unix:///Users/me/.lima/docker/sock/docker.sock",
                "unix:///Users/me/.lima/docker/sock/docker.sock",
            ),
            "Lima"
        );
    }

    #[test]
    fn falls_back_to_connection_name_for_unknown_socket() {
        assert_eq!(
            docker_socket_display_name("Current Docker context", "Docker defaults"),
            "Current Docker context"
        );
    }

    #[test]
    fn strips_unix_scheme_from_socket_address() {
        assert_eq!(
            docker_socket_address("unix:///var/run/docker.sock"),
            "/var/run/docker.sock"
        );
        assert_eq!(docker_socket_address("Docker defaults"), "Docker defaults");
    }
}

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{
    ActiveTheme as _, Disableable, Icon, IconName, Sizable, ThemeMode, WindowExt,
    button::{Button, ButtonVariant, ButtonVariants},
    dialog::DialogButtonProps,
    h_flex,
    input::Input,
    scroll::ScrollableElement as _,
    skeleton::Skeleton,
    v_flex,
};
use rust_i18n::t;

use crate::{
    app::EchoApp,
    domain::VolumeSummary,
    ui::{
        header::page_header,
        sidebar::nav_label,
        snapshot::WorkspaceSnapshot,
        theme::{theme_border, theme_content_bg, theme_muted, theme_secondary, theme_text},
    },
};

const VOLUME_ICON: &str = "assets/images/list-icons/List-Volume-Icon.svg";
const ICON_TRASH_2: &str = "assets/icons/trash-2.svg";

pub(super) fn volumes_page(
    app: &mut EchoApp,
    snapshot: &WorkspaceSnapshot,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    v_flex()
        .id("volumes-page")
        .flex_1()
        .min_h_0()
        .h_full()
        .overflow_hidden()
        .bg(theme_content_bg(snapshot.theme_mode))
        .child(page_header(
            nav_label(snapshot.active_nav),
            Some(volumes_header_actions(app, snapshot, cx).into_any_element()),
            snapshot.theme_mode,
        ))
        .child(volumes_body(snapshot, cx))
}

fn volumes_header_actions(
    app: &EchoApp,
    snapshot: &WorkspaceSnapshot,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let search = Input::new(&app.volume_search_input)
        .tab_index(-1)
        .prefix(
            Icon::new(IconName::Search)
                .small()
                .text_color(theme_muted(snapshot.theme_mode)),
        )
        .small()
        .w(px(210.))
        .text_color(theme_text(snapshot.theme_mode));

    h_flex().items_center().gap(px(10.)).child(search).child(
        Button::new("volumes-import")
            .outline()
            .icon(IconName::Plus)
            .label(t!("volumes.import"))
            .small()
            .loading(snapshot.is_volume_importing)
            .disabled(snapshot.is_volume_importing || snapshot.pending_volume_action.is_some())
            .on_click(cx.listener(|_, _, window, cx| {
                open_import_dialog(window, cx);
            })),
    )
}

fn open_import_dialog(window: &mut Window, cx: &mut Context<EchoApp>) {
    let paths = cx.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some(t!("volumes.import_prompt").to_string().into()),
    });
    let app = cx.entity().downgrade();

    cx.spawn_in(window, async move |_, window| {
        let path = paths.await.ok()?.ok()??.into_iter().next()?;

        window
            .update(|_, cx| {
                let _ = app.update(cx, |app, cx| {
                    app.import_volume_archive(path.clone(), cx);
                });
            })
            .ok()
    })
    .detach();
}

fn volumes_body(snapshot: &WorkspaceSnapshot, cx: &mut Context<EchoApp>) -> AnyElement {
    if snapshot.is_volumes_loading && snapshot.filtered_volumes.is_empty() {
        return volume_skeleton_body(cx);
    }

    if snapshot.filtered_volumes.is_empty() {
        return centered_message(t!("volumes.empty"), snapshot.theme_mode);
    }

    let mut in_use = Vec::new();
    let mut unused = Vec::new();
    for volume in snapshot.filtered_volumes.iter().cloned() {
        if volume.ref_count.unwrap_or_default() > 0 {
            in_use.push(volume);
        } else {
            unused.push(volume);
        }
    }

    div()
        .flex_1()
        .min_h_0()
        .overflow_hidden()
        .child(
            div().size_full().overflow_y_scrollbar().child(
                v_flex()
                    .w_full()
                    .min_h_0()
                    .p(px(16.))
                    .gap(px(12.))
                    .when_some(snapshot.volume_error.clone(), |this, error| {
                        this.child(error_banner(error, snapshot.theme_mode))
                    })
                    .when(!in_use.is_empty(), |this| {
                        this.child(volume_group(
                            t!("volumes.in_use"),
                            in_use,
                            snapshot.theme_mode,
                            snapshot
                                .pending_volume_action
                                .as_ref()
                                .map(|pending| pending.volume_name.clone()),
                            cx,
                        ))
                    })
                    .when(!unused.is_empty(), |this| {
                        this.child(volume_group(
                            t!("volumes.unused"),
                            unused,
                            snapshot.theme_mode,
                            snapshot
                                .pending_volume_action
                                .as_ref()
                                .map(|pending| pending.volume_name.clone()),
                            cx,
                        ))
                    }),
            ),
        )
        .into_any_element()
}

fn volume_skeleton_body(cx: &mut Context<EchoApp>) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .overflow_hidden()
        .child(
            div().size_full().overflow_y_scrollbar().child(
                v_flex()
                    .w_full()
                    .min_h_0()
                    .p(px(16.))
                    .gap(px(12.))
                    .child(skeleton_group(cx)),
            ),
        )
        .into_any_element()
}

fn skeleton_group(cx: &mut Context<EchoApp>) -> impl IntoElement {
    let cards = (0..6)
        .map(|index| skeleton_card(index, cx).into_any_element())
        .collect::<Vec<_>>();

    v_flex()
        .w_full()
        .gap(px(10.))
        .child(
            h_flex()
                .items_end()
                .gap(px(6.))
                .child(Skeleton::new().w(px(104.)).h_4().rounded(cx.theme().radius))
                .child(
                    Skeleton::new()
                        .secondary()
                        .w(px(48.))
                        .h_3()
                        .rounded(cx.theme().radius),
                ),
        )
        .child(div().grid().grid_cols(3).gap(px(10.)).children(cards))
}

fn skeleton_card(index: usize, cx: &mut Context<EchoApp>) -> impl IntoElement {
    h_flex()
        .id(("volume-skeleton-card", index))
        .min_w(px(180.))
        .h(px(66.))
        .items_center()
        .gap(px(12.))
        .p(px(12.))
        .rounded(px(4.))
        .border_1()
        .border_color(cx.theme().border)
        .overflow_hidden()
        .child(Skeleton::new().size(px(40.)).rounded(px(8.)))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(4.))
                .child(Skeleton::new().h_4().w_full().rounded(cx.theme().radius))
                .child(
                    Skeleton::new()
                        .secondary()
                        .h_4()
                        .w(px(150.))
                        .max_w_full()
                        .rounded(cx.theme().radius),
                ),
        )
}

fn volume_group(
    title: impl Into<SharedString>,
    volumes: Vec<VolumeSummary>,
    theme_mode: ThemeMode,
    pending_volume_name: Option<String>,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let count = volumes.len();
    let size = volumes
        .iter()
        .filter_map(|volume| volume.size_bytes)
        .sum::<u64>();
    let cards = volumes
        .into_iter()
        .map(|volume| {
            volume_card(
                VolumeRowVm::from(&volume),
                theme_mode,
                pending_volume_name.as_deref() == Some(&volume.name),
                cx,
            )
            .into_any_element()
        })
        .collect::<Vec<_>>();

    v_flex()
        .w_full()
        .gap(px(10.))
        .child(
            h_flex()
                .items_end()
                .gap(px(6.))
                .child(
                    div()
                        .text_sm()
                        .line_height(relative(1.2))
                        .child(format!("{} ({count})", title.into())),
                )
                .child(
                    div()
                        .text_xs()
                        .line_height(relative(1.2))
                        .text_color(theme_secondary(theme_mode))
                        .child(format_bytes(size)),
                ),
        )
        .child(div().grid().grid_cols(3).gap(px(10.)).children(cards))
}

fn volume_card(
    volume: VolumeRowVm,
    theme_mode: ThemeMode,
    pending: bool,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let group_id = volume.name.clone();
    let delete_volume = volume.clone();

    h_flex()
        .id(("volume-card", volume.index))
        .group(group_id.clone())
        .min_w(px(180.))
        .h(px(66.))
        .items_center()
        .justify_between()
        .gap(px(12.))
        .p(px(12.))
        .rounded(px(4.))
        .border_1()
        .border_color(theme_border(theme_mode))
        .bg(card_bg(theme_mode))
        .overflow_hidden()
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap(px(12.))
                .child(
                    img(VOLUME_ICON)
                        .size(px(40.))
                        .rounded(px(8.))
                        .object_fit(ObjectFit::Contain),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(4.))
                        .child(
                            div()
                                .text_size(px(12.))
                                .line_height(relative(1.2))
                                .truncate()
                                .child(volume.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .line_height(relative(1.2))
                                .text_color(theme_secondary(theme_mode))
                                .truncate()
                                .child(format!("{}, {}", volume.size_label, volume.created_label)),
                        ),
                ),
        )
        .child(
            h_flex()
                .flex_shrink_0()
                .items_center()
                .gap(px(2.))
                .invisible()
                .group_hover(group_id, |this| this.visible())
                .child(
                    Button::new(("volume-delete", volume.index))
                        .ghost()
                        .icon(Icon::new(Icon::empty()).path(ICON_TRASH_2))
                        .tooltip(t!("volumes.delete"))
                        .xsmall()
                        .loading(pending)
                        .disabled(pending)
                        .on_click(cx.listener(move |app, _, window, cx| {
                            open_delete_dialog(app, delete_volume.clone(), window, cx);
                        })),
                ),
        )
}

fn open_delete_dialog(
    _app: &mut EchoApp,
    volume: VolumeRowVm,
    window: &mut Window,
    cx: &mut Context<EchoApp>,
) {
    let volume_name = volume.name.clone();
    let app = cx.entity().downgrade();

    window.open_alert_dialog(cx, move |dialog, _, _| {
        let remove_app = app.clone();
        dialog
            .confirm()
            .title(t!("volumes.delete_title"))
            .description(t!("volumes.delete_description", name = volume.name.clone()))
            .button_props(
                DialogButtonProps::default()
                    .ok_text(t!("volumes.delete_confirm"))
                    .ok_variant(ButtonVariant::Danger)
                    .cancel_text(t!("volumes.delete_cancel"))
                    .show_cancel(true),
            )
            .on_ok({
                let volume_name = volume_name.clone();
                move |_, _, cx| {
                    let _ = remove_app.update(cx, |app, cx| {
                        app.remove_volume(volume_name.clone(), cx);
                    });
                    true
                }
            })
    });
}

fn error_banner(error: String, theme_mode: ThemeMode) -> impl IntoElement {
    div()
        .w_full()
        .p(px(10.))
        .rounded(px(4.))
        .border_1()
        .border_color(theme_border(theme_mode))
        .text_sm()
        .line_height(relative(1.35))
        .text_color(theme_text(theme_mode))
        .child(error)
}

fn centered_message(text: impl Into<SharedString>, theme_mode: ThemeMode) -> AnyElement {
    div()
        .flex()
        .flex_1()
        .min_h_0()
        .items_center()
        .justify_center()
        .px_4()
        .text_sm()
        .text_center()
        .text_color(theme_secondary(theme_mode))
        .child(text.into())
        .into_any_element()
}

fn card_bg(theme_mode: ThemeMode) -> Hsla {
    if theme_mode.is_dark() {
        hsla(0.0, 0.0, 0.184, 1.0)
    } else {
        hsla(0.0, 0.0, 0.973, 1.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VolumeRowVm {
    name: String,
    index: u64,
    mountpoint: String,
    size_label: String,
    created_label: String,
}

impl From<&VolumeSummary> for VolumeRowVm {
    fn from(volume: &VolumeSummary) -> Self {
        Self {
            name: volume.name.clone(),
            index: index_from_name(&volume.name),
            mountpoint: volume.mountpoint.clone(),
            size_label: volume
                .size_bytes
                .map(format_bytes)
                .unwrap_or_else(|| t!("volumes.size_unknown").to_string()),
            created_label: format_created(volume.created_at),
        }
    }
}

fn format_created(created_at: Option<std::time::SystemTime>) -> String {
    let Some(created_at) = created_at else {
        return t!("volumes.created_unknown").to_string();
    };

    let Ok(elapsed) = std::time::SystemTime::now().duration_since(created_at) else {
        return t!("volumes.created_recently").to_string();
    };

    let days = elapsed.as_secs() / 86_400;
    if days >= 365 {
        t!("volumes.created_years", count = days / 365).to_string()
    } else if days >= 30 {
        t!("volumes.created_months", count = days / 30).to_string()
    } else if days >= 1 {
        t!("volumes.created_days", count = days).to_string()
    } else {
        let hours = elapsed.as_secs() / 3_600;
        if hours >= 1 {
            t!("volumes.created_hours", count = hours).to_string()
        } else {
            t!("volumes.created_recently").to_string()
        }
    }
}

fn index_from_name(name: &str) -> u64 {
    name.bytes().fold(0_u64, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as u64)
    })
}

fn format_bytes(bytes: u64) -> String {
    let (value, unit) = format_bytes_value(bytes as f64);
    format!("{}{}", value, unit)
}

fn format_bytes_value(bytes: f64) -> (String, String) {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes.max(0.);
    let mut unit = UNITS[0];

    for next_unit in UNITS.iter().skip(1) {
        if value < 1024. {
            break;
        }
        value /= 1024.;
        unit = next_unit;
    }

    (
        format_number(value, if value >= 10. { 1 } else { 2 }),
        unit.to_string(),
    )
}

fn format_number(value: f64, decimals: usize) -> String {
    let formatted = format!("{:.*}", decimals, value);
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use rust_i18n::t;

    use crate::domain::VolumeSummary;

    use super::{VolumeRowVm, format_created};

    #[test]
    fn maps_volume_to_row() {
        let row = VolumeRowVm::from(&VolumeSummary::new(
            "db-data".to_string(),
            "local".to_string(),
            "/var/lib/docker/volumes/db-data/_data".to_string(),
            Some(2048),
            None,
            Some(1),
        ));

        assert_eq!(row.name, "db-data");
        assert_eq!(row.size_label, "2KB");
        assert_eq!(row.created_label, t!("volumes.created_unknown").to_string());
    }

    #[test]
    fn formats_created_age() {
        assert_eq!(
            format_created(None),
            t!("volumes.created_unknown").to_string()
        );
        assert_eq!(
            format_created(Some(SystemTime::now() - Duration::from_secs(90))),
            t!("volumes.created_recently").to_string()
        );
        assert_eq!(
            format_created(Some(SystemTime::now() - Duration::from_secs(3 * 86_400))),
            t!("volumes.created_days", count = 3).to_string()
        );
    }
}

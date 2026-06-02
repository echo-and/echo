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
    domain::ImageSummary,
    ui::{
        docker_icons::{DockerIconState, DockerIconStyle, docker_icon_style_for_reference},
        header::page_header,
        sidebar::nav_label,
        snapshot::WorkspaceSnapshot,
        theme::{theme_border, theme_content_bg, theme_muted, theme_secondary, theme_text},
    },
};

const IMAGE_ICON: &str = "assets/images/list-icons/List-Image-Icon.svg";
const ICON_TRASH_2: &str = "assets/icons/trash-2.svg";

pub(super) fn images_page(
    app: &mut EchoApp,
    snapshot: &WorkspaceSnapshot,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    v_flex()
        .id("images-page")
        .flex_1()
        .min_h_0()
        .h_full()
        .overflow_hidden()
        .bg(theme_content_bg(snapshot.theme_mode))
        .child(page_header(
            nav_label(snapshot.active_nav),
            Some(images_header_actions(app, snapshot, cx).into_any_element()),
            snapshot.theme_mode,
        ))
        .child(images_body(snapshot, cx))
}

fn images_header_actions(
    app: &EchoApp,
    snapshot: &WorkspaceSnapshot,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let search = Input::new(&app.image_search_input)
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
        Button::new("images-import")
            .outline()
            .icon(IconName::Plus)
            .label(t!("images.import"))
            .small()
            .loading(snapshot.is_image_importing)
            .disabled(snapshot.is_image_importing || snapshot.pending_image_action.is_some())
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
        prompt: Some(t!("images.import_prompt").to_string().into()),
    });
    let app = cx.entity().downgrade();

    cx.spawn_in(window, async move |_, window| {
        let path = paths.await.ok()?.ok()??.into_iter().next()?;

        window
            .update(|_, cx| {
                let _ = app.update(cx, |app, cx| {
                    app.import_image(path.clone(), cx);
                });
            })
            .ok()
    })
    .detach();
}

fn images_body(snapshot: &WorkspaceSnapshot, cx: &mut Context<EchoApp>) -> AnyElement {
    if snapshot.is_images_loading && snapshot.filtered_images.is_empty() {
        return image_skeleton_body(cx);
    }

    if snapshot.filtered_images.is_empty() {
        return centered_message(t!("images.empty"), snapshot.theme_mode);
    }

    let mut in_use = Vec::new();
    let mut unused = Vec::new();
    for image in snapshot.filtered_images.iter().cloned() {
        if image.containers.unwrap_or_default() > 0 {
            in_use.push(image);
        } else {
            unused.push(image);
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
                    .when_some(snapshot.image_error.clone(), |this, error| {
                        this.child(error_banner(error, snapshot.theme_mode))
                    })
                    .when(!in_use.is_empty(), |this| {
                        this.child(image_group(
                            t!("images.in_use"),
                            in_use,
                            snapshot.theme_mode,
                            snapshot
                                .pending_image_action
                                .as_ref()
                                .map(|pending| pending.image_id.clone()),
                            cx,
                        ))
                    })
                    .when(!unused.is_empty(), |this| {
                        this.child(image_group(
                            t!("images.unused"),
                            unused,
                            snapshot.theme_mode,
                            snapshot
                                .pending_image_action
                                .as_ref()
                                .map(|pending| pending.image_id.clone()),
                            cx,
                        ))
                    }),
            ),
        )
        .into_any_element()
}

fn image_skeleton_body(cx: &mut Context<EchoApp>) -> AnyElement {
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
                .child(Skeleton::new().w(px(96.)).h_4().rounded(cx.theme().radius))
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
        .id(("image-skeleton-card", index))
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

fn image_group(
    title: impl Into<SharedString>,
    images: Vec<ImageSummary>,
    theme_mode: ThemeMode,
    pending_image_id: Option<String>,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let count = images.len();
    let size = images.iter().map(|image| image.size_bytes).sum::<u64>();
    let cards = images
        .into_iter()
        .map(|image| {
            image_card(
                ImageRowVm::from_image(&image, theme_mode),
                theme_mode,
                pending_image_id.as_deref() == Some(&image.id),
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

fn image_card(
    image: ImageRowVm,
    theme_mode: ThemeMode,
    pending: bool,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let group_id = image.id.clone();
    let delete_image = image.clone();

    h_flex()
        .id(("image-card", image.index))
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
                    div()
                        .size(px(40.))
                        .flex_shrink_0()
                        .rounded(px(8.))
                        .overflow_hidden()
                        .when_some(image.icon_background, |this, background| {
                            this.bg(background)
                        })
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            img(image.icon_path)
                                .size(image_icon_size(image.icon_path))
                                .rounded(px(8.))
                                .grayscale(image.icon_grayscale)
                                .object_fit(ObjectFit::Contain),
                        ),
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
                                .child(image.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .line_height(relative(1.2))
                                .text_color(theme_secondary(theme_mode))
                                .truncate()
                                .child(format!("{}, {}", image.size_label, image.created_label)),
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
                    Button::new(("image-delete", image.index))
                        .ghost()
                        .icon(Icon::new(Icon::empty()).path(ICON_TRASH_2))
                        .tooltip(t!("images.delete"))
                        .xsmall()
                        .loading(pending)
                        .disabled(pending)
                        .on_click(cx.listener(move |app, _, window, cx| {
                            open_delete_dialog(app, delete_image.clone(), window, cx);
                        })),
                ),
        )
}

fn open_delete_dialog(
    _app: &mut EchoApp,
    image: ImageRowVm,
    window: &mut Window,
    cx: &mut Context<EchoApp>,
) {
    let image_id = image.id.clone();
    let app = cx.entity().downgrade();

    window.open_alert_dialog(cx, move |dialog, _, _| {
        let remove_app = app.clone();
        dialog
            .confirm()
            .title(t!("images.delete_title"))
            .description(t!("images.delete_description", name = image.name.clone()))
            .button_props(
                DialogButtonProps::default()
                    .ok_text(t!("images.delete_confirm"))
                    .ok_variant(ButtonVariant::Danger)
                    .cancel_text(t!("images.delete_cancel"))
                    .show_cancel(true),
            )
            .on_ok({
                let image_id = image_id.clone();
                move |_, _, cx| {
                    let _ = remove_app.update(cx, |app, cx| {
                        app.remove_image(image_id.clone(), cx);
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
struct ImageRowVm {
    id: String,
    index: u64,
    name: String,
    icon_path: &'static str,
    icon_background: Option<Hsla>,
    icon_grayscale: bool,
    size_label: String,
    created_label: String,
}

impl From<&ImageSummary> for ImageRowVm {
    fn from(image: &ImageSummary) -> Self {
        Self::from_image(image, ThemeMode::Light)
    }
}

impl ImageRowVm {
    fn from_image(image: &ImageSummary, theme_mode: ThemeMode) -> Self {
        let icon_style = image_icon_style(image, theme_mode);

        Self {
            id: image.id.clone(),
            index: index_from_id(&image.id),
            name: image_display_name(image),
            icon_path: icon_style.path,
            icon_background: icon_style.background,
            icon_grayscale: icon_style.grayscale,
            size_label: format_bytes(image.size_bytes),
            created_label: format_created(image.created_at),
        }
    }
}

fn image_icon_style(image: &ImageSummary, theme_mode: ThemeMode) -> DockerIconStyle {
    image
        .repo_tags
        .iter()
        .chain(image.repo_digests.iter())
        .find_map(|reference| {
            let icon_style = docker_icon_style_for_reference(
                reference,
                IMAGE_ICON,
                theme_mode,
                DockerIconState::Normal,
            );
            (icon_style.path != IMAGE_ICON).then_some(icon_style)
        })
        .unwrap_or(DockerIconStyle {
            path: IMAGE_ICON,
            background: None,
            grayscale: false,
        })
}

fn image_icon_size(path: &str) -> Pixels {
    if path.starts_with("assets/images/docker-icons/") {
        px(32.)
    } else {
        px(40.)
    }
}

fn image_display_name(image: &ImageSummary) -> String {
    image
        .repo_tags
        .iter()
        .find(|tag| !tag.is_empty() && tag.as_str() != "<none>:<none>")
        .or_else(|| image.repo_digests.iter().find(|digest| !digest.is_empty()))
        .cloned()
        .unwrap_or_else(|| short_id(&image.id))
}

fn format_created(created_at: Option<std::time::SystemTime>) -> String {
    let Some(created_at) = created_at else {
        return t!("images.created_unknown").to_string();
    };

    let Ok(elapsed) = std::time::SystemTime::now().duration_since(created_at) else {
        return t!("images.created_recently").to_string();
    };

    let days = elapsed.as_secs() / 86_400;
    if days >= 365 {
        t!("images.created_years", count = days / 365).to_string()
    } else if days >= 30 {
        t!("images.created_months", count = days / 30).to_string()
    } else if days >= 1 {
        t!("images.created_days", count = days).to_string()
    } else {
        let hours = elapsed.as_secs() / 3_600;
        if hours >= 1 {
            t!("images.created_hours", count = hours).to_string()
        } else {
            t!("images.created_recently").to_string()
        }
    }
}

fn index_from_id(id: &str) -> u64 {
    let id = id.strip_prefix("sha256:").unwrap_or(id);
    u64::from_str_radix(id.chars().take(12).collect::<String>().as_str(), 16).unwrap_or(0)
}

fn short_id(id: &str) -> String {
    id.strip_prefix("sha256:")
        .unwrap_or(id)
        .chars()
        .take(12)
        .collect()
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

    use crate::domain::ImageSummary;
    use gpui::px;
    use rust_i18n::t;

    use super::{IMAGE_ICON, ImageRowVm, format_created, image_display_name, image_icon_size};

    #[test]
    fn chooses_image_display_name() {
        let tagged = image(&["postgres:latest"], &[]);
        assert_eq!(image_display_name(&tagged), "postgres:latest");

        let digested = image(&[], &["postgres@sha256:abc"]);
        assert_eq!(image_display_name(&digested), "postgres@sha256:abc");

        let anonymous = image(&[], &[]);
        assert_eq!(image_display_name(&anonymous), "1234567890ab");
    }

    #[test]
    fn maps_image_to_row() {
        let row = ImageRowVm::from(&ImageSummary::new(
            "sha256:1234567890abcdef".to_string(),
            vec!["redis:7".to_string()],
            Vec::new(),
            1024,
            None,
            Some(1),
        ));

        assert_eq!(row.name, "redis:7");
        assert_eq!(row.icon_path, "assets/images/docker-icons/redis.png");
        assert_eq!(row.size_label, "1KB");
        assert_eq!(row.created_label, t!("images.created_unknown").to_string());
    }

    #[test]
    fn falls_back_for_non_official_image_icon() {
        let row = ImageRowVm::from(&ImageSummary::new(
            "sha256:1234567890abcdef".to_string(),
            vec!["private/redis:7".to_string()],
            Vec::new(),
            1024,
            None,
            Some(1),
        ));

        assert_eq!(row.icon_path, IMAGE_ICON);
    }

    #[test]
    fn fallback_image_icon_renders_at_full_list_icon_size() {
        assert_eq!(
            image_icon_size("assets/images/docker-icons/redis.png"),
            px(32.)
        );
        assert_eq!(image_icon_size(IMAGE_ICON), px(40.));
    }

    #[test]
    fn formats_created_age() {
        assert_eq!(
            format_created(None),
            t!("images.created_unknown").to_string()
        );
        assert_eq!(
            format_created(Some(SystemTime::now() - Duration::from_secs(90))),
            t!("images.created_recently").to_string()
        );
        assert_eq!(
            format_created(Some(SystemTime::now() - Duration::from_secs(3 * 86_400))),
            t!("images.created_days", count = 3).to_string()
        );
    }

    fn image(tags: &[&str], digests: &[&str]) -> ImageSummary {
        ImageSummary::new(
            "sha256:1234567890abcdef".to_string(),
            tags.iter().map(|tag| tag.to_string()).collect(),
            digests.iter().map(|digest| digest.to_string()).collect(),
            0,
            None,
            None,
        )
    }
}

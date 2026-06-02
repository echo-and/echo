use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
    rc::Rc,
};

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{
    Icon, IconName, Sizable, StyledExt as _, ThemeMode, VirtualListScrollHandle, h_flex,
    input::Input, v_flex, v_virtual_list,
};
use rust_i18n::t;

use crate::{
    app::EchoApp,
    domain::ContainerSummary,
    ui::{
        TOP_BAR_HEIGHT,
        containers::format::short_id,
        docker_icons::{DockerIconState, DockerIconStyle, docker_icon_style_for_reference},
        snapshot::WorkspaceSnapshot,
        theme::{
            theme_border, theme_error, theme_list_bg, theme_muted, theme_secondary, theme_text,
        },
    },
};

const CONTAINER_ICON: &str = "assets/images/list-icons/List-Container-Icon.svg";
const CONTAINER_ICON_INACTIVE: &str = "assets/images/list-icons/List-Container-Icon-Inactive.svg";
const COMPOSE_ICON: &str = "assets/images/list-icons/List-Compose-Icon.svg";
const COMPOSE_ICON_INACTIVE: &str = "assets/images/list-icons/List-Compose-Icon-Inactive.svg";
const CHILD_ICON: &str = "assets/images/list-icons/List-Child-Icon.svg";
const LIST_ROW_PADDING_LEFT: Pixels = px(16.);
const CONTAINER_ROW_HEIGHT: Pixels = px(73.);
const COMPOSE_CHILD_ROW_HEIGHT: Pixels = px(57.);

pub(in crate::ui) fn list_panel(
    app: &mut EchoApp,
    snapshot: &WorkspaceSnapshot,
    row_sizes: Rc<Vec<Size<Pixels>>>,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let input = Input::new(&app.search_input)
        .tab_index(-1)
        .prefix(
            Icon::new(IconName::Search)
                .small()
                .text_color(theme_muted(snapshot.theme_mode)),
        )
        .small()
        .w_full()
        .text_color(theme_text(snapshot.theme_mode));

    v_flex()
        .size_full()
        .overflow_hidden()
        .bg(theme_list_bg(snapshot.theme_mode))
        .child(
            div()
                .flex()
                .h(TOP_BAR_HEIGHT)
                .px_3()
                .items_center()
                .border_b_1()
                .border_color(theme_border(snapshot.theme_mode))
                .child(input),
        )
        .child(
            div()
                .id("container-list-pane")
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .child(container_list(
                    snapshot,
                    row_sizes,
                    app.container_scroll.clone(),
                    cx,
                )),
        )
}

pub(in crate::ui) fn container_list_row_sizes(
    snapshot: &WorkspaceSnapshot,
) -> Rc<Vec<Size<Pixels>>> {
    Rc::new(
        container_list_items(
            &snapshot.filtered_containers,
            &snapshot.selected_container_id,
            &snapshot.search_text,
            &snapshot.expanded_compose_projects,
        )
        .iter()
        .map(container_list_row_size)
        .collect(),
    )
}

fn container_list_row_size(item: &ContainerListItem) -> Size<Pixels> {
    size(px(1.), item.height())
}

fn container_list(
    snapshot: &WorkspaceSnapshot,
    row_sizes: Rc<Vec<Size<Pixels>>>,
    scroll_handle: VirtualListScrollHandle,
    cx: &mut Context<EchoApp>,
) -> impl IntoElement {
    let items = container_list_items(
        &snapshot.filtered_containers,
        &snapshot.selected_container_id,
        &snapshot.search_text,
        &snapshot.expanded_compose_projects,
    );

    if items.is_empty() {
        return div()
            .flex()
            .size_full()
            .items_center()
            .justify_center()
            .px_4()
            .text_sm()
            .text_center()
            .text_color(theme_muted(snapshot.theme_mode))
            .child(t!("list.empty"))
            .into_any_element();
    }

    let items = Rc::new(items);
    let selected_container_id = snapshot.selected_container_id.clone();
    let theme_mode = snapshot.theme_mode;

    v_virtual_list(
        cx.entity().clone(),
        "container-list",
        row_sizes,
        move |_, range: Range<usize>, _, cx| {
            range
                .map(|index| {
                    let item = items[index].clone();
                    let next = items.get(index + 1);
                    container_list_item_row(
                        item,
                        next,
                        selected_container_id.clone(),
                        theme_mode,
                        cx,
                    )
                })
                .collect()
        },
    )
    .track_scroll(&scroll_handle)
    .overflow_x_hidden()
    .into_any_element()
}

fn container_list_item_row(
    item: ContainerListItem,
    next: Option<&ContainerListItem>,
    selected_container_id: Option<String>,
    theme_mode: ThemeMode,
    cx: &mut Context<EchoApp>,
) -> AnyElement {
    match item {
        ContainerListItem::Container(container) => container_row(
            container,
            selected_container_id,
            theme_mode,
            false,
            true,
            cx,
        ),
        ContainerListItem::ComposeProject(project) => {
            let has_bottom_border = !project.expanded;
            compose_project_row(
                project,
                selected_container_id,
                theme_mode,
                has_bottom_border,
                cx,
            )
        }
        ContainerListItem::ComposeChild(container) => {
            let has_bottom_border = !matches!(next, Some(ContainerListItem::ComposeChild(_)));
            container_row(
                container,
                selected_container_id,
                theme_mode,
                true,
                has_bottom_border,
                cx,
            )
        }
    }
}

fn container_row(
    container: ContainerSummary,
    selected_container_id: Option<String>,
    theme_mode: ThemeMode,
    is_child: bool,
    has_bottom_border: bool,
    cx: &mut Context<EchoApp>,
) -> AnyElement {
    let row = ContainerRowVm::from(&container);
    let selected = selected_container_id.as_deref() == Some(&container.id);
    let height = if is_child {
        COMPOSE_CHILD_ROW_HEIGHT
    } else {
        CONTAINER_ROW_HEIGHT
    };

    let background = if selected {
        hsla(0.539, 1.0, 0.263, 0.20)
    } else {
        transparent_black()
    };
    let icon_style = container_icon_style(&row, is_child, theme_mode);
    let icon_image_size = container_icon_image_size(icon_style.path);

    div()
        .w_full()
        .id(("container-row", index_from_short_id(&row.id)))
        .h(height)
        .relative()
        .pl(LIST_ROW_PADDING_LEFT)
        .pr(px(10.))
        .overflow_hidden()
        .border_b_1()
        .border_color(if has_bottom_border {
            theme_border(theme_mode)
        } else {
            transparent_black()
        })
        .bg(background)
        .cursor_pointer()
        .on_click(cx.listener(move |app, _, _, cx| {
            app.model.update(cx, |model, cx| {
                model.select_container(Some(container.id.clone()), cx)
            });
        }))
        .child(
            h_flex()
                .h_full()
                .items_center()
                .min_w_0()
                .gap(if is_child { px(10.) } else { px(12.) })
                .child(
                    div()
                        .size(px(40.))
                        .flex_shrink_0()
                        .rounded(px(8.))
                        .overflow_hidden()
                        .when_some(icon_style.background, |this, background| {
                            this.bg(background)
                        })
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            img(icon_style.path)
                                .size(icon_image_size)
                                .rounded(px(8.))
                                .grayscale(icon_style.grayscale)
                                .object_fit(ObjectFit::Contain),
                        ),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(if is_child { px(2.) } else { px(3.) })
                        .child(
                            div()
                                .h(px(18.))
                                .text_sm()
                                .font_medium()
                                .line_height(relative(1.2))
                                .truncate()
                                .child(row.name),
                        )
                        .child(
                            div()
                                .h(px(17.))
                                .text_xs()
                                .line_height(relative(1.2))
                                .text_color(theme_secondary(theme_mode))
                                .truncate()
                                .child(row.image),
                        ),
                )
                .child(status_label_area(
                    row.status_label,
                    row.error_badge_label,
                    theme_mode,
                )),
        )
        .into_any_element()
}

fn container_icon_style(
    row: &ContainerRowVm,
    is_child: bool,
    theme_mode: ThemeMode,
) -> DockerIconStyle {
    if is_child {
        return DockerIconStyle {
            path: CHILD_ICON,
            background: None,
            grayscale: false,
        };
    }

    let fallback_icon_path = if row.is_compose && row.is_running {
        COMPOSE_ICON
    } else if row.is_compose {
        COMPOSE_ICON_INACTIVE
    } else if row.is_running {
        CONTAINER_ICON
    } else {
        CONTAINER_ICON_INACTIVE
    };
    let icon_state = if row.is_running {
        DockerIconState::Normal
    } else {
        DockerIconState::Stopped
    };

    docker_icon_style_for_reference(&row.image, fallback_icon_path, theme_mode, icon_state)
}

fn is_docker_icon_asset(path: &str) -> bool {
    path.starts_with("assets/images/docker-icons/")
}

fn container_icon_image_size(path: &str) -> Pixels {
    if is_docker_icon_asset(path) {
        px(32.)
    } else {
        px(40.)
    }
}

fn compose_project_row(
    project: ComposeProjectRow,
    selected_container_id: Option<String>,
    theme_mode: ThemeMode,
    has_bottom_border: bool,
    cx: &mut Context<EchoApp>,
) -> AnyElement {
    let selected = selected_container_id
        .as_ref()
        .is_some_and(|selected| project.child_ids.iter().any(|id| id == selected));
    let background = if selected {
        hsla(0.539, 1.0, 0.263, 0.20)
    } else {
        transparent_black()
    };
    let icon_path = if project.running_count > 0 {
        COMPOSE_ICON
    } else {
        COMPOSE_ICON_INACTIVE
    };
    let project_id = project.project.clone();

    div()
        .w_full()
        .id(("compose-project-row", index_from_project(&project.project)))
        .h(CONTAINER_ROW_HEIGHT)
        .relative()
        .pl(LIST_ROW_PADDING_LEFT)
        .pr(px(10.))
        .overflow_hidden()
        .border_b_1()
        .border_color(if has_bottom_border {
            theme_border(theme_mode)
        } else {
            transparent_black()
        })
        .bg(background)
        .cursor_pointer()
        .on_click(cx.listener(move |app, _, _, cx| {
            app.toggle_compose_project(project_id.clone(), cx);
        }))
        .child(
            h_flex()
                .h_full()
                .items_center()
                .min_w_0()
                .gap(px(10.))
                .child(
                    div()
                        .size(px(40.))
                        .flex_shrink_0()
                        .rounded(px(8.))
                        .overflow_hidden()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            img(icon_path)
                                .size(px(40.))
                                .rounded(px(8.))
                                .object_fit(ObjectFit::Contain),
                        ),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(3.))
                        .child(
                            div()
                                .h(px(18.))
                                .text_sm()
                                .font_medium()
                                .line_height(relative(1.2))
                                .truncate()
                                .child(project.project),
                        )
                        .child(
                            h_flex()
                                .min_w_0()
                                .gap(px(2.))
                                .items_center()
                                .text_xs()
                                .line_height(relative(1.2))
                                .text_color(theme_secondary(theme_mode))
                                .child(project.services_label)
                                .child(
                                    Icon::new(if project.expanded {
                                        IconName::ChevronDown
                                    } else {
                                        IconName::ChevronRight
                                    })
                                    .xsmall()
                                    .text_color(theme_secondary(theme_mode)),
                                ),
                        ),
                )
                .child(status_label_area(
                    project.status_label,
                    project.error_badge_label,
                    theme_mode,
                )),
        )
        .into_any_element()
}

fn status_label_area(
    status_label: String,
    error_badge_label: Option<String>,
    theme_mode: ThemeMode,
) -> AnyElement {
    let mut area = h_flex()
        .w(px(92.))
        .h_full()
        .flex_shrink_0()
        .items_center()
        .justify_end()
        .gap(px(5.))
        .text_xs()
        .line_height(relative(1.2))
        .text_color(theme_muted(theme_mode));

    if let Some(error_badge_label) = error_badge_label {
        area = area.child(error_status_badge(error_badge_label, theme_mode));
    }

    area.child(div().truncate().child(status_label))
        .into_any_element()
}

fn error_status_badge(label: String, theme_mode: ThemeMode) -> impl IntoElement {
    div()
        .min_w(px(14.))
        .h(px(14.))
        .px(px(4.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(theme_error(theme_mode))
        .text_color(white())
        .text_xs()
        .line_height(relative(1.0))
        .child(label)
}

fn index_from_short_id(id: &str) -> u64 {
    u64::from_str_radix(id, 16).unwrap_or(0)
}

fn index_from_project(project: &str) -> u64 {
    project.bytes().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

struct ContainerRowVm {
    id: String,
    name: String,
    image: String,
    status_label: String,
    error_badge_label: Option<String>,
    is_compose: bool,
    is_running: bool,
}

#[derive(Clone)]
enum ContainerListItem {
    Container(ContainerSummary),
    ComposeProject(ComposeProjectRow),
    ComposeChild(ContainerSummary),
}

impl ContainerListItem {
    fn height(&self) -> Pixels {
        match self {
            Self::ComposeChild(_) => COMPOSE_CHILD_ROW_HEIGHT,
            Self::Container(_) | Self::ComposeProject(_) => CONTAINER_ROW_HEIGHT,
        }
    }
}

#[derive(Clone)]
struct ComposeProjectRow {
    project: String,
    services_label: String,
    status_label: String,
    expanded: bool,
    running_count: usize,
    error_badge_label: Option<String>,
    child_ids: Vec<String>,
}

impl From<&ContainerSummary> for ContainerRowVm {
    fn from(container: &ContainerSummary) -> Self {
        let state = container.state.as_deref();

        Self {
            id: short_id(&container.id),
            name: container.name.clone(),
            image: container.image.clone(),
            status_label: container_status_label(&container.status, state),
            error_badge_label: has_container_error(&container.status, state)
                .then(|| "!".to_string()),
            is_compose: container.is_compose,
            is_running: container_is_running(container),
        }
    }
}

fn container_list_items(
    containers: &[ContainerSummary],
    _selected_container_id: &Option<String>,
    search_text: &str,
    expanded_compose_projects: &BTreeSet<String>,
) -> Vec<ContainerListItem> {
    let searching = !search_text.trim().is_empty();
    let mut compose_projects = BTreeMap::<String, Vec<ContainerSummary>>::new();
    let mut items = Vec::new();

    for container in containers {
        if let Some(compose) = &container.compose {
            compose_projects
                .entry(compose.project.clone())
                .or_default()
                .push(container.clone());
        } else {
            items.push(ContainerListItem::Container(container.clone()));
        }
    }

    for (project, children) in compose_projects {
        let expanded = searching || expanded_compose_projects.contains(&project);
        let running_count = children
            .iter()
            .filter(|container| container_is_running(container))
            .count();
        let error_count = children
            .iter()
            .filter(|container| has_container_error(&container.status, container.state.as_deref()))
            .count();
        let child_ids = children
            .iter()
            .map(|container| container.id.clone())
            .collect::<Vec<_>>();
        items.push(ContainerListItem::ComposeProject(ComposeProjectRow {
            project,
            services_label: services_label(children.len()),
            status_label: compose_running_status_label(running_count, children.len()),
            expanded,
            running_count,
            error_badge_label: (error_count > 0).then(|| error_count.to_string()),
            child_ids,
        }));

        if expanded {
            items.extend(children.into_iter().map(ContainerListItem::ComposeChild));
        }
    }

    items
}

fn services_label(count: usize) -> String {
    if count == 1 {
        t!("list.service", count = count).to_string()
    } else {
        t!("list.services", count = count).to_string()
    }
}

fn container_status_label(_status: &str, state: Option<&str>) -> String {
    running_status_label(state.is_some_and(|state| state.eq_ignore_ascii_case("running")))
}

fn running_status_label(is_running: bool) -> String {
    if is_running {
        t!("list.running").to_string()
    } else {
        t!("list.stopped").to_string()
    }
}

fn compose_running_status_label(running_count: usize, total_count: usize) -> String {
    if running_count == 0 {
        t!("list.stopped").to_string()
    } else if running_count == total_count {
        t!("list.all_running").to_string()
    } else {
        t!("list.running_count", count = running_count).to_string()
    }
}

fn container_is_running(container: &ContainerSummary) -> bool {
    container
        .state
        .as_deref()
        .is_some_and(|state| state.eq_ignore_ascii_case("running"))
}

fn has_container_error(status: &str, state: Option<&str>) -> bool {
    if matches!(state, Some(state) if state.eq_ignore_ascii_case("exited")) {
        return !(status.contains("Exited (0)") || status.contains("Exit 0"));
    }

    false
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use gpui::px;

    use crate::{
        domain::{ComposeMetadata, ContainerSummary},
        ui::docker_icons::DockerIconState,
    };

    use super::{
        CHILD_ICON, CONTAINER_ICON, ContainerListItem, ContainerRowVm, container_icon_image_size,
        container_icon_style, container_list_items, container_list_row_size,
    };

    #[test]
    fn maps_container_summary_to_row() {
        let container = ContainerSummary::new(
            "1234567890abcdef".to_string(),
            "api".to_string(),
            "echo:latest".to_string(),
            "Up 2 hours".to_string(),
            Some("running".to_string()),
            None,
            Vec::new(),
            false,
        );

        let row = ContainerRowVm::from(&container);

        assert_eq!(row.id, "1234567890ab");
        assert_eq!(row.name, "api");
        assert_eq!(row.image, "echo:latest");
        assert_eq!(row.status_label, "Running");
        assert_eq!(row.error_badge_label, None);
        assert!(!row.is_compose);
        assert!(row.is_running);
    }

    #[test]
    fn virtual_list_row_size_does_not_force_panel_width() {
        let container = ContainerSummary::new(
            "1234567890abcdef".to_string(),
            "api".to_string(),
            "echo:latest".to_string(),
            "Up 2 hours".to_string(),
            Some("running".to_string()),
            None,
            Vec::new(),
            false,
        );

        let size = container_list_row_size(&ContainerListItem::Container(container));

        assert_eq!(size.width, px(1.));
        assert_eq!(size.height, super::CONTAINER_ROW_HEIGHT);
    }

    #[test]
    fn uses_official_container_icon_when_available() {
        assert_eq!(
            super::docker_icon_style_for_reference(
                "nginx:latest",
                CONTAINER_ICON,
                gpui_component::ThemeMode::Light,
                DockerIconState::Normal
            )
            .path,
            "assets/images/docker-icons/nginx.png"
        );
        assert_eq!(
            super::docker_icon_style_for_reference(
                "private/nginx:latest",
                CONTAINER_ICON,
                gpui_component::ThemeMode::Light,
                DockerIconState::Normal
            )
            .path,
            CONTAINER_ICON
        );
    }

    #[test]
    fn compose_child_uses_child_icon_without_official_matching() {
        let container = ContainerSummary::new_with_compose(
            "abcdef1234567890".to_string(),
            "web".to_string(),
            "nginx:latest".to_string(),
            "Up 2 hours".to_string(),
            Some("running".to_string()),
            None,
            Vec::new(),
            Some(ComposeMetadata {
                project: "stack".to_string(),
                service: Some("web".to_string()),
            }),
            true,
        );
        let row = ContainerRowVm::from(&container);

        let icon_style = container_icon_style(&row, true, gpui_component::ThemeMode::Light);

        assert_eq!(icon_style.path, CHILD_ICON);
        assert_eq!(icon_style.background, None);
        assert!(!icon_style.grayscale);
    }

    #[test]
    fn fallback_icons_render_at_full_list_icon_size() {
        assert_eq!(
            container_icon_image_size("assets/images/docker-icons/nginx.png"),
            px(32.)
        );
        assert_eq!(container_icon_image_size(CHILD_ICON), px(40.));
        assert_eq!(container_icon_image_size(CONTAINER_ICON), px(40.));
    }

    #[test]
    fn maps_stopped_container_status_to_stopped() {
        let container = ContainerSummary::new(
            "abcdef1234567890".to_string(),
            "worker".to_string(),
            "echo:latest".to_string(),
            "Exited (1) 5 minutes ago".to_string(),
            Some("exited".to_string()),
            None,
            Vec::new(),
            false,
        );

        let row = ContainerRowVm::from(&container);

        assert_eq!(row.status_label, "Stopped");
        assert_eq!(row.error_badge_label.as_deref(), Some("!"));
        assert!(!row.is_running);
    }

    #[test]
    fn groups_compose_containers_when_expanded() {
        let containers = vec![
            ContainerSummary::new_with_compose(
                "one".to_string(),
                "web".to_string(),
                "nginx:latest".to_string(),
                "Up 1 hour".to_string(),
                Some("running".to_string()),
                None,
                Vec::new(),
                Some(ComposeMetadata::new(
                    "stack".to_string(),
                    Some("web".to_string()),
                )),
                false,
            ),
            ContainerSummary::new_with_compose(
                "two".to_string(),
                "db".to_string(),
                "postgres:latest".to_string(),
                "Exited (0)".to_string(),
                Some("exited".to_string()),
                None,
                Vec::new(),
                Some(ComposeMetadata::new(
                    "stack".to_string(),
                    Some("db".to_string()),
                )),
                false,
            ),
        ];
        let mut expanded = BTreeSet::new();
        expanded.insert("stack".to_string());

        let items = container_list_items(&containers, &None, "", &expanded);

        assert_eq!(items.len(), 3);
        match &items[0] {
            ContainerListItem::ComposeProject(project) => {
                assert_eq!(project.project, "stack");
                assert_eq!(project.services_label, "2 services");
                assert_eq!(project.status_label, "1 Running");
                assert!(project.expanded);
                assert_eq!(project.running_count, 1);
                assert_eq!(project.error_badge_label, None);
            }
            _ => panic!("expected compose project"),
        }
    }

    #[test]
    fn labels_fully_running_compose_project() {
        let containers = vec![
            compose_container("one", "web", "Up 1 hour", Some("running")),
            compose_container("two", "db", "Up 1 hour", Some("running")),
        ];

        let items = container_list_items(&containers, &None, "", &BTreeSet::new());

        match &items[0] {
            ContainerListItem::ComposeProject(project) => {
                assert_eq!(project.status_label, "All running");
                assert_eq!(project.running_count, 2);
            }
            _ => panic!("expected compose project"),
        }
    }

    #[test]
    fn labels_stopped_compose_project() {
        let containers = vec![
            compose_container("one", "web", "Exited (0)", Some("exited")),
            compose_container("two", "db", "Exited (0)", Some("exited")),
        ];

        let items = container_list_items(&containers, &None, "", &BTreeSet::new());

        match &items[0] {
            ContainerListItem::ComposeProject(project) => {
                assert_eq!(project.status_label, "Stopped");
                assert_eq!(project.running_count, 0);
            }
            _ => panic!("expected compose project"),
        }
    }

    #[test]
    fn counts_compose_project_errors() {
        let containers = vec![
            compose_container("one", "web", "Exited (1) 5 minutes ago", Some("exited")),
            compose_container("two", "db", "Exited (0)", Some("exited")),
            compose_container("three", "worker", "Exited (2) 1 minute ago", Some("exited")),
        ];

        let items = container_list_items(&containers, &None, "", &BTreeSet::new());

        match &items[0] {
            ContainerListItem::ComposeProject(project) => {
                assert_eq!(project.error_badge_label.as_deref(), Some("2"));
            }
            _ => panic!("expected compose project"),
        }
    }

    fn compose_container(
        id: &str,
        name: &str,
        status: &str,
        state: Option<&str>,
    ) -> ContainerSummary {
        ContainerSummary::new_with_compose(
            id.to_string(),
            name.to_string(),
            "echo:latest".to_string(),
            status.to_string(),
            state.map(str::to_string),
            None,
            Vec::new(),
            Some(ComposeMetadata::new(
                "stack".to_string(),
                Some(name.to_string()),
            )),
            false,
        )
    }
}

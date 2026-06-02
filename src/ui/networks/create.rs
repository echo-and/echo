use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{
    IndexPath, Sizable, StyledExt as _, ThemeMode, WindowExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    dialog::{DialogAction, DialogClose, DialogFooter},
    h_flex,
    input::{Input, InputState},
    select::{Select, SelectState},
    v_flex,
};
use rust_i18n::t;

use crate::{
    app::EchoApp,
    bridge::NetworkCreateConfig,
    ui::theme::{theme_border, theme_secondary, theme_text},
};

const NETWORK_DRIVERS: [&str; 5] = ["bridge", "host", "none", "overlay", "macvlan"];

pub(super) fn open_create_network_dialog(
    _app: &mut EchoApp,
    theme_mode: ThemeMode,
    window: &mut Window,
    cx: &mut Context<EchoApp>,
) {
    let form = cx.new(|cx| NetworkCreateForm::new(theme_mode, window, cx));
    let app = cx.entity().downgrade();

    window.open_dialog(cx, move |dialog, _, _| {
        let submit_form = form.clone();
        let submit_app = app.clone();

        dialog
            .title(t!("networks.create_title"))
            .w(px(460.))
            .child(form.clone())
            .footer(
                DialogFooter::new().child(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap(px(8.))
                        .child(
                            DialogClose::new().child(
                                Button::new("network-create-cancel")
                                    .outline()
                                    .label(t!("networks.create_cancel")),
                            ),
                        )
                        .child(
                            DialogAction::new().child(
                                Button::new("network-create-confirm")
                                    .primary()
                                    .label(t!("networks.create_confirm")),
                            ),
                        ),
                ),
            )
            .on_ok(move |_, _, cx| {
                let config = submit_form.update(cx, |form, cx| form.submit(cx));
                let Some(config) = config else {
                    return false;
                };

                let _ = submit_app.update(cx, |app, cx| {
                    app.create_network(config, cx);
                });
                true
            })
    });
}

struct NetworkCreateForm {
    name_input: Entity<InputState>,
    driver_select: Entity<SelectState<Vec<SharedString>>>,
    subnet_input: Entity<InputState>,
    gateway_input: Entity<InputState>,
    enable_ipv6: bool,
    internal: bool,
    error: Option<String>,
    theme_mode: ThemeMode,
}

impl NetworkCreateForm {
    fn new(theme_mode: ThemeMode, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("networks.name_placeholder")));
        let driver_select = cx.new(|cx| {
            SelectState::new(
                NETWORK_DRIVERS
                    .iter()
                    .map(|driver| SharedString::from(*driver))
                    .collect::<Vec<_>>(),
                Some(IndexPath::default()),
                window,
                cx,
            )
        });
        let subnet_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("networks.subnet_placeholder")));
        let gateway_input = cx
            .new(|cx| InputState::new(window, cx).placeholder(t!("networks.gateway_placeholder")));

        Self {
            name_input,
            driver_select,
            subnet_input,
            gateway_input,
            enable_ipv6: false,
            internal: false,
            error: None,
            theme_mode,
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) -> Option<NetworkCreateConfig> {
        let name = input_value(&self.name_input, cx);
        if name.trim().is_empty() {
            self.error = Some(t!("networks.name_required").to_string());
            cx.notify();
            return None;
        }

        self.error = None;
        Some(NetworkCreateConfig {
            name,
            driver: self
                .driver_select
                .read(cx)
                .selected_value()
                .map(ToString::to_string)
                .unwrap_or_else(|| "bridge".to_string()),
            subnet: input_optional_value(&self.subnet_input, cx),
            gateway: input_optional_value(&self.gateway_input, cx),
            enable_ipv6: self.enable_ipv6,
            internal: self.internal,
        })
    }
}

impl Render for NetworkCreateForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme_mode = self.theme_mode;

        v_flex()
            .gap(px(12.))
            .pt(px(10.))
            .child(form_field(
                t!("networks.name"),
                Input::new(&self.name_input)
                    .small()
                    .w_full()
                    .text_color(theme_text(theme_mode)),
                theme_mode,
            ))
            .child(form_field(
                t!("networks.driver"),
                Select::new(&self.driver_select).small().w_full(),
                theme_mode,
            ))
            .child(
                h_flex()
                    .gap(px(10.))
                    .child(
                        form_field(
                            t!("networks.subnet"),
                            Input::new(&self.subnet_input)
                                .small()
                                .w_full()
                                .text_color(theme_text(theme_mode)),
                            theme_mode,
                        )
                        .flex_1(),
                    )
                    .child(
                        form_field(
                            t!("networks.gateway"),
                            Input::new(&self.gateway_input)
                                .small()
                                .w_full()
                                .text_color(theme_text(theme_mode)),
                            theme_mode,
                        )
                        .flex_1(),
                    ),
            )
            .child(
                v_flex()
                    .gap(px(8.))
                    .pt(px(2.))
                    .child(
                        Checkbox::new("network-create-ipv6")
                            .checked(self.enable_ipv6)
                            .label(t!("networks.enable_ipv6").to_string())
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.enable_ipv6 = *checked;
                                cx.notify();
                            })),
                    )
                    .child(
                        Checkbox::new("network-create-internal")
                            .checked(self.internal)
                            .label(t!("networks.internal").to_string())
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.internal = *checked;
                                cx.notify();
                            })),
                    ),
            )
            .when_some(self.error.clone(), |this, error| {
                this.child(
                    div()
                        .w_full()
                        .p(px(8.))
                        .rounded(px(4.))
                        .border_1()
                        .border_color(theme_border(theme_mode))
                        .text_sm()
                        .line_height(relative(1.35))
                        .text_color(theme_text(theme_mode))
                        .child(error),
                )
            })
    }
}

fn form_field(
    label: impl Into<SharedString>,
    input: impl IntoElement,
    theme_mode: ThemeMode,
) -> Div {
    v_flex()
        .min_w_0()
        .gap(px(6.))
        .child(
            div()
                .text_xs()
                .font_medium()
                .text_color(theme_secondary(theme_mode))
                .child(label.into()),
        )
        .child(input)
}

fn input_value(input: &Entity<InputState>, cx: &mut Context<NetworkCreateForm>) -> String {
    input.read(cx).value().to_string()
}

fn input_optional_value(
    input: &Entity<InputState>,
    cx: &mut Context<NetworkCreateForm>,
) -> Option<String> {
    let value = input_value(input, cx);
    (!value.trim().is_empty()).then_some(value)
}

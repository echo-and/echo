use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AppLocale {
    English,
    Chinese,
}

impl AppLocale {
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Chinese => "zh-CN",
        }
    }
}

pub fn set_locale(locale: AppLocale) {
    let code = locale.code();
    rust_i18n::set_locale(code);
    gpui_component::set_locale(code);
}

use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

use gpui_component::ThemeMode;
use serde::{Deserialize, Serialize};

use crate::i18n::AppLocale;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppPreferences {
    pub locale: AppLocale,
    pub theme_mode: ThemeMode,
    pub font_family: AppFontFamily,
    pub auto_check_updates: bool,
    pub notify_new_version: bool,
    pub container_list_width: u16,
    pub docker_backend_id: Option<String>,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            locale: AppLocale::English,
            theme_mode: ThemeMode::Light,
            font_family: AppFontFamily::default(),
            auto_check_updates: true,
            notify_new_version: true,
            container_list_width: DEFAULT_CONTAINER_LIST_WIDTH,
            docker_backend_id: None,
        }
    }
}

const APP_FONT_FAMILIES: [AppFontFamily; 6] = [
    AppFontFamily::SystemDefault,
    AppFontFamily::HelveticaNeue,
    AppFontFamily::Arial,
    AppFontFamily::PingFangSc,
    AppFontFamily::MicrosoftYaHei,
    AppFontFamily::NotoSansCjkSc,
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum AppFontFamily {
    #[default]
    SystemDefault,
    HelveticaNeue,
    Arial,
    PingFangSc,
    MicrosoftYaHei,
    NotoSansCjkSc,
}

impl AppFontFamily {
    pub fn all() -> &'static [Self] {
        &APP_FONT_FAMILIES
    }

    pub fn setting_value(self) -> &'static str {
        match self {
            Self::SystemDefault => "system",
            Self::HelveticaNeue => "helvetica-neue",
            Self::Arial => "arial",
            Self::PingFangSc => "pingfang-sc",
            Self::MicrosoftYaHei => "microsoft-yahei",
            Self::NotoSansCjkSc => "noto-sans-cjk-sc",
        }
    }

    pub fn from_setting_value(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::SystemDefault),
            "helvetica-neue" => Some(Self::HelveticaNeue),
            "arial" => Some(Self::Arial),
            "pingfang-sc" => Some(Self::PingFangSc),
            "microsoft-yahei" => Some(Self::MicrosoftYaHei),
            "noto-sans-cjk-sc" => Some(Self::NotoSansCjkSc),
            _ => None,
        }
    }

    pub fn family_name(self) -> &'static str {
        match self {
            Self::SystemDefault => ".SystemUIFont",
            Self::HelveticaNeue => "Helvetica Neue",
            Self::Arial => "Arial",
            Self::PingFangSc => "PingFang SC",
            Self::MicrosoftYaHei => "Microsoft YaHei",
            Self::NotoSansCjkSc => "Noto Sans CJK SC",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::SystemDefault => "System Default",
            Self::HelveticaNeue => "Helvetica Neue",
            Self::Arial => "Arial",
            Self::PingFangSc => "PingFang SC",
            Self::MicrosoftYaHei => "Microsoft YaHei",
            Self::NotoSansCjkSc => "Noto Sans CJK SC",
        }
    }
}

pub const DEFAULT_CONTAINER_LIST_WIDTH: u16 = 260;
pub const MIN_CONTAINER_LIST_WIDTH: u16 = 220;
pub const MAX_CONTAINER_LIST_WIDTH: u16 = 480;

impl AppPreferences {
    pub fn load() -> Self {
        let path = preferences_path();
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => return Self::default(),
        };

        serde_json::from_str::<StoredPreferences>(&content)
            .map(Self::from)
            .unwrap_or_default()
    }

    pub fn save(&self) -> io::Result<()> {
        let path = preferences_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = fs::File::create(path)?;
        let payload = serde_json::to_string_pretty(&StoredPreferences::from(self))
            .unwrap_or_else(|_| "{}".to_string());
        file.write_all(payload.as_bytes())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct StoredPreferences {
    locale: AppLocale,
    theme_mode: StoredThemeMode,
    #[serde(default)]
    font_family: AppFontFamily,
    #[serde(default = "default_true")]
    auto_check_updates: bool,
    #[serde(default = "default_true")]
    notify_new_version: bool,
    #[serde(default = "default_container_list_width")]
    container_list_width: u16,
    #[serde(default)]
    docker_backend_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum StoredThemeMode {
    Light,
    Dark,
}

impl From<&AppPreferences> for StoredPreferences {
    fn from(value: &AppPreferences) -> Self {
        Self {
            locale: value.locale,
            theme_mode: value.theme_mode.into(),
            font_family: value.font_family,
            auto_check_updates: value.auto_check_updates,
            notify_new_version: value.notify_new_version,
            container_list_width: clamp_container_list_width(value.container_list_width),
            docker_backend_id: value.docker_backend_id.clone(),
        }
    }
}

impl From<StoredPreferences> for AppPreferences {
    fn from(value: StoredPreferences) -> Self {
        Self {
            locale: value.locale,
            theme_mode: value.theme_mode.into(),
            font_family: value.font_family,
            auto_check_updates: value.auto_check_updates,
            notify_new_version: value.notify_new_version,
            container_list_width: clamp_container_list_width(value.container_list_width),
            docker_backend_id: value.docker_backend_id,
        }
    }
}

fn default_container_list_width() -> u16 {
    DEFAULT_CONTAINER_LIST_WIDTH
}

fn default_true() -> bool {
    true
}

pub fn clamp_container_list_width(width: u16) -> u16 {
    width.clamp(MIN_CONTAINER_LIST_WIDTH, MAX_CONTAINER_LIST_WIDTH)
}

impl From<ThemeMode> for StoredThemeMode {
    fn from(value: ThemeMode) -> Self {
        match value {
            ThemeMode::Light => Self::Light,
            ThemeMode::Dark => Self::Dark,
        }
    }
}

impl From<StoredThemeMode> for ThemeMode {
    fn from(value: StoredThemeMode) -> Self {
        match value {
            StoredThemeMode::Light => ThemeMode::Light,
            StoredThemeMode::Dark => ThemeMode::Dark,
        }
    }
}

fn preferences_path() -> PathBuf {
    app_config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("preferences.json")
}

fn app_config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("echo"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_english_light() {
        let prefs = AppPreferences::default();
        assert_eq!(prefs.locale, AppLocale::English);
        assert_eq!(prefs.theme_mode, ThemeMode::Light);
        assert_eq!(prefs.font_family, AppFontFamily::SystemDefault);
        assert!(prefs.auto_check_updates);
        assert!(prefs.notify_new_version);
        assert_eq!(prefs.container_list_width, DEFAULT_CONTAINER_LIST_WIDTH);
    }

    #[test]
    fn serializes_round_trip() {
        let prefs = AppPreferences {
            locale: AppLocale::Chinese,
            theme_mode: ThemeMode::Dark,
            font_family: AppFontFamily::PingFangSc,
            auto_check_updates: false,
            notify_new_version: false,
            container_list_width: 320,
            docker_backend_id: Some("docker:host:unix:///tmp/docker.sock".to_string()),
        };
        let json = serde_json::to_string(&StoredPreferences::from(&prefs)).unwrap();
        let decoded: AppPreferences = serde_json::from_str::<StoredPreferences>(&json)
            .map(AppPreferences::from)
            .unwrap();
        assert_eq!(decoded, prefs);
    }

    #[test]
    fn defaults_missing_container_width() {
        let json = r#"{"locale":"English","theme_mode":"Light"}"#;
        let decoded: AppPreferences = serde_json::from_str::<StoredPreferences>(json)
            .map(AppPreferences::from)
            .unwrap();
        assert_eq!(decoded.container_list_width, DEFAULT_CONTAINER_LIST_WIDTH);
        assert_eq!(decoded.font_family, AppFontFamily::SystemDefault);
        assert!(decoded.auto_check_updates);
        assert!(decoded.notify_new_version);
        assert_eq!(decoded.docker_backend_id, None);
    }

    #[test]
    fn converts_font_setting_values() {
        for font_family in AppFontFamily::all() {
            assert_eq!(
                AppFontFamily::from_setting_value(font_family.setting_value()),
                Some(*font_family)
            );
        }
        assert_eq!(AppFontFamily::from_setting_value("unknown"), None);
    }
}

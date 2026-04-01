use std::{collections::HashMap, sync::OnceLock};

use serde::{Deserialize, Serialize};

/// App UI language preference stored in persisted app state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppLanguage {
    #[serde(rename = "en", alias = "English")]
    #[default]
    English,
    #[serde(rename = "zh-CN", alias = "ChineseSimplified")]
    ChineseSimplified,
}

impl AppLanguage {
    pub const ALL: [Self; 2] = [
        Self::English,
        Self::ChineseSimplified,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::ChineseSimplified => "zh-CN",
        }
    }

    pub fn from_dropdown_index(index: usize) -> Self {
        Self::ALL
            .get(index)
            .copied()
            .unwrap_or(Self::English)
    }

    pub fn dropdown_index(self) -> usize {
        Self::ALL
            .iter()
            .position(|lang| *lang == self)
            .unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum I18nKey {
    AllSettingsTitle,
    SettingsCategoryAccount,
    SettingsCategoryPreferences,
    SettingsCategoryLabs,
    LanguageTitle,
    ApplicationLanguageLabel,
    LanguageReloadHint,
    LanguageOptionEnglish,
    LanguageOptionChineseSimplified,
}

impl I18nKey {
    fn as_str(self) -> &'static str {
        match self {
            I18nKey::AllSettingsTitle => "settings.all_settings_title",
            I18nKey::SettingsCategoryAccount => "settings.category.account",
            I18nKey::SettingsCategoryPreferences => "settings.category.preferences",
            I18nKey::SettingsCategoryLabs => "settings.category.labs",
            I18nKey::LanguageTitle => "settings.preferences.language.title",
            I18nKey::ApplicationLanguageLabel => "settings.preferences.language.application_label",
            I18nKey::LanguageReloadHint => "settings.preferences.language.reload_hint",
            I18nKey::LanguageOptionEnglish => "language.option.english",
            I18nKey::LanguageOptionChineseSimplified => "language.option.chinese_simplified",
        }
    }
}

fn load_dictionary(language: AppLanguage) -> HashMap<String, String> {
    let json = match language {
        AppLanguage::English => include_str!("../resources/i18n/en.json"),
        AppLanguage::ChineseSimplified => include_str!("../resources/i18n/zh-CN.json"),
    };
    serde_json::from_str(json).unwrap_or_default()
}

fn dictionary(language: AppLanguage) -> &'static HashMap<String, String> {
    static EN_DICTIONARY: OnceLock<HashMap<String, String>> = OnceLock::new();
    static ZH_CN_DICTIONARY: OnceLock<HashMap<String, String>> = OnceLock::new();

    match language {
        AppLanguage::English => EN_DICTIONARY.get_or_init(|| load_dictionary(AppLanguage::English)),
        AppLanguage::ChineseSimplified => ZH_CN_DICTIONARY.get_or_init(|| load_dictionary(AppLanguage::ChineseSimplified)),
    }
}

pub fn tr_key<'a>(language: AppLanguage, key: &'a str) -> &'a str {
    dictionary(language)
        .get(key)
        .map(String::as_str)
        .or_else(|| dictionary(AppLanguage::English).get(key).map(String::as_str))
        .unwrap_or(key)
}

pub fn tr_fmt(language: AppLanguage, key: &str, vars: &[(&str, &str)]) -> String {
    let mut output = tr_key(language, key).to_string();
    for (name, value) in vars {
        output = output.replace(&format!("{{{name}}}"), value);
    }
    output
}

pub fn tr(language: AppLanguage, key: I18nKey) -> &'static str {
    tr_key(language, key.as_str())
}

pub fn language_dropdown_labels(language: AppLanguage) -> Vec<String> {
    vec![
        tr(language, I18nKey::LanguageOptionEnglish).to_string(),
        tr(language, I18nKey::LanguageOptionChineseSimplified).to_string(),
    ]
}

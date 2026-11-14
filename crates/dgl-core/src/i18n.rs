//! UI copy only. Never pass host names, terminal output, or protocol values here.
//! Text is looked up by `area.meaning` keys in `locales/*.json`; see `scripts/generate_locales.py`.
use std::collections::HashMap;
use std::sync::{
    OnceLock,
    atomic::{AtomicUsize, Ordering},
};

#[path = "i18n_catalog.rs"]
mod catalog;

/// A supported UI language, in the picker order of `catalog::LOCALES`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Language(usize);

impl Language {
    pub fn from_code(code: &str) -> Option<Self> {
        catalog::LOCALES
            .iter()
            .position(|(candidate, _)| *candidate == code)
            .map(Self)
    }
    fn known(code: &str) -> Self {
        Self::from_code(code).expect("generated UI catalog lists the language")
    }
    pub fn korean() -> Self {
        Self::known("ko")
    }
    pub fn english() -> Self {
        Self::known("en")
    }
    pub fn base() -> Self {
        Self::known(catalog::BASE)
    }
    pub fn all() -> impl Iterator<Item = Self> {
        (0..catalog::LOCALES.len()).map(Self)
    }
    pub fn code(self) -> &'static str {
        catalog::LOCALES[self.0].0
    }
    /// The language's name written in that language, for language pickers.
    pub fn native_name(self) -> &'static str {
        text_in(self, "language.name")
    }
    /// Explicit supported preferences win; anything else ("system", old or unknown
    /// values) follows the system locale and falls back to the base language.
    pub fn resolve(preference: &str, system_locale: &str) -> Self {
        Self::from_code(preference).unwrap_or_else(|| {
            let tag = system_locale
                .split(['-', '_', '.', '@'])
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            Self::from_code(&tag).unwrap_or_else(Self::base)
        })
    }
}

// Applications configure this before opening UI. Until then Korean is used, which
// preserves the language of historical fixtures used by engine-only tests.
const UNSET: usize = usize::MAX;
static LANGUAGE: AtomicUsize = AtomicUsize::new(UNSET);
pub fn configure(preference: &str, system_locale: &str) {
    LANGUAGE.store(
        Language::resolve(preference, system_locale).0,
        Ordering::Relaxed,
    );
}
pub fn current() -> Language {
    match LANGUAGE.load(Ordering::Relaxed) {
        UNSET => Language::korean(),
        index => Language(index),
    }
}
fn catalogs() -> &'static [HashMap<String, String>] {
    static CATALOGS: OnceLock<Vec<HashMap<String, String>>> = OnceLock::new();
    CATALOGS.get_or_init(|| {
        catalog::LOCALES
            .iter()
            .map(|(_, json)| serde_json::from_str(json).expect("validated UI catalog"))
            .collect()
    })
}
/// Looks up `key` in `language`, then in the base language. Unknown keys are shown
/// as-is so text that was already localized passes through unchanged.
pub fn text_in(language: Language, key: &str) -> &str {
    let catalogs = catalogs();
    catalogs[language.0]
        .get(key)
        .or_else(|| catalogs[Language::base().0].get(key))
        .map_or(key, String::as_str)
}
pub fn t(key: &str) -> &str {
    text_in(current(), key)
}

/// Numbered placeholders can be reordered in translations. Arguments are appended
/// once, so braces and placeholder-looking text in user data remain literal.
pub fn interpolate(template: &str, arguments: &[String]) -> String {
    let mut output = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        output.push_str(&rest[..start]);
        rest = &rest[start..];
        if let Some(end) = rest.find('}')
            && let Ok(index) = rest[1..end].parse::<usize>()
            && let Some(value) = arguments.get(index)
        {
            output.push_str(value);
            rest = &rest[end + 1..];
        } else {
            output.push('{');
            rest = &rest[1..];
        }
    }
    output.push_str(rest);
    output
}

#[macro_export]
macro_rules! tr {
    ($source:literal $(, $argument:expr)* $(,)?) => {
        $crate::i18n::interpolate($crate::i18n::t($source), &[$($argument.to_string()),*])
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn system_selection_overrides_and_fallback() {
        let (korean, english) = (Language::korean(), Language::english());
        for tag in ["ko", "ko-KR", "KO_kr.UTF-8", "ko_KR@variant"] {
            assert_eq!(Language::resolve("system", tag), korean);
            assert_eq!(Language::resolve("en", tag), english);
        }
        for tag in ["en-US", "zz-ZZ", "", "C"] {
            assert_eq!(Language::resolve("system", tag), english);
            assert_eq!(Language::resolve("ko", tag), korean);
        }
        assert_eq!(Language::resolve("future-language", "ko-KR"), korean);
        assert_eq!(Language::base(), english);
    }
    #[test]
    fn every_language_is_listed_with_its_own_name() {
        let codes: Vec<_> = Language::all().map(Language::code).collect();
        assert!(codes.contains(&"ko") && codes.contains(&"en"));
        assert_eq!(Language::korean().native_name(), "한국어");
        assert_eq!(Language::english().native_name(), "English");
        for language in Language::all() {
            assert_eq!(Language::from_code(language.code()), Some(language));
        }
    }
    #[test]
    fn embedded_catalogs_share_keys_and_placeholders() {
        let catalogs = catalogs();
        let base = &catalogs[Language::base().0];
        let placeholders = |text: &str| {
            let mut found: Vec<_> = text
                .match_indices('{')
                .filter_map(|(start, _)| {
                    let rest = &text[start + 1..];
                    let end = rest.find('}')?;
                    rest[..end].parse::<usize>().ok()
                })
                .collect();
            found.sort_unstable();
            found.dedup();
            found
        };
        for catalog in catalogs {
            assert_eq!(catalog.len(), base.len());
            for (key, text) in base {
                assert_eq!(placeholders(&catalog[key]), placeholders(text), "{key}");
            }
        }
    }
    #[test]
    fn unknown_keys_and_finished_text_pass_through() {
        assert_eq!(text_in(Language::korean(), "missing.key"), "missing.key");
        assert_eq!(
            text_in(Language::english(), "이미 번역된 문구"),
            "이미 번역된 문구"
        );
    }
    #[test]
    fn translated_templates_preserve_user_data_and_reorder_arguments() {
        let key = "common.delete_named";
        let name = "한글 {1} $HOME 😀".to_string();
        assert_eq!(
            interpolate(
                text_in(Language::english(), key),
                std::slice::from_ref(&name)
            ),
            format!("Delete {name}")
        );
        assert_eq!(
            interpolate(
                text_in(Language::korean(), key),
                std::slice::from_ref(&name)
            ),
            format!("{name} 삭제")
        );
        assert_eq!(
            interpolate("{1} / {0} / {1}", &["a".into(), name.clone()]),
            format!("{name} / a / {name}")
        );
    }
    #[test]
    fn language_preference_round_trips_and_old_settings_use_system() {
        let old = crate::model::LocalState::decode(br#"{}"#).unwrap();
        assert_eq!(old.preferences.language, "system");
        let mut state = old;
        state.preferences.language = "en".into();
        let encoded = serde_json::to_vec(&state).unwrap();
        assert_eq!(crate::model::LocalState::decode(&encoded).unwrap(), state);
    }
}

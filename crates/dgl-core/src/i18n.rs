//! UI copy only. Never pass host names, terminal output, or protocol values here.
use std::collections::HashMap;
use std::sync::{
    OnceLock,
    atomic::{AtomicU8, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Korean,
    English,
}

impl Language {
    pub fn resolve(preference: &str, system_locale: &str) -> Self {
        let tag = match preference {
            "ko" => "ko",
            "en" => "en",
            _ => system_locale,
        };
        match tag
            .split(['-', '_', '.', '@'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "ko" => Self::Korean,
            _ => Self::English,
        }
    }
}

// Applications configure this before opening UI. The default also preserves the
// language of historical fixtures used by engine-only tests.
static LANGUAGE: AtomicU8 = AtomicU8::new(0);
pub fn configure(preference: &str, system_locale: &str) {
    LANGUAGE.store(
        if Language::resolve(preference, system_locale) == Language::Korean {
            0
        } else {
            1
        },
        Ordering::Relaxed,
    );
}
pub fn current() -> Language {
    if LANGUAGE.load(Ordering::Relaxed) == 0 {
        Language::Korean
    } else {
        Language::English
    }
}
fn english() -> &'static HashMap<String, String> {
    static CATALOG: OnceLock<HashMap<String, String>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../../../locales/en.json"))
            .expect("validated UI catalog")
    })
}
pub fn text_in(language: Language, source: &str) -> &str {
    if language == Language::English {
        english().get(source).map(String::as_str).unwrap_or(source)
    } else {
        source
    }
}
pub fn t(source: &str) -> &str {
    text_in(current(), source)
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
        for tag in ["ko", "ko-KR", "KO_kr.UTF-8", "ko_KR@variant"] {
            assert_eq!(Language::resolve("system", tag), Language::Korean);
            assert_eq!(Language::resolve("en", tag), Language::English);
        }
        for tag in ["en-US", "ja-JP", "", "C"] {
            assert_eq!(Language::resolve("system", tag), Language::English);
            assert_eq!(Language::resolve("ko", tag), Language::Korean);
        }
        assert_eq!(
            Language::resolve("future-language", "ko-KR"),
            Language::Korean
        );
    }
    #[test]
    fn translated_templates_preserve_user_data_and_reorder_arguments() {
        let source = "{0} 삭제";
        let name = "한글 {1} $HOME 😀".to_string();
        assert_eq!(
            interpolate(
                text_in(Language::English, source),
                std::slice::from_ref(&name)
            ),
            format!("Delete {name}")
        );
        assert_eq!(
            interpolate(
                text_in(Language::Korean, source),
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

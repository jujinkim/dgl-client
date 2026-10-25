//! Persist only the user's choice. Resolved platform colors are ephemeral.
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccentSettings {
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_color: Option<String>,
}
impl Default for AccentSettings {
    fn default() -> Self {
        Self {
            mode: "system".into(),
            custom_color: None,
        }
    }
}
impl<'de> Deserialize<'de> for AccentSettings {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        let decode = || {
            let object = value.as_object()?;
            let mode = object.get("mode").map_or(Some("system"), |v| v.as_str())?;
            if !["system", "default", "custom"].contains(&mode) {
                return None;
            }
            let custom_color = match object.get("customColor") {
                None | Some(serde_json::Value::Null) => None,
                Some(v) => Some(normalize_hex(v.as_str()?)?),
            };
            Some(Self {
                mode: mode.into(),
                custom_color,
            })
        };
        Ok(decode().unwrap_or_default())
    }
}
pub fn normalize_hex(value: &str) -> Option<String> {
    let value = value.trim();
    (value.len() == 7
        && value.starts_with('#')
        && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit))
    .then(|| value.to_ascii_uppercase())
}
pub fn parse_hex(value: &str) -> Option<u32> {
    normalize_hex(value).and_then(|v| u32::from_str_radix(&v[1..], 16).ok())
}
impl AccentSettings {
    pub fn valid(&self) -> bool {
        ["system", "default", "custom"].contains(&self.mode.as_str())
            && self
                .custom_color
                .as_ref()
                .is_none_or(|s| normalize_hex(s).as_ref() == Some(s))
    }
    pub fn resolve(&self, dark: bool, system: Option<u32>) -> u32 {
        let fallback = if dark { 0x75aaff } else { 0x2468c9 };
        match self.mode.as_str() {
            "system" => system.filter(|c| *c <= 0xffffff).unwrap_or(fallback),
            "custom" => self
                .custom_color
                .as_deref()
                .and_then(parse_hex)
                .unwrap_or(fallback),
            _ => fallback,
        }
    }
}
pub const PRESETS: [(&str, &str); 8] = [
    ("파랑", "#2468C9"),
    ("보라", "#8056D9"),
    ("분홍", "#D84F99"),
    ("빨강", "#D63D45"),
    ("주황", "#E57B25"),
    ("노랑", "#E6BB24"),
    ("초록", "#329654"),
    ("청록", "#21999D"),
];
pub fn blend(color: u32, background: u32, amount: f64) -> u32 {
    [16, 8, 0].into_iter().fold(0, |out, shift| {
        let a = ((color >> shift) & 255) as f64;
        let b = ((background >> shift) & 255) as f64;
        out | (((a * amount + b * (1. - amount)).round() as u32) << shift)
    })
}
fn luminance(color: u32) -> f64 {
    [16, 8, 0]
        .into_iter()
        .zip([0.2126, 0.7152, 0.0722])
        .map(|(shift, weight)| {
            let c = ((color >> shift) & 255) as f64 / 255.;
            weight
                * if c <= 0.04045 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
        })
        .sum()
}
pub fn contrast(a: u32, b: u32) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
pub fn on_color(color: u32) -> u32 {
    if contrast(0, color) >= contrast(0xffffff, color) {
        0
    } else {
        0xffffff
    }
}
pub fn readable(color: u32, backgrounds: &[u32], ratio: f64) -> u32 {
    let target = on_color(backgrounds[0]);
    for step in 0..=100 {
        let result = blend(target, color, step as f64 / 100.);
        if backgrounds.iter().all(|bg| contrast(result, *bg) >= ratio) {
            return result;
        }
    }
    target
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_are_tolerant_normalized_and_round_trip() {
        for invalid in [
            "null",
            "42",
            "[]",
            r##"{"mode":"unknown","customColor":"#123456"}"##,
            r##"{"mode":"custom","customColor":"#ZZ1234"}"##,
            r#"{"mode":42}"#,
        ] {
            let state = crate::model::LocalState::decode(
                format!(r#"{{"preferences":{{"fontSize":20,"accent":{invalid}}}}}"#).as_bytes(),
            )
            .unwrap();
            assert_eq!(state.preferences.accent, AccentSettings::default());
            assert_eq!(state.preferences.font_size, 20);
        }
        assert_eq!(
            crate::model::LocalState::decode(b"{}")
                .unwrap()
                .preferences
                .accent,
            AccentSettings::default()
        );
        let s: AccentSettings =
            serde_json::from_str(r##"{"mode":"custom","customColor":" #aBc123 "}"##).unwrap();
        assert_eq!(s.custom_color.as_deref(), Some("#ABC123"));
        assert_eq!(
            serde_json::from_str::<AccentSettings>(&serde_json::to_string(&s).unwrap()).unwrap(),
            s
        );
        for invalid in ["123456", "#123", "#12345678", "#abcdeg", "#é0000"] {
            assert!(normalize_hex(invalid).is_none());
        }
    }
    #[test]
    fn fallback_and_contrast_cover_both_themes_and_extreme_colors() {
        for dark in [false, true] {
            let s = AccentSettings::default();
            assert_eq!(
                s.resolve(dark, None),
                if dark { 0x75aaff } else { 0x2468c9 }
            );
            assert_eq!(s.resolve(dark, Some(0x123456)), 0x123456);
            let bg = if dark { 0x1e1f22 } else { 0xf7f7f8 };
            let panel = if dark { 0x25262a } else { 0xeeeef0 };
            for raw in PRESETS
                .iter()
                .map(|(_, s)| parse_hex(s).unwrap())
                .chain([0, 0xffffff, 0x777777])
            {
                assert!(contrast(on_color(raw), raw) >= 4.5);
                let selection = blend(raw, bg, 0.22);
                let text = readable(raw, &[bg, panel, selection], 4.5);
                assert!(
                    [bg, panel, selection]
                        .iter()
                        .all(|c| contrast(text, *c) >= 4.5)
                );
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: u32,
    pub panel: u32,
    pub session_header: u32,
    pub fg: u32,
    pub muted: u32,
    pub line: u32,
    pub accent: u32,
    pub accent_fill: u32,
    pub on_accent: u32,
    pub selection: u32,
    pub input: u32,
}
impl Palette {
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                bg: 0x1e1f22,
                panel: 0x25262a,
                session_header: 0x303239,
                fg: 0xdadce0,
                muted: 0x9ca1aa,
                line: 0x3b3d42,
                accent: 0x75aaff,
                accent_fill: 0x75aaff,
                on_accent: 0x000000,
                selection: 0x2b4264,
                input: 0x202125,
            }
        } else {
            Self {
                bg: 0xf7f7f8,
                panel: 0xeeeef0,
                session_header: 0xe1e3e8,
                fg: 0x24262b,
                muted: 0x626670,
                line: 0xd7d9df,
                accent: 0x2468c9,
                accent_fill: 0x2468c9,
                on_accent: 0xffffff,
                selection: 0xdbe9ff,
                input: 0xffffff,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dgl_core::accent::*;
    #[test]
    fn accent_changes_roles_but_preserves_neutral_and_terminal_backgrounds() {
        for dark in [false, true] {
            let base = Palette::new(dark);
            for color in PRESETS
                .iter()
                .map(|(_, hex)| parse_hex(hex).unwrap())
                .chain([0, 0xffffff])
            {
                let system = Palette::with_accent(dark, &AccentSettings::default(), Some(color));
                assert_eq!(system.bg, base.bg);
                assert_eq!(system.panel, base.panel);
                assert_eq!(system.input, base.input);
                assert_eq!(system.fg, base.fg);
                assert_eq!(system.accent_fill, color);
                assert!(contrast(system.on_accent, system.accent_fill) >= 4.5);
                for bg in [
                    system.bg,
                    system.panel,
                    system.session_header,
                    system.input,
                    system.selection,
                ] {
                    assert!(contrast(system.accent, bg) >= 4.5);
                    assert!(contrast(system.fg, system.selection) >= 4.5);
                }
                let custom = AccentSettings {
                    mode: "custom".into(),
                    custom_color: Some(format!("#{color:06X}")),
                };
                assert_eq!(
                    Palette::with_accent(dark, &custom, Some(0x123456)).accent_fill,
                    color
                );
            }
            assert_eq!(
                Palette::with_accent(dark, &AccentSettings::default(), None).selection,
                base.selection
            );
        }
    }
}

impl Palette {
    pub fn with_accent(
        dark: bool,
        settings: &dgl_core::accent::AccentSettings,
        system: Option<u32>,
    ) -> Self {
        use dgl_core::accent::*;
        let mut p = Self::new(dark);
        let raw = settings.resolve(dark, system);
        if raw != p.accent_fill {
            p.selection = blend(raw, p.bg, 0.22);
        }
        p.accent_fill = raw;
        p.on_accent = on_color(raw);
        p.accent = readable(
            raw,
            &[p.bg, p.panel, p.session_header, p.input, p.selection],
            4.5,
        );
        p
    }
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: u32,
    pub panel: u32,
    pub session_header: u32,
    pub fg: u32,
    pub muted: u32,
    pub line: u32,
    pub accent: u32,
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
                selection: 0xdbe9ff,
                input: 0xffffff,
            }
        }
    }
}

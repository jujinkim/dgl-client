use crate::{Error, Result, model::Size};
use serde::{Deserialize, Serialize};
pub fn quote(s: &str) -> Result<String> {
    if s.encode_utf16().count() > 1024 || s.chars().any(|c| c < ' ' || c == '\x7f') {
        Err(Error::InvalidArgument)
    } else {
        Ok(format!("'{}'", s.replace('\'', "'\\''")))
    }
}
pub fn target(s: &str, prefix: char) -> Result<String> {
    if !s.starts_with(prefix) || s.len() < 2 || !s.as_bytes()[1..].iter().all(u8::is_ascii_digit) {
        Err(Error::InvalidArgument)
    } else {
        quote(s)
    }
}
fn name(s: &str) -> Result<String> {
    if s.trim().is_empty() || s.encode_utf16().count() > 128 || s.contains([':', '.']) {
        return Err(Error::InvalidArgument);
    }
    quote(&s.replace('#', "##"))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Operation {
    NewSession {
        name: String,
    },
    RenameSession {
        target: String,
        name: String,
    },
    KillSession {
        target: String,
    },
    SwitchSession {
        target: String,
    },
    NewWindow {
        target: String,
        name: String,
    },
    RenameWindow {
        target: String,
        name: String,
    },
    SelectWindow {
        target: String,
    },
    KillWindow {
        target: String,
    },
    SelectPane {
        target: String,
    },
    KillPane {
        target: String,
    },
    RenamePane {
        target: String,
        name: String,
    },
    Split {
        target: String,
        horizontal: bool,
    },
    ResizePane {
        target: String,
        direction: char,
    },
    ResizePaneTo {
        target: String,
        columns: u16,
        rows: u16,
    },
    FocusPane {
        target: String,
        direction: char,
    },
    ClientSize {
        columns: u16,
        rows: u16,
    },
    /// Makes this client the latest one of its current window, so that under tmux's
    /// default `window-size latest` the window takes this client's size. A control
    /// client's `refresh-client -C` and `send-keys` never do this; `select-window` does.
    /// Without a target it resolves to the client's own current window and changes no
    /// selection that other clients share.
    ClaimSize,
    /// Lets this client's size count again after an attach with `ignore-size`.
    CountSize,
    Send {
        target: String,
        bytes: Vec<u8>,
    },
    Capture {
        target: String,
    },
    CaptureMain {
        target: String,
    },
    Pending {
        target: String,
    },
    Pause {
        target: String,
    },
    Resume {
        target: String,
    },
    Modes {
        target: String,
    },
    PaneTitle {
        target: String,
    },
    Sessions,
    Windows,
    Panes,
    FlowControl,
    MetadataSubscription,
    /// The subscription format read once, for servers that cannot keep a subscription safely.
    MetadataPoll,
}
const SESSIONS_FORMAT: &str = "#{session_id}|#{q:session_name}|#{session_attached}";
const WINDOWS_FORMAT: &str = "#{session_id}|#{window_id}|#{q:window_name}|#{window_active}|#{window_layout}|#{window_index}|#{window_width}|#{window_height}|#{window_zoomed_flag}|#{window_visible_layout}";
const PANES_FORMAT: &str = "#{pane_id}|#{window_id}|#{pane_width}|#{pane_height}|#{pane_active}|#{q:pane_mode}|#{q:pane_current_command}|#{q;s/[[#:cntrl#:]]/ /:pane_title}|#{pane_left}|#{pane_top}";
/// Every session's windows and panes; any change requires a metadata refresh.
const METADATA_FORMAT: &str = "#{S:#{session_id}|#{q:session_name}|#{W:#{window_id}|#{window_index}|#{q:window_name}|#{window_active}|#{P:#{pane_id}|#{pane_active}|#{q:pane_current_command}|#{q;s/[[#:cntrl#:]]/ /:pane_title};}}}";
#[derive(Clone, Debug)]
pub struct Command {
    pub wire: String,
    pub shell_wire: String,
}
impl Command {
    pub fn new(op: &Operation) -> Result<Self> {
        use Operation::*;
        let wire = match op {
            NewSession { name: n } => format!("new-session -d -s {}", name(n)?),
            RenameSession { target: t, name: n } => {
                format!("rename-session -t {} {}", target(t, '$')?, name(n)?)
            }
            KillSession { target: t } => format!("kill-session -t {}", target(t, '$')?),
            SwitchSession { target: t } => format!("switch-client -t {}", target(t, '$')?),
            NewWindow { target: t, name: n } => {
                format!("new-window -t {} -n {}", target(t, '$')?, name(n)?)
            }
            RenameWindow { target: t, name: n } => {
                format!("rename-window -t {} {}", target(t, '@')?, name(n)?)
            }
            SelectWindow { target: t } => format!("select-window -t {}", target(t, '@')?),
            KillWindow { target: t } => format!("kill-window -t {}", target(t, '@')?),
            SelectPane { target: t } => format!("select-pane -t {}", target(t, '%')?),
            RenamePane { target: t, name: n } => {
                format!("select-pane -t {} -T {}", target(t, '%')?, name(n)?)
            }
            KillPane { target: t } => format!("kill-pane -t {}", target(t, '%')?),
            Split {
                target: t,
                horizontal,
            } => format!(
                "split-window -{} -t {}",
                if *horizontal { 'h' } else { 'v' },
                target(t, '%')?
            ),
            FocusPane {
                target: t,
                direction,
            } => {
                if !"LRUD".contains(*direction) {
                    return Err(Error::InvalidArgument);
                }
                format!("select-pane -t {} -{}", target(t, '%')?, direction)
            }
            ResizePane {
                target: t,
                direction,
            } => {
                if !"LRUD".contains(*direction) {
                    return Err(Error::InvalidArgument);
                }
                format!("resize-pane -t {} -{} 3", target(t, '%')?, direction)
            }
            ResizePaneTo {
                target: t,
                columns,
                rows,
            } => {
                if *columns == 0 || *rows == 0 {
                    return Err(Error::InvalidArgument);
                }
                format!("resize-pane -t {} -x {columns} -y {rows}", target(t, '%')?)
            }
            ClientSize { columns, rows } => {
                Size::new(*columns, *rows)?;
                format!("refresh-client -C {columns},{rows}")
            }
            ClaimSize => "select-window".into(),
            CountSize => "refresh-client -f '!ignore-size'".into(),
            Send { target: t, bytes } => {
                if bytes.is_empty() || bytes.len() > 256 {
                    return Err(Error::InvalidArgument);
                }
                format!(
                    "send-keys -H -t {} {}",
                    target(t, '%')?,
                    bytes
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            }
            Capture { target: t } => format!("capture-pane -p -e -C -N -t {}", target(t, '%')?),
            CaptureMain { target: t } => {
                format!("capture-pane -p -a -q -e -C -N -t {}", target(t, '%')?)
            }
            Pending { target: t } => format!("capture-pane -p -P -C -t {}", target(t, '%')?),
            Pause { target: t } | Resume { target: t } => {
                target(t, '%')?;
                format!(
                    "refresh-client -A {}",
                    quote(&format!(
                        "{}:{}",
                        t,
                        if matches!(op, Pause { .. }) {
                            "pause"
                        } else {
                            "continue"
                        }
                    ))?
                )
            }
            Modes { target: t } => format!(
                "display-message -p -t {} '#{{cursor_x}}|#{{cursor_y}}|#{{alternate_on}}|#{{cursor_flag}}|#{{keypad_cursor_flag}}|#{{keypad_flag}}|#{{mouse_any_flag}}|#{{mouse_button_flag}}|#{{mouse_standard_flag}}|#{{mouse_utf8_flag}}|#{{mouse_sgr_flag}}|#{{bracketed_paste}}|#{{scroll_region_upper}}|#{{scroll_region_lower}}|#{{insert_flag}}|#{{pane_width}}|#{{pane_height}}'",
                target(t, '%')?
            ),
            PaneTitle { target: t } => format!(
                "display-message -p -t {} '#{{q;s/[[#:cntrl#:]]/ /:pane_title}}'",
                target(t, '%')?
            ),
            Sessions => format!("list-sessions -F '{SESSIONS_FORMAT}'"),
            Windows => format!("list-windows -a -F '{WINDOWS_FORMAT}'"),
            Panes => format!("list-panes -a -F '{PANES_FORMAT}'"),
            MetadataSubscription => format!("refresh-client -B 'dgl-metadata::{METADATA_FORMAT}'"),
            MetadataPoll => format!("display-message -p '{METADATA_FORMAT}'"),
            FlowControl => "refresh-client -f 'pause-after=2'".into(),
        };
        let shell_wire = if let NewSession { name: n } = op {
            let n = n.replace('#', "##");
            let n = if let Some(s) = n.strip_suffix(';') {
                format!("{s}\\;")
            } else {
                n
            };
            format!("tmux new-session -d -s {}", quote(&n)?)
        } else {
            format!("tmux {wire}")
        };
        Ok(Self { wire, shell_wire })
    }
    /// The three listings in one `tmux` process outside control mode. Their output is
    /// concatenated, so each row is tagged `s|`, `w|` or `p|` (see `model::inventory`).
    pub fn inventory() -> Self {
        let parts = [
            format!("list-sessions -F 's|{SESSIONS_FORMAT}'"),
            format!("list-windows -a -F 'w|{WINDOWS_FORMAT}'"),
            format!("list-panes -a -F 'p|{PANES_FORMAT}'"),
        ];
        Self {
            wire: parts.join(" ; "),
            shell_wire: format!("tmux {}", parts.join(" \\; ")),
        }
    }
    /// Writes paste buffer `name` raw to stdout. Run outside control mode only
    /// (`Transport::execute_tmux`): a control client cannot write to `-`, and an in-band
    /// reply would carry the buffer's bytes unescaped into the control stream.
    pub fn save_buffer(name: &str) -> Result<Self> {
        // tmux's own argv parser would take a trailing semicolon as a command separator.
        if name.is_empty() || name.ends_with(';') {
            return Err(Error::InvalidArgument);
        }
        let wire = format!("save-buffer -b {} -", quote(name)?);
        Ok(Self {
            shell_wire: format!("tmux {wire}"),
            wire,
        })
    }
    /// Parse our generated control grammar into argv, without involving a shell.
    pub fn argv(&self) -> Result<Vec<String>> {
        let mut args = argv(&self.wire)?;
        if self.wire != Self::inventory().wire {
            for arg in &mut args {
                // tmux's argv parser treats a trailing semicolon as a command separator.
                if arg.ends_with(';') {
                    arg.insert(arg.len() - 1, '\\');
                }
            }
        }
        Ok(args)
    }
    pub fn shell_with_executable(&self, path: &str) -> Result<String> {
        if !path.starts_with('/') {
            return Err(Error::InvalidArgument);
        }
        Ok(format!(
            "{} -u {}",
            quote(path)?,
            self.shell_wire
                .strip_prefix("tmux ")
                .ok_or(Error::InvalidArgument)?
        ))
    }
}
fn tmux_minor(version: &str) -> Option<u8> {
    let v = version.trim().strip_prefix("tmux ")?;
    let v = v
        .strip_suffix(|c: char| c.is_ascii_lowercase())
        .unwrap_or(v);
    let (major, minor) = v.split_once('.')?;
    (major == "3").then(|| minor.parse().ok()).flatten()
}
/// `%paste-buffer-changed` exists from tmux 3.4.
pub fn paste_buffer_notifications(version: &str) -> bool {
    tmux_minor(version).is_some_and(|n| n >= 4)
}
pub fn supports_tmux(version: &str) -> bool {
    tmux_minor(version).is_some_and(|n| (3..=7).contains(&n))
}
/// tmux 3.2-3.6 crash when a control client without a session keeps a format subscription
/// (`control_check_subs_timer`); 3.7 skips such clients.
pub fn safe_subscriptions(version: &str) -> bool {
    tmux_minor(version).is_some_and(|n| n >= 7)
}

/// Only the quoting grammar emitted above. A semicolon is a separate tmux argument.
pub(crate) fn argv(wire: &str) -> Result<Vec<String>> {
    let mut args = Vec::new();
    let mut word = String::new();
    let mut quote = false;
    let mut escaped = false;
    let mut started = false;
    for ch in wire.chars() {
        if escaped {
            word.push(ch);
            escaped = false;
        } else if ch == '\\' && !quote {
            escaped = true;
            started = true;
        } else if ch == '\'' {
            quote = !quote;
            started = true;
        } else if ch.is_whitespace() && !quote {
            if started {
                args.push(std::mem::take(&mut word));
                started = false;
            }
        } else {
            word.push(ch);
            started = true;
        }
    }
    if quote || escaped {
        return Err(Error::InvalidArgument);
    }
    if started {
        args.push(word);
    }
    Ok(args)
}

#[cfg(test)]
mod paste_buffer_tests {
    use super::{Command, paste_buffer_notifications};

    #[test]
    fn save_buffer_quotes_names_and_rejects_unsafe_ones() {
        let c = Command::save_buffer("a b'#{x};y").unwrap();
        assert_eq!(c.wire, r"save-buffer -b 'a b'\''#{x};y' -");
        assert_eq!(c.argv().unwrap(), ["save-buffer", "-b", "a b'#{x};y", "-"]);
        assert_eq!(
            c.shell_with_executable("/usr/bin/tmux").unwrap(),
            r"'/usr/bin/tmux' -u save-buffer -b 'a b'\''#{x};y' -"
        );
        for name in ["", "x;", "a\nb", "\x1b", &"n".repeat(1025)] {
            assert!(Command::save_buffer(name).is_err(), "{name:?}");
        }
    }

    #[test]
    fn paste_notifications_start_at_tmux_3_4() {
        for (version, expected) in [
            ("tmux 3.3a", false),
            ("tmux 3.4", true),
            ("tmux 3.7c", true),
            ("", false),
        ] {
            assert_eq!(paste_buffer_notifications(version), expected, "{version}");
        }
    }
}

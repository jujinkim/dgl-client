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
    NewSession { name: String },
    RenameSession { target: String, name: String },
    KillSession { target: String },
    SwitchSession { target: String },
    NewWindow { target: String, name: String },
    RenameWindow { target: String, name: String },
    SelectWindow { target: String },
    KillWindow { target: String },
    SelectPane { target: String },
    KillPane { target: String },
    Split { target: String, horizontal: bool },
    ResizePane { target: String, direction: char },
    FocusPane { target: String, direction: char },
    ClientSize { columns: u16, rows: u16 },
    Send { target: String, bytes: Vec<u8> },
    Capture { target: String },
    CaptureMain { target: String },
    Pending { target: String },
    Pause { target: String },
    Resume { target: String },
    Modes { target: String },
    Sessions,
    Windows,
    Panes,
    FlowControl,
}
#[derive(Clone, Debug)]
pub struct Command {
    pub wire: String,
    pub shell_wire: String,
}
impl Command {
    pub fn new(op: &Operation) -> Result<Self> {
        use Operation::*;
        let wire=match op{
            NewSession{name:n}=>format!("new-session -d -s {}",name(n)?),
            RenameSession{target:t,name:n}=>format!("rename-session -t {} {}",target(t,'$')?,name(n)?),
            KillSession{target:t}=>format!("kill-session -t {}",target(t,'$')?),
            SwitchSession{target:t}=>format!("switch-client -t {}",target(t,'$')?),
            NewWindow{target:t,name:n}=>format!("new-window -t {} -n {}",target(t,'$')?,name(n)?),
            RenameWindow{target:t,name:n}=>format!("rename-window -t {} {}",target(t,'@')?,name(n)?),
            SelectWindow{target:t}=>format!("select-window -t {}",target(t,'@')?),
            KillWindow{target:t}=>format!("kill-window -t {}",target(t,'@')?),
            SelectPane{target:t}=>format!("select-pane -t {}",target(t,'%')?),
            KillPane{target:t}=>format!("kill-pane -t {}",target(t,'%')?),
            Split{target:t,horizontal}=>format!("split-window -{} -t {}",if *horizontal{'h'}else{'v'},target(t,'%')?),
            FocusPane{target:t,direction}=>{if !"LRUD".contains(*direction){return Err(Error::InvalidArgument)}format!("select-pane -t {} -{}",target(t,'%')?,direction)},
            ResizePane{target:t,direction}=>{if !"LRUD".contains(*direction){return Err(Error::InvalidArgument)}format!("resize-pane -t {} -{} 3",target(t,'%')?,direction)},
            ClientSize{columns,rows}=>{Size::new(*columns,*rows)?;format!("refresh-client -C {columns},{rows}")},
            Send{target:t,bytes}=>{if bytes.is_empty()||bytes.len()>256{return Err(Error::InvalidArgument)}format!("send-keys -H -t {} {}",target(t,'%')?,bytes.iter().map(|b|format!("{b:02x}")).collect::<Vec<_>>().join(" "))},
            Capture{target:t}=>format!("capture-pane -p -e -C -N -t {}",target(t,'%')?),
            CaptureMain{target:t}=>format!("capture-pane -p -a -q -e -C -N -t {}",target(t,'%')?),
            Pending{target:t}=>format!("capture-pane -p -P -C -t {}",target(t,'%')?),
            Pause{target:t}|Resume{target:t}=>{target(t,'%')?;format!("refresh-client -A {}",quote(&format!("{}:{}",t,if matches!(op,Pause{..}){"pause"}else{"continue"}))?)},
            Modes{target:t}=>format!("display-message -p -t {} '#{{cursor_x}}|#{{cursor_y}}|#{{alternate_on}}|#{{cursor_flag}}|#{{keypad_cursor_flag}}|#{{keypad_flag}}|#{{mouse_any_flag}}|#{{mouse_button_flag}}|#{{mouse_standard_flag}}|#{{mouse_utf8_flag}}|#{{mouse_sgr_flag}}|#{{bracketed_paste}}|#{{scroll_region_upper}}|#{{scroll_region_lower}}|#{{insert_flag}}|#{{pane_width}}|#{{pane_height}}'",target(t,'%')?),
            Sessions=>"list-sessions -F '#{session_id}|#{q:session_name}|#{session_attached}'".into(),
            Windows=>"list-windows -a -F '#{session_id}|#{window_id}|#{q:window_name}|#{window_active}|#{window_layout}'".into(),
            Panes=>"list-panes -a -F '#{pane_id}|#{window_id}|#{pane_width}|#{pane_height}|#{pane_active}|#{q:pane_mode}|#{q:pane_current_command}'".into(),
            FlowControl=>"refresh-client -f 'pause-after=2'".into(),
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
    pub fn shell_with_executable(&self, path: &str) -> Result<String> {
        if !path.starts_with('/') {
            return Err(Error::InvalidArgument);
        }
        Ok(format!(
            "{} {}",
            quote(path)?,
            self.shell_wire
                .strip_prefix("tmux ")
                .ok_or(Error::InvalidArgument)?
        ))
    }
}
pub fn supports_tmux(version: &str) -> bool {
    let Some(v) = version.trim().strip_prefix("tmux ") else {
        return false;
    };
    let v = v
        .strip_suffix(|c: char| c.is_ascii_lowercase())
        .unwrap_or(v);
    let Some((major, minor)) = v.split_once('.') else {
        return false;
    };
    major == "3" && minor.parse::<u8>().is_ok_and(|n| (3..=7).contains(&n))
}

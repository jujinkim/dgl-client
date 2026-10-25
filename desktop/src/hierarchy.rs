//! Display-only projection. Stable tmux IDs remain the identity and selection target.
use dgl_core::{
    connection::Phase,
    model::{RemotePane, RemoteSession, RemoteState, RemoteWindow},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub title: String,
    pub detail: String,
}
impl Label {
    pub fn full_text(&self) -> String {
        format!("{}\n{}", self.title, self.detail)
    }
}
pub fn display_text(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() || matches!(c, '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}') {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}
fn named(text: &str, fallback: &str) -> String {
    let text = display_text(text);
    if text.is_empty() {
        fallback.into()
    } else {
        text
    }
}
pub fn session_label(state: &RemoteState, session: &RemoteSession) -> Label {
    let windows: Vec<_> = state
        .windows
        .iter()
        .filter(|w| w.session == session.id)
        .collect();
    let panes = state
        .panes
        .iter()
        .filter(|p| windows.iter().any(|w| w.id == p.window))
        .count();
    Label {
        title: named(&session.name, "이름 없는 세션"),
        detail: format!("창 {}개 · 패널 {panes}개", windows.len()),
    }
}
pub fn window_label(state: &RemoteState, window: &RemoteWindow) -> Label {
    let count = state.panes.iter().filter(|p| p.window == window.id).count();
    Label {
        title: named(&window.name, "이름 없는 창"),
        detail: format!("창 {} · 패널 {count}개", window.index),
    }
}
pub fn pane_label(pane: &RemotePane) -> Label {
    let command = named(&pane.command, "프로세스 미확인");
    let mode = display_text(&pane.mode);
    Label {
        title: named(&pane.title, &command),
        detail: format!(
            "{command} · {}×{}{}",
            pane.width,
            pane.height,
            if mode.is_empty() {
                String::new()
            } else {
                format!(" · {mode}")
            }
        ),
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Session,
    Window,
    Pane,
}
#[derive(Clone, Debug)]
pub struct Row {
    pub kind: Kind,
    pub key: String,
    pub session: String,
    pub window: Option<String>,
    pub pane: Option<String>,
    pub label: Label,
    pub children: Vec<Row>,
}
impl Row {
    pub fn selected(&self, state: &dgl_core::connection::State) -> bool {
        state.session.as_ref() == Some(&self.session)
            && if let Some(pane) = &self.pane {
                state.pane.as_ref() == Some(pane)
            } else if let Some(window) = &self.window {
                state
                    .remote
                    .panes
                    .iter()
                    .any(|p| state.pane.as_ref() == Some(&p.id) && p.window == *window)
            } else {
                true
            }
    }
}
pub fn rows(state: &RemoteState, show_all: bool) -> Vec<Row> {
    state
        .sessions
        .iter()
        .map(|s| {
            let mut row = Row {
                kind: Kind::Session,
                key: s.id.clone(),
                session: s.id.clone(),
                window: None,
                pane: None,
                label: session_label(state, s),
                children: vec![],
            };
            let windows: Vec<_> = state.windows.iter().filter(|w| w.session == s.id).collect();
            for w in &windows {
                let window = Row {
                    kind: Kind::Window,
                    key: format!("{}/{}", s.id, w.id),
                    session: s.id.clone(),
                    window: Some(w.id.clone()),
                    pane: None,
                    label: window_label(state, w),
                    children: vec![],
                };
                row.children.push(window);
            }
            if !show_all && windows.len() == 1 {
                let window = row.children.remove(0);
                row.window = window.window;
                row.pane = None;
                row.label.detail = format!(
                    "{}: {} · {}",
                    windows[0].index,
                    window.label.title,
                    window
                        .label
                        .detail
                        .split(" · ")
                        .nth(1)
                        .unwrap_or("패널 0개")
                );
                row.children = window.children;
            }
            row
        })
        .collect()
}
pub fn phase_label(phase: &Phase) -> &'static str {
    match phase {
        Phase::Closed => "연결 안 됨",
        Phase::Connecting => "연결 중",
        Phase::TrustRequired => "호스트키 확인",
        Phase::Authenticating => "인증 중",
        Phase::Selecting => "세션 선택",
        Phase::Restoring => "터미널 복원 중",
        Phase::Ready => "연결됨",
        Phase::Reconnecting => "재접속 중",
        Phase::MissingSession => "이전 세션 없음",
        Phase::Failed => "연결 실패",
        Phase::Shell => "연결됨 · 일반 SSH",
    }
}
pub fn target_label(state: &RemoteState, session: Option<&str>, pane: Option<&str>) -> String {
    let mut parts = vec![];
    if let Some(s) = state
        .sessions
        .iter()
        .find(|s| Some(s.id.as_str()) == session)
    {
        parts.push(named(&s.name, "이름 없는 세션"));
    }
    if let Some(p) = state.panes.iter().find(|p| Some(p.id.as_str()) == pane) {
        if let Some(w) = state
            .windows
            .iter()
            .find(|w| w.id == p.window && Some(w.session.as_str()) == session)
        {
            parts.push(named(&w.name, "이름 없는 창"));
        }
        parts.push(pane_label(p).title);
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_hierarchy_projection() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../fixtures/migration/hierarchy-v2.json"))
                .unwrap();
        fn flatten(rows: &[Row], depth: usize, out: &mut Vec<serde_json::Value>) {
            for r in rows {
                out.push(serde_json::json!([
                    r.key,
                    depth,
                    r.label.title,
                    r.label.detail,
                    r.window,
                    r.pane,
                    !r.children.is_empty()
                ]));
                flatten(&r.children, depth + 1, out);
            }
        }
        for case in fixture.as_array().unwrap() {
            let state: RemoteState = serde_json::from_value(case["remote"].clone()).unwrap();
            for (all, key) in [(false, "compact"), (true, "all")] {
                let mut actual = vec![];
                flatten(&rows(&state, all), 0, &mut actual);
                assert_eq!(
                    serde_json::json!(actual),
                    case[key],
                    "{} / {all}",
                    case["name"]
                );
            }
        }
    }
}

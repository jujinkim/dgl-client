//! Display-only projection. Stable tmux IDs remain the identity and selection target.
use dgl_core::{
    connection::Phase,
    model::{RemotePane, RemoteSession, RemoteState, RemoteWindow},
};
use dgl_core::{i18n::t, tr};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub title: String,
    pub subtitle: String,
    pub detail: String,
}
impl Label {
    pub fn full_text(&self) -> String {
        [&self.title, &self.subtitle, &self.detail]
            .into_iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    }
}
pub fn tab_title(text: &str) -> String {
    let text = display_text(text);
    match text.grapheme_indices(true).nth(20) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text,
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
fn live_subtitle(
    state: &RemoteState,
    name: &str,
    fallback: &str,
    window: Option<&RemoteWindow>,
) -> String {
    let name = named(name, fallback);
    let title = window
        .and_then(|w| state.panes.iter().find(|p| p.window == w.id && p.active))
        .map(|p| display_text(&p.title))
        .unwrap_or_default();
    if title == name { String::new() } else { title }
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
        title: named(&session.name, t("hierarchy.unnamed_session")),
        subtitle: live_subtitle(
            state,
            &session.name,
            t("hierarchy.unnamed_session"),
            windows.iter().find(|w| w.active).copied(),
        ),
        detail: tr!("hierarchy.windows_panes", windows.len(), panes),
    }
}
/// The tmux session name shown where input is aimed; the stable ID stays the identity.
pub fn session_name(state: &RemoteState, id: &str) -> String {
    state
        .sessions
        .iter()
        .find(|s| s.id == id)
        .map(|s| named(&s.name, t("hierarchy.unnamed_session")))
        .unwrap_or_else(|| display_text(id))
}
/// A tab's session name and its title: the active window name and pane title, minus repeats.
/// The title is empty when neither adds anything beyond the name.
pub fn session_tab_parts(state: &RemoteState, session: &RemoteSession) -> (String, String) {
    let window = state
        .windows
        .iter()
        .find(|w| w.session == session.id && w.active);
    let pane = window.and_then(|w| state.panes.iter().find(|p| p.window == w.id && p.active));
    let name = named(&session.name, t("hierarchy.unnamed_session"));
    let mut parts: Vec<String> = Vec::new();
    for text in [
        window.map(|w| w.name.as_str()),
        pane.map(|p| p.title.as_str()),
    ]
    .into_iter()
    .flatten()
    {
        let text = display_text(text);
        if !text.is_empty() && text != name && !parts.contains(&text) {
            parts.push(text);
        }
    }
    (name, parts.join(" · "))
}
pub fn window_label(state: &RemoteState, window: &RemoteWindow) -> Label {
    let count = state.panes.iter().filter(|p| p.window == window.id).count();
    Label {
        title: named(&window.name, t("hierarchy.unnamed_window")),
        subtitle: live_subtitle(
            state,
            &window.name,
            t("hierarchy.unnamed_window"),
            Some(window),
        ),
        detail: tr!("hierarchy.window_panes", window.index, count),
    }
}
pub fn pane_label(pane: &RemotePane) -> Label {
    let command = named(&pane.command, t("hierarchy.process_unknown"));
    let mode = display_text(&pane.mode);
    Label {
        title: named(&pane.title, &command),
        subtitle: String::new(),
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
                if row.label.subtitle == named(&windows[0].name, t("hierarchy.unnamed_window")) {
                    row.label.subtitle.clear();
                }
                row.label.subtitle = [
                    format!(
                        "{}: {}",
                        windows[0].index,
                        named(&windows[0].name, t("hierarchy.unnamed_window")),
                    ),
                    row.label.subtitle,
                ]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
                row.label.detail = tr!(
                    "hierarchy.panes",
                    state
                        .panes
                        .iter()
                        .filter(|p| p.window == windows[0].id)
                        .count()
                );
                row.children = window.children;
            }
            row
        })
        .collect()
}
pub fn phase_label(phase: &Phase) -> &'static str {
    match phase {
        Phase::Closed => t("common.disconnected"),
        Phase::Connecting => t("hierarchy.connecting"),
        Phase::TrustRequired => t("hierarchy.verify_host_key"),
        Phase::Authenticating => t("hierarchy.authenticating"),
        Phase::Selecting => t("common.select_session"),
        Phase::Restoring => t("hierarchy.restoring_terminal"),
        Phase::Ready => t("hierarchy.connected"),
        Phase::Reconnecting => t("hierarchy.reconnecting"),
        Phase::MissingSession => t("hierarchy.previous_session_missing"),
        Phase::Failed => t("hierarchy.connection_failed"),
        Phase::Shell => t("hierarchy.connected_plain_ssh"),
    }
}
pub fn target_label(state: &RemoteState, session: Option<&str>, pane: Option<&str>) -> String {
    let mut parts = vec![];
    if let Some(s) = state
        .sessions
        .iter()
        .find(|s| Some(s.id.as_str()) == session)
    {
        parts.push(named(&s.name, t("hierarchy.unnamed_session")));
    }
    if let Some(p) = state.panes.iter().find(|p| Some(p.id.as_str()) == pane) {
        if let Some(w) = state
            .windows
            .iter()
            .find(|w| w.id == p.window && Some(w.session.as_str()) == session)
        {
            parts.push(named(&w.name, t("hierarchy.unnamed_window")));
        }
        parts.push(pane_label(p).title);
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_name_uses_the_tmux_name_and_falls_back_safely() {
        let state = RemoteState {
            sessions: vec![
                RemoteSession {
                    id: "$1".into(),
                    name: "work\u{202e}".into(),
                    attached: 1,
                },
                RemoteSession {
                    id: "$2".into(),
                    name: " ".into(),
                    attached: 0,
                },
            ],
            ..Default::default()
        };
        assert_eq!(session_name(&state, "$1"), "work");
        assert_eq!(session_name(&state, "$2"), t("hierarchy.unnamed_session"));
        assert_eq!(session_name(&state, "$3"), "$3");
    }
    #[test]
    fn tab_titles_follow_active_window_and_pane_and_ignore_other_sessions() {
        for glyph in ["a", "한", "e\u{301}", "한", "😀", "👩🏽‍💻", "🇰🇷"] {
            for count in [19, 20, 21] {
                let input = glyph.repeat(count);
                assert_eq!(
                    tab_title(&input),
                    if count > 20 {
                        format!("{}…", glyph.repeat(20))
                    } else {
                        input
                    }
                );
            }
        }
        assert_eq!(tab_title("\n\u{202e}"), "");
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../fixtures/migration/hierarchy-v2.json"))
                .unwrap();
        let mut state: RemoteState = serde_json::from_value(fixture[3]["remote"].clone()).unwrap();
        state.windows[1].active = false;
        state.panes[1].active = false;
        state.panes[3].active = false;
        let session_tab_title = |state: &RemoteState, session: &RemoteSession| {
            let (name, title) = session_tab_parts(state, session);
            [name, title]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ")
        };
        let title = |state: &RemoteState| session_tab_title(state, &state.sessions[0]);
        assert_eq!(
            session_tab_parts(&state, &state.sessions[0]),
            ("개발".into(), "편집 · 작업".into())
        );
        assert_eq!(title(&state), "개발 · 편집 · 작업");
        state.panes[2].title = "Hidden".into();
        state.panes[4].title = "Other session".into();
        assert_eq!(title(&state), "개발 · 편집 · 작업");
        state.sessions[0].name = " 프로젝트\n".into();
        state.windows[0].name = "Editor".into();
        state.panes[0].title = "Building 😀\u{202e}\n".into();
        assert_eq!(title(&state), "프로젝트 · Editor · Building 😀");
        state.panes[0].active = false;
        state.panes[1].active = true;
        state.panes[1].title = "Review".into();
        assert_eq!(title(&state), "프로젝트 · Editor · Review");
        state.windows[0].active = false;
        state.windows[1].active = true;
        assert_eq!(title(&state), "프로젝트 · 서버 · Hidden");
        state.windows[1].name = "프로젝트".into();
        state.panes[2].title = " 프로젝트\u{2069}".into();
        assert_eq!(title(&state), "프로젝트");
        state.windows[1].name = "\t\n".into();
        state.panes[2].title.clear();
        assert_eq!(title(&state), "프로젝트");
        assert_eq!(
            session_tab_title(&state, &state.sessions[1]),
            "개발 · Other session"
        );
        assert_eq!(session_tab_title(&state, &state.sessions[2]), "빈 세션");
        state.windows.clear();
        assert_eq!(title(&state), "프로젝트");
        assert_eq!(state.sessions[0].id, "$1");
        assert_eq!(state.panes[1].id, "%2");
    }
    #[test]
    fn live_titles_follow_active_panes_without_changing_identity() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../fixtures/migration/hierarchy-v2.json"))
                .unwrap();
        let mut state: RemoteState = serde_json::from_value(fixture[3]["remote"].clone()).unwrap();
        state.windows[1].active = false;
        state.panes[1].active = false;
        state.panes[3].active = false;
        let keys: Vec<_> = rows(&state, true)
            .into_iter()
            .map(|r| {
                (
                    r.key,
                    r.children.into_iter().map(|c| c.key).collect::<Vec<_>>(),
                )
            })
            .collect();
        assert_eq!(session_label(&state, &state.sessions[0]).subtitle, "작업");
        state.panes[0].title = "Building 😀\u{202e}\n".into();
        assert_eq!(
            session_label(&state, &state.sessions[0]).subtitle,
            "Building 😀"
        );
        assert_eq!(
            window_label(&state, &state.windows[0]).subtitle,
            "Building 😀"
        );
        state.panes[0].active = false;
        state.panes[1].active = true;
        state.panes[1].title = "Review".into();
        assert_eq!(session_label(&state, &state.sessions[0]).subtitle, "Review");
        state.windows[0].active = false;
        state.windows[1].active = true;
        state.panes[2].title = "Tests".into();
        assert_eq!(session_label(&state, &state.sessions[0]).subtitle, "Tests");
        assert_eq!(window_label(&state, &state.windows[0]).subtitle, "Review");
        state.panes[4].title = "Other session".into();
        assert_eq!(
            session_label(&state, &state.sessions[1]).subtitle,
            "Other session"
        );
        state.panes[2].title.clear();
        assert_eq!(session_label(&state, &state.sessions[0]).subtitle, "");
        state.panes[2].title = "개발".into();
        assert_eq!(session_label(&state, &state.sessions[0]).subtitle, "");
        assert_eq!(session_label(&state, &state.sessions[0]).title, "개발");
        assert_eq!(window_label(&state, &state.windows[0]).title, "편집");
        assert_eq!(state.sessions[0].name, "개발");
        assert_eq!(state.windows[0].name, "편집");
        assert_eq!(
            rows(&state, true)
                .into_iter()
                .map(|r| (
                    r.key,
                    r.children.into_iter().map(|c| c.key).collect::<Vec<_>>()
                ))
                .collect::<Vec<_>>(),
            keys
        );
    }
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
                    r.label.subtitle,
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

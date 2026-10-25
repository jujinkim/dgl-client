//! Plain text labels; IDs remain the identity of every tree item.
use dgl_core::model::{RemotePane, RemoteSession, RemoteState, RemoteWindow};

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
            if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}') {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
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
        title: display_text(&session.name),
        detail: format!("창 {}개 · 패널 {panes}개", windows.len()),
    }
}

pub fn window_label(state: &RemoteState, window: &RemoteWindow) -> Label {
    let panes: Vec<_> = state
        .panes
        .iter()
        .filter(|p| p.window == window.id)
        .collect();
    let active = panes.iter().find(|p| p.active);
    let title = active.map(|p| display_text(&p.title)).unwrap_or_default();
    let name = display_text(&window.name);
    let session = state
        .sessions
        .iter()
        .find(|s| s.id == window.session)
        .map(|s| display_text(&s.name))
        .unwrap_or_else(|| window.session.clone());
    let command = active.map(|p| display_text(&p.command)).unwrap_or_default();
    Label {
        title: if title.is_empty() {
            name.clone()
        } else {
            title
        },
        detail: format!(
            "{session} · {}: {name}{} · 패널 {}개",
            window.index,
            if command.is_empty() {
                String::new()
            } else {
                format!(" / {command}")
            },
            panes.len()
        ),
    }
}

pub fn pane_label(pane: &RemotePane) -> Label {
    let title = display_text(&pane.title);
    let command = display_text(&pane.command);
    let mode = display_text(&pane.mode);
    Label {
        title: if title.is_empty() {
            format!("{command} {}", pane.id).trim().to_string()
        } else {
            title
        },
        detail: format!(
            "{} · {command} · {}×{}{}",
            pane.id,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn shared_hierarchy_labels_and_active_pane_fallback() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../fixtures/migration/hierarchy-v1.json"))
                .unwrap();
        let mut remote: RemoteState = serde_json::from_value(fixture["remote"].clone()).unwrap();
        for (kind, labels) in [
            (
                "sessions",
                remote
                    .sessions
                    .iter()
                    .map(|s| session_label(&remote, s))
                    .collect::<Vec<_>>(),
            ),
            (
                "windows",
                remote
                    .windows
                    .iter()
                    .map(|w| window_label(&remote, w))
                    .collect(),
            ),
            ("panes", remote.panes.iter().map(pane_label).collect()),
        ] {
            for (label, expected) in labels
                .iter()
                .zip(fixture["expected"][kind].as_array().unwrap())
            {
                assert_eq!(label.title, expected[0].as_str().unwrap());
                assert_eq!(label.detail, expected[1].as_str().unwrap());
                assert_eq!(
                    label.full_text(),
                    format!("{}\n{}", label.title, label.detail)
                );
            }
        }
        remote.panes[0].active = true;
        remote.panes[1].active = false;
        assert_eq!(
            window_label(&remote, &remote.windows[0]).title,
            "비활성 패널"
        );
        remote.panes[0].title.clear();
        assert_eq!(
            window_label(&remote, &remote.windows[0]).title,
            "실제 창 이름"
        );
        assert_eq!(pane_label(&remote.panes[0]).title, "sh %1");
    }
}

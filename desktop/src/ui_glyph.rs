use super::*;
use dgl_core::agent::Kind;

pub(super) enum ButtonLabel {
    Text(SharedString),
    Icon(UiIcon),
    IconText(UiIcon, SharedString),
}
impl From<&'static str> for ButtonLabel {
    fn from(value: &'static str) -> Self {
        Self::Text(value.into())
    }
}
impl From<String> for ButtonLabel {
    fn from(value: String) -> Self {
        Self::Text(value.into())
    }
}
impl From<SharedString> for ButtonLabel {
    fn from(value: SharedString) -> Self {
        Self::Text(value)
    }
}
impl From<UiIcon> for ButtonLabel {
    fn from(value: UiIcon) -> Self {
        Self::Icon(value)
    }
}
impl<T: Into<SharedString>> From<(UiIcon, T)> for ButtonLabel {
    fn from((icon, text): (UiIcon, T)) -> Self {
        Self::IconText(icon, text.into())
    }
}

impl UiIcon {
    pub(super) fn description(self) -> &'static str {
        match self {
            Self::Plus => t("common.add"),
            Self::Close => t("common.close"),
            Self::More => t("common.more"),
            Self::Refresh => t("common.refresh"),
            Self::Settings => t("common.settings"),
            Self::ChevronDown => t("common.collapse"),
            Self::ChevronRight => t("common.expand"),
            Self::SplitHorizontal => t("workspace.split_screen_horizontally"),
            Self::SplitVertical => t("workspace.split_screen_vertically"),
            Self::Merge => t("workspace.merge_groups"),
            Self::Grip => t("common.reorder"),
            Self::ArrowUp => t("common.move_up"),
            Self::ArrowDown => t("common.move_down"),
            Self::ArrowLeft => t("pane.focus_pane_left"),
            Self::ArrowRight => t("pane.focus_pane_right"),
            _ => t("agent.status_label"),
        }
    }
    pub(super) fn agent(kind: Option<Kind>) -> Self {
        match kind {
            Some(Kind::Question) => Self::Question,
            Some(Kind::Approval) => Self::Approval,
            Some(Kind::Response) => Self::Response,
            Some(Kind::Error) => Self::Error,
            Some(Kind::Activity) => Self::Refresh,
            Some(Kind::Ended) => Self::Ended,
            None => Self::Unknown,
        }
    }
}

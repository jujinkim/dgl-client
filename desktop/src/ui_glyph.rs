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
            Self::Plus => t("추가"),
            Self::Close => t("닫기"),
            Self::More => t("더보기"),
            Self::Refresh => t("새로고침"),
            Self::Settings => t("설정"),
            Self::ChevronDown => t("접기"),
            Self::ChevronRight => t("펼치기"),
            Self::SplitHorizontal => t("화면 좌우 분할"),
            Self::SplitVertical => t("화면 상하 분할"),
            Self::Merge => t("그룹 합치기"),
            Self::Grip => t("순서 이동"),
            Self::ArrowUp => t("위로 이동"),
            Self::ArrowDown => t("아래로 이동"),
            Self::ArrowLeft => t("왼쪽 패널로 이동"),
            Self::ArrowRight => t("오른쪽 패널로 이동"),
            _ => t("에이전트 상태"),
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

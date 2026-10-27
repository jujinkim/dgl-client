use super::*;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const WEBSITE: &str = "https://ddal.ggak.life";
pub const REPOSITORY: &str = "https://github.com/jujinkim/dgl-client";

pub fn base_title() -> String {
    format!("DaGaLi v{VERSION}")
}

impl Desktop {
    pub(super) fn app_window_title(&self) -> String {
        match self.active_tab() {
            Some(tab) => format!("{} - {}", base_title(), tab.title()),
            None => base_title(),
        }
    }

    pub(super) fn about_app(&self, cx: &mut Context<Self>) -> Div {
        let mut content = div()
            .flex()
            .flex_col()
            .items_center()
            .gap_4()
            .child(div().text_xl().child(t("About app")))
            .child(self.button("about-back", t("뒤로"), cx, |s, _, _| s.settings_back()))
            .child(img("app-icon.png").size(px(80.)).rounded_xl())
            .child(div().text_2xl().child("DaGaLi"))
            .child(tr!("버전 {0}", VERSION))
            .child("Reconnect less. Resume more.");
        for (id, label, url) in [
            ("about-website", t("앱 사이트"), WEBSITE),
            ("about-repository", t("GitHub 저장소"), REPOSITORY),
        ] {
            content = content.child(
                self.button(id, label, cx, move |_, _, cx| cx.open_url(url))
                    .child(div().text_xs().child(url)),
            );
        }
        content
    }
}

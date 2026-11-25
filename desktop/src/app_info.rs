use super::*;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const WEBSITE: &str = "https://ddal.ggak.life";
pub const REPOSITORY: &str = "https://github.com/jujinkim/dgl-client";

pub fn base_title() -> String {
    format!("DaGaLi v{VERSION}")
}

impl Desktop {
    pub(super) fn app_window_title(&self) -> String {
        if let Some(tab) = self.workspace.file_view.and_then(|id| self.files.get(&id)) {
            return format!("{} - {}", base_title(), tab.title);
        }
        match self.active_tab() {
            Some(tab) => format!("{} - {}", base_title(), tab.title()),
            None => base_title(),
        }
    }

    /// The about page: the app identity and its links.
    pub(super) fn about_app(&self, cx: &mut Context<Self>) -> Div {
        let hero = div()
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .py_4()
            .child(img("app-icon.png").size(px(80.)).rounded_xl())
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("DaGaLi"),
            )
            .child(
                div()
                    .text_size(px(settings_ui::BODY))
                    .child(tr!("settings.about_version", VERSION)),
            )
            .child(self.settings_note("Reconnect less. Resume more."));
        let links = [
            ("about-website", t("settings.about_website"), WEBSITE),
            ("about-repository", t("settings.about_github"), REPOSITORY),
        ]
        .into_iter()
        .map(|(id, label, url)| {
            self.settings_link(id, label, Some(url.into()), cx, move |_, _, cx| {
                cx.open_url(url)
            })
            .into_any_element()
        })
        .collect();
        div()
            .flex()
            .flex_col()
            .gap(px(settings_ui::PAGE_GAP))
            .child(hero)
            .child(self.settings_group("about-links", None, None, links))
    }
}

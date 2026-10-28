use super::*;
use dgl_core::connection::State;
use std::borrow::Cow;

#[derive(Clone, Copy)]
pub enum LaunchCommand {
    Claude,
    ClaudeBypass,
    Codex,
    CodexBypass,
    Agy,
    AgyBypass,
    Grok,
    GrokBypass,
}
impl LaunchCommand {
    pub fn bytes(self) -> Vec<u8> {
        match self {
            Self::Claude => b"claude\r".as_slice(),
            Self::ClaudeBypass => b"claude --dangerously-skip-permissions\r",
            Self::Codex => b"codex\r",
            Self::CodexBypass => b"codex --yolo\r",
            Self::Agy => b"agy\r",
            Self::AgyBypass => b"agy --dangerously-skip-permissions\r",
            Self::Grok => b"grok\r",
            Self::GrokBypass => b"grok --always-approve\r",
        }
        .to_vec()
    }
}

// Size/remote-list updates must not dismiss the menu, but a new connection or
// pane generation must invalidate even a return to the same pane ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchContext {
    group: GroupId,
    tab: Option<TabId>,
    target: Option<LaunchTarget>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct LaunchTarget {
    connection: u64,
    connection_generation: u64,
    pane_generation: u64,
    phase: Phase,
    session: Option<String>,
    pane: Option<String>,
}
impl LaunchContext {
    fn new(group: GroupId, tab: Option<TabId>, state: Option<&State>) -> Self {
        Self {
            group,
            tab,
            target: state.map(|s| LaunchTarget {
                connection: s.handle,
                connection_generation: s.generation.connection,
                pane_generation: s.generation.pane,
                phase: s.phase.clone(),
                session: s.session.clone(),
                pane: s.pane.clone(),
            }),
        }
    }
    fn accepts_input(&self) -> bool {
        self.tab.is_some()
            && matches!(
                self.target,
                Some(LaunchTarget {
                    phase: Phase::Ready | Phase::Shell,
                    pane: Some(_),
                    ..
                })
            )
    }
}

#[derive(Default)]
pub struct LaunchMenu {
    pub opened: Option<LaunchContext>,
}
impl LaunchMenu {
    fn take_command(
        &mut self,
        current: &LaunchContext,
        allowed: bool,
        command: LaunchCommand,
    ) -> Option<Vec<u8>> {
        let opened = self.opened.take()?;
        (allowed && opened == *current && current.accepts_input()).then(|| command.bytes())
    }
}

pub struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(match path {
            "app-icon.png" => Some(Cow::Borrowed(include_bytes!(
                "../../assets/app-icon/icon.png"
            ))),
            "launcher/claude.svg" => Some(Cow::Borrowed(include_bytes!(
                "../../assets/launcher/claude.svg"
            ))),
            "launcher/openai.svg" => Some(Cow::Borrowed(include_bytes!(
                "../../assets/launcher/openai.svg"
            ))),
            "launcher/bolt.svg" => Some(Cow::Borrowed(include_bytes!(
                "../../assets/launcher/bolt.svg"
            ))),
            "launcher/agy.svg" => Some(Cow::Borrowed(include_bytes!(
                "../../assets/launcher/agy.svg"
            ))),
            "launcher/grok.svg" => Some(Cow::Borrowed(include_bytes!(
                "../../assets/launcher/grok.svg"
            ))),
            _ => UiIcon::ALL
                .iter()
                .find(|icon| icon.path() == path)
                .map(|icon| Cow::Borrowed(icon.bytes())),
        })
    }
    fn list(&self, _: &str) -> gpui::Result<Vec<SharedString>> {
        let mut paths = vec![
            "app-icon.png".into(),
            "launcher/claude.svg".into(),
            "launcher/openai.svg".into(),
            "launcher/bolt.svg".into(),
            "launcher/agy.svg".into(),
            "launcher/grok.svg".into(),
        ];
        paths.extend(UiIcon::ALL.iter().map(|icon| icon.path().into()));
        Ok(paths)
    }
}

impl Desktop {
    fn launch_context(&self) -> LaunchContext {
        let tab = self.active_tab();
        LaunchContext::new(
            self.workspace.active,
            tab.map(|t| t.id),
            tab.map(|t| t.connection.state()).as_deref(),
        )
    }
    pub fn sync_launcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.launcher.opened.as_ref().is_some_and(|opened| {
            *opened != self.launch_context() || self.dialog.is_some() || self.snackbar.is_some()
        }) {
            self.dismiss_launcher(window, cx);
        }
    }
    fn dismiss_launcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.launcher.opened = None;
        if let Some(tab) = self.active_tab() {
            window.focus(&tab.terminal.focus_handle(cx));
        } else {
            window.focus(&self.focus);
        }
        cx.notify();
    }
    fn launch(&mut self, command: LaunchCommand, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.launch_context();
        if let Some(bytes) = self.launcher.take_command(
            &current,
            self.dialog.is_none() && self.snackbar.is_none(),
            command,
        ) && let Some(tab) = self.active_tab()
        {
            tab.terminal.update(cx, |terminal, _| terminal.send(bytes));
        }
        self.dismiss_launcher(window, cx);
    }
    pub fn launcher_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        self.input_button("cli-launcher", "…", cx, |s, window, cx| {
            if s.launcher.opened.is_some() {
                s.dismiss_launcher(window, cx);
            } else {
                s.menu = None;
                s.launcher.opened = Some(s.launch_context());
                // Keep menu keys out of the remote terminal.
                window.focus(&s.focus);
            }
        })
        .w(px(44.))
        .h(px(32.))
        .rounded_md()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(20.))
        .native_tooltip(t("CLI 실행 메뉴"))
    }
    pub fn render_launcher(&self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        self.launcher.opened.as_ref()?;
        let p = self.palette;
        let enabled = self.launch_context().accepts_input();
        let mut menu = div()
            .id("cli-launcher-menu")
            .debug_selector(|| "cli-launcher-menu".into())
            .absolute()
            .right(px(4.))
            .bottom(px(66.))
            .flex()
            .flex_col()
            .items_start()
            .bg(rgb(p.panel))
            .border_1()
            .border_color(rgb(p.line))
            .shadow_md()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        for (service, icon, normal, bypass) in [
            (
                "Claude Code",
                "launcher/claude.svg",
                LaunchCommand::Claude,
                LaunchCommand::ClaudeBypass,
            ),
            (
                "Codex",
                "launcher/openai.svg",
                LaunchCommand::Codex,
                LaunchCommand::CodexBypass,
            ),
            (
                "AGY",
                "launcher/agy.svg",
                LaunchCommand::Agy,
                LaunchCommand::AgyBypass,
            ),
            (
                "Grok Build",
                "launcher/grok.svg",
                LaunchCommand::Grok,
                LaunchCommand::GrokBypass,
            ),
        ] {
            let mut row = div().flex().h(px(44.));
            for (label, icon, command, suffix) in [
                (Some(service), icon, normal, "normal"),
                (None, "launcher/bolt.svg", bypass, "bypass"),
            ] {
                let id = format!("launch-{service}-{suffix}");
                let experimental = matches!(normal, LaunchCommand::Agy | LaunchCommand::Grok);
                let item = div()
                    .id(ElementId::Name(id.clone().into()))
                    .debug_selector(move || id.clone())
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .px_3()
                    .h_full()
                    .flex_shrink_0()
                    .min_w(px(44.))
                    .text_color(rgb(if enabled { p.fg } else { p.muted }))
                    .when(enabled, |d| {
                        d.cursor_pointer().hover(move |s| s.bg(rgb(p.selection)))
                    })
                    .child(div().size(px(18.)).flex_shrink_0().child(
                        svg().path(icon).size_full().text_color(rgb(if enabled {
                            p.fg
                        } else {
                            p.muted
                        })),
                    ))
                    .children(label)
                    .native_tooltip(tr!(
                        "{0} {1} 실행{2}",
                        service,
                        suffix,
                        if experimental {
                            t(" · 실험 지원")
                        } else {
                            ""
                        }
                    ))
                    .on_click(cx.listener(move |s, _, w, cx| {
                        if enabled {
                            s.launch(command, w, cx);
                        }
                        cx.stop_propagation();
                    }));
                row = row.child(item);
            }
            menu = menu.child(row);
        }
        let mut settings_entry = div()
            .id("launcher-shortcut-settings")
            .debug_selector(|| "launcher-shortcut-settings".into())
            .h(px(45.))
            .border_t_1()
            .border_color(rgb(p.line))
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .cursor_pointer()
            .hover(move |s| s.bg(rgb(p.selection)))
            .child(
                svg()
                    .path(UiIcon::Settings.path())
                    .size(px(18.))
                    .text_color(rgb(p.fg)),
            )
            .child(t("단축키 설정"))
            .on_click(cx.listener(|s, _, window, cx| {
                s.dismiss_launcher(window, cx);
                s.menu = None;
                s.settings = settings::SettingsState::default();
                s.dialog = Some(Dialog::Shortcuts);
                cx.notify();
                cx.stop_propagation();
            }));
        settings_entry.style().align_self = Some(gpui::AlignSelf::Stretch);
        menu = menu.child(settings_entry);
        Some(
            div()
                .id("cli-launcher-overlay")
                .absolute()
                .inset_0()
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|s, _, w, cx| {
                        s.dismiss_launcher(w, cx);
                        cx.stop_propagation();
                    }),
                )
                .child(menu),
        )
    }
    pub fn launcher_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss_launcher(window, cx);
        cx.stop_propagation();
        window.prevent_default();
    }
}

#[cfg(test)]
mod tests {
    use super::{LaunchCommand, LaunchContext, LaunchMenu, LaunchTarget, Phase};
    fn target() -> LaunchContext {
        LaunchContext {
            group: 1,
            tab: Some(1),
            target: Some(LaunchTarget {
                connection: 1,
                connection_generation: 1,
                pane_generation: 1,
                phase: Phase::Ready,
                session: Some("$1".into()),
                pane: Some("%1".into()),
            }),
        }
    }
    #[test]
    fn commands_are_single_use_and_require_the_same_live_target() {
        for (command, wire) in [
            (LaunchCommand::Claude, "claude\r"),
            (
                LaunchCommand::ClaudeBypass,
                "claude --dangerously-skip-permissions\r",
            ),
            (LaunchCommand::Codex, "codex\r"),
            (LaunchCommand::CodexBypass, "codex --yolo\r"),
            (LaunchCommand::Agy, "agy\r"),
            (
                LaunchCommand::AgyBypass,
                "agy --dangerously-skip-permissions\r",
            ),
            (LaunchCommand::Grok, "grok\r"),
            (LaunchCommand::GrokBypass, "grok --always-approve\r"),
        ] {
            let context = target();
            let mut menu = LaunchMenu {
                opened: Some(context.clone()),
            };
            assert_eq!(
                menu.take_command(&context, true, command),
                Some(wire.as_bytes().to_vec())
            );
            assert_eq!(menu.take_command(&context, true, command), None);
            for mutation in 0..9 {
                let mut current = context.clone();
                match mutation {
                    0 => current.group += 1,
                    1 => current.tab = Some(2),
                    2 => current.target.as_mut().unwrap().connection += 1,
                    3 => current.target.as_mut().unwrap().connection_generation += 1,
                    4 => current.target.as_mut().unwrap().pane_generation += 1,
                    5 => current.target.as_mut().unwrap().phase = Phase::Reconnecting,
                    6 => current.target.as_mut().unwrap().session = Some("$2".into()),
                    7 => current.target.as_mut().unwrap().pane = Some("%2".into()),
                    _ => (),
                }
                menu.opened = Some(context.clone());
                assert_eq!(menu.take_command(&current, mutation != 8, command), None);
            }
            for phase in [
                Phase::Connecting,
                Phase::Restoring,
                Phase::Reconnecting,
                Phase::Closed,
                Phase::Failed,
                Phase::Selecting,
            ] {
                let mut blocked = context.clone();
                blocked.target.as_mut().unwrap().phase = phase;
                menu.opened = Some(blocked.clone());
                assert_eq!(menu.take_command(&blocked, true, command), None);
            }
            let mut shell = context;
            shell.target.as_mut().unwrap().phase = Phase::Shell;
            menu.opened = Some(shell.clone());
            assert_eq!(
                menu.take_command(&shell, true, command),
                Some(wire.as_bytes().to_vec())
            );
        }
    }
}

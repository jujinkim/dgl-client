use super::*;
use crate::launcher_accessibility::Control;
use dgl_core::agent::Agent;
use dgl_core::connection::State;
use dgl_core::launcher::{AgentLaunch, LAUNCH_AGENTS};
use std::borrow::Cow;

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
        command: impl FnOnce() -> Option<Vec<u8>>,
    ) -> Option<Vec<u8>> {
        let opened = self.opened.take()?;
        if allowed && opened == *current && current.accepts_input() {
            command()
        } else {
            None
        }
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
    /// A CLI already running in the input pane would take the command as prompt text.
    /// Checked when painting and again when clicking, without dismissing the menu.
    fn launch_blocker(&self) -> Option<Agent> {
        let state = self.active_tab()?.connection.state();
        state
            .agents
            .agent_in_pane(state.pane.as_deref()?, dgl_core::agent::now_ms())
    }
    pub(super) fn agent_launch(&self, agent: Agent) -> Cow<'_, AgentLaunch> {
        self.local
            .preferences
            .agent_launch
            .get(&agent)
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(AgentLaunch::defaults(agent)))
    }
    pub(super) fn update_agent_launch(
        &mut self,
        agent: Agent,
        update: impl FnOnce(&mut AgentLaunch),
    ) -> bool {
        let mut config = self.agent_launch(agent).into_owned();
        update(&mut config);
        let previous = self.local.preferences.agent_launch.insert(agent, config);
        if self.save() {
            return true;
        }
        if let Some(previous) = previous {
            self.local.preferences.agent_launch.insert(agent, previous);
        } else {
            self.local.preferences.agent_launch.remove(&agent);
        }
        false
    }
    fn launch(&mut self, agent: Agent, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab() {
            // Build the context, running-agent check and final input target from one core snapshot.
            let state = tab.connection.state();
            let current = LaunchContext::new(self.workspace.active, Some(tab.id), Some(&state));
            let target = state.input_target();
            let allowed = state.accepts_input()
                && self.dialog.is_none()
                && self.snackbar.is_none()
                && state.pane.as_deref().is_some_and(|p| {
                    state
                        .agents
                        .agent_in_pane(p, dgl_core::agent::now_ms())
                        .is_none()
                });
            let powershell = cfg!(windows) && tab.host.id == local_host::ID;
            let terminal = tab.terminal.clone();
            let config = self.local.preferences.agent_launch.get(&agent);
            if let Some(bytes) = self.launcher.take_command(&current, allowed, || {
                config
                    .map(Cow::Borrowed)
                    .unwrap_or_else(|| Cow::Owned(AgentLaunch::defaults(agent)))
                    .command(agent, powershell)
            }) && let Some(target) = target
            {
                terminal.update(cx, |t, _| t.send_to(target, bytes));
            }
        }
        self.dismiss_launcher(window, cx);
    }
    pub(super) fn activate_launcher_control(
        &mut self,
        control: Control,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.dialog.is_some() {
            return;
        }
        match control {
            Control::Menu => {
                if self.launcher.opened.is_some() {
                    self.dismiss_launcher(window, cx);
                } else {
                    self.menu = None;
                    self.launcher.opened = Some(self.launch_context());
                    window.focus(&self.focus);
                }
            }
            _ if self.launcher.opened.as_ref() != Some(&self.launch_context()) => return,
            Control::Run(agent) => self.launch(agent, window, cx),
            Control::Bypass(agent) => {
                self.update_agent_launch(agent, |config| {
                    config.bypass_checked = !config.bypass_checked
                });
            }
            Control::Option(agent, index) => {
                self.update_agent_launch(agent, |config| {
                    if let Some(option) = config.options.get_mut(index) {
                        option.checked = !option.checked;
                    }
                });
            }
            Control::Settings => {
                self.dismiss_launcher(window, cx);
                self.menu = None;
                self.settings = settings::SettingsState::default();
                self.dialog = Some(Dialog::Shortcuts);
            }
        }
        cx.notify();
    }
    pub fn launcher_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let label = t("launcher.cli_launch_menu");
        let button = self
            .input_button("cli-launcher", "…", cx, |s, window, cx| {
                s.activate_launcher_control(Control::Menu, window, cx);
            })
            .w(px(44.))
            .h(px(32.))
            .rounded_md()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(20.))
            .native_tooltip(label);
        self.launch_access
            .control(button, Control::Menu, label, "", None, true)
    }
    pub fn render_launcher(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        self.launcher.opened.as_ref()?;
        let p = self.palette;
        let blocker = self.launch_blocker();
        let enabled = self.launch_context().accepts_input() && blocker.is_none();
        let max_width = (window.viewport_size().width - px(10.)).max(px(1.));
        let mut menu = div()
            .id("cli-launcher-menu")
            .debug_selector(|| "cli-launcher-menu".into())
            .absolute()
            .right(px(4.))
            .bottom(px(66.))
            .max_w(max_width)
            .flex()
            .flex_col()
            .items_start()
            .bg(rgb(p.panel))
            .border_1()
            .border_color(rgb(p.line))
            .shadow_md()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        if let Some(agent) = blocker {
            let mut notice = div()
                .id("launcher-agent-running")
                .debug_selector(|| "launcher-agent-running".into())
                .flex()
                .flex_col()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(rgb(p.line))
                .text_size(px(13.))
                .text_color(rgb(p.fg))
                .child(tr!("launcher.running_in_pane", agent.label()))
                .child(
                    div()
                        .text_color(rgb(p.muted))
                        .child(t("launcher.launch_another_pane")),
                );
            notice.style().align_self = Some(gpui::AlignSelf::Stretch);
            menu = menu.child(notice);
        }
        for agent in LAUNCH_AGENTS {
            let service = agent.launch_label();
            let mut row = div().flex().h(px(44.)).max_w(max_width - px(2.));
            let experimental = matches!(agent, Agent::Agy | Agent::Grok);
            let help = tr!(
                "launcher.launch",
                service,
                if experimental {
                    t("launcher.experimental_suffix")
                } else {
                    ""
                }
            );
            let id = format!("launch-{service}-normal");
            let fg = if enabled { p.fg } else { p.muted };
            let run = div()
                .id(SharedString::from(id.clone()))
                .debug_selector(move || id)
                .flex()
                .items_center()
                .justify_center()
                .gap_2()
                .px_3()
                .h_full()
                .flex_shrink_0()
                .min_w(px(44.))
                .text_color(rgb(fg))
                .when(enabled, |d| {
                    d.cursor_pointer().hover(move |s| s.bg(rgb(p.selection)))
                })
                .child(
                    div()
                        .size(px(18.))
                        .flex_shrink_0()
                        .child(svg().path(agent.icon()).size_full().text_color(rgb(fg))),
                )
                .child(service)
                .native_tooltip(help.clone())
                .on_click(cx.listener(move |s, _, w, cx| {
                    if enabled {
                        s.activate_launcher_control(Control::Run(agent), w, cx);
                    }
                    cx.stop_propagation();
                }));
            row = row.child(self.launch_access.control(
                run,
                Control::Run(agent),
                &help,
                "",
                None,
                enabled,
            ));

            let checkbox = |control, suffix: &str, label: &str, description: &str, checked| {
                let id = format!("launch-{service}-{suffix}");
                let label_id = format!("{id}-label");
                let check_id = format!("{id}-check");
                let icon = if checked {
                    UiIcon::CheckboxChecked
                } else {
                    UiIcon::Checkbox
                };
                let help = if description.is_empty() {
                    label.to_owned()
                } else {
                    format!("{label}\n{description}")
                };
                let item = div()
                    .id(SharedString::from(id.clone()))
                    .debug_selector(move || id)
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .h_full()
                    .min_w(px(48.))
                    .max_w(px(88.))
                    .px_1()
                    .cursor_pointer()
                    .hover(move |s| s.bg(rgb(p.selection)))
                    .child(
                        div()
                            .debug_selector(move || label_id)
                            .max_w_full()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_size(px(11.))
                            .child(label.to_owned()),
                    )
                    .child(
                        div().debug_selector(move || check_id).size(px(18.)).child(
                            icon.render(if checked { p.accent } else { p.fg })
                                .size_full(),
                        ),
                    )
                    .native_tooltip(help)
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.activate_launcher_control(control, w, cx);
                        cx.stop_propagation();
                    }));
                self.launch_access
                    .control(item, control, label, description, Some(checked), true)
            };
            let config = self.agent_launch(agent);
            row = row.child(checkbox(
                Control::Bypass(agent),
                "bypass",
                "Bypass",
                &tr!("launcher.bypass_description", service),
                config.bypass_checked,
            ));
            if !config.options.is_empty() {
                let mut strip = div()
                    .id(SharedString::from(format!("launch-options-{service}")))
                    .debug_selector(move || format!("launch-options-{service}"))
                    .flex()
                    .min_w_0()
                    .overflow_x_scroll()
                    .h_full();
                for (index, option) in config.options.iter().enumerate() {
                    strip = strip.child(checkbox(
                        Control::Option(agent, index),
                        &format!("option-{index}"),
                        &option.name,
                        &option.description,
                        option.checked,
                    ));
                }
                row = row.child(strip);
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
            .child(t("shortcut.settings"))
            .on_click(cx.listener(|s, _, window, cx| {
                s.activate_launcher_control(Control::Settings, window, cx);
                cx.stop_propagation();
            }));
        settings_entry.style().align_self = Some(gpui::AlignSelf::Stretch);
        menu = menu.child(self.launch_access.control(
            settings_entry,
            Control::Settings,
            t("shortcut.settings"),
            "",
            None,
            true,
        ));
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
    use super::{Agent, AgentLaunch, LaunchContext, LaunchMenu, LaunchTarget, Phase};
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
        for (agent, bypass, wire) in [
            (Agent::Claude, false, "claude\r"),
            (
                Agent::Claude,
                true,
                "claude --dangerously-skip-permissions\r",
            ),
            (Agent::Codex, false, "codex\r"),
            (Agent::Codex, true, "codex --yolo\r"),
            (Agent::Agy, false, "agy\r"),
            (Agent::Agy, true, "agy --dangerously-skip-permissions\r"),
            (Agent::Grok, false, "grok\r"),
            (Agent::Grok, true, "grok --always-approve\r"),
        ] {
            let mut config = AgentLaunch::defaults(agent);
            config.bypass_checked = bypass;
            let command = || config.command(agent, false);
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

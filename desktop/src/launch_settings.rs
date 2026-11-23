use super::*;
use dgl_core::agent::Agent;
use dgl_core::launcher::{LAUNCH_AGENTS, LaunchOption, OptionKind, single_line};

#[derive(Clone)]
pub(super) enum Editor {
    Bypass {
        agent: Agent,
        value: Entity<TextInput>,
    },
    Option {
        agent: Agent,
        index: Option<usize>,
        kind: OptionKind,
        name: Entity<TextInput>,
        description: Entity<TextInput>,
        value: Entity<TextInput>,
    },
}
impl Editor {
    pub fn inputs(&self) -> Vec<Entity<TextInput>> {
        match self {
            Self::Bypass { value, .. } => vec![value.clone()],
            Self::Option {
                name,
                description,
                value,
                ..
            } => vec![name.clone(), description.clone(), value.clone()],
        }
    }
    fn option(&self, cx: &App) -> Option<LaunchOption> {
        let Self::Option {
            name,
            description,
            kind,
            value,
            ..
        } = self
        else {
            return None;
        };
        Some(LaunchOption {
            name: name.read(cx).content.to_string(),
            description: description.read(cx).content.to_string(),
            value: value.read(cx).content.to_string(),
            kind: *kind,
            checked: false,
        })
    }
    pub fn valid(&self, cx: &App) -> bool {
        match self {
            Self::Bypass { value, .. } => single_line(&value.read(cx).content),
            Self::Option { .. } => self.option(cx).is_some_and(|o| o.valid()),
        }
    }
}

impl Desktop {
    fn launch_field(&self, text: &str, cx: &mut Context<Self>) -> Entity<TextInput> {
        let input = cx.new(|cx| TextInput::strict_single_line(text, cx));
        cx.observe(&input, |_, _, cx| cx.notify()).detach();
        input
    }
    fn open_launch_option(&mut self, agent: Agent, index: Option<usize>, cx: &mut Context<Self>) {
        let config = self.agent_launch(agent);
        let option = index.and_then(|i| config.options.get(i));
        let edit = Editor::Option {
            agent,
            index,
            kind: option.map_or(OptionKind::Argument, |o| o.kind),
            name: self.launch_field(option.map_or("", |o| &o.name), cx),
            description: self.launch_field(option.map_or("", |o| &o.description), cx),
            value: self.launch_field(option.map_or("", |o| &o.value), cx),
        };
        self.dialog = Some(Dialog::LaunchEditor(edit));
    }
    pub(super) fn launch_settings(&self, cx: &mut Context<Self>) -> Div {
        let mut list = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child(t("launcher.settings")));
        for agent in LAUNCH_AGENTS {
            let config = self.agent_launch(agent);
            let mut section = div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .border_1()
                .border_color(rgb(self.palette.line))
                .child(div().text_lg().child(agent.launch_label()))
                .child(div().child(t("launcher.bypass_arguments")))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().min_w_0().child(config.bypass.clone()))
                        .child(self.button(
                            format!("launch-bypass-edit-{agent:?}"),
                            t("common.edit"),
                            cx,
                            move |s, _, cx| {
                                let value = s.launch_field(&s.agent_launch(agent).bypass, cx);
                                s.dialog =
                                    Some(Dialog::LaunchEditor(Editor::Bypass { agent, value }));
                            },
                        )),
                );
            for (index, opt) in config.options.iter().enumerate() {
                section = section.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().min_w_0().truncate().child(opt.name.clone()))
                        .child(self.button(
                            format!("launch-option-edit-{agent:?}-{index}"),
                            t("common.edit"),
                            cx,
                            move |s, _, cx| {
                                s.open_launch_option(agent, Some(index), cx);
                            },
                        ))
                        .child(self.button(
                            format!("launch-option-delete-{agent:?}-{index}"),
                            t("common.delete"),
                            cx,
                            move |s, _, _| {
                                s.update_agent_launch(agent, |config| {
                                    config.options.remove(index);
                                });
                            },
                        )),
                );
            }
            list = list.child(section.child(self.button(
                format!("launch-option-add-{agent:?}"),
                t("launcher.add_option"),
                cx,
                move |s, _, cx| s.open_launch_option(agent, None, cx),
            )));
        }
        list
    }
    pub(super) fn launch_editor(&self, edit: &Editor, cx: &mut Context<Self>) -> Div {
        let agent = match edit {
            Editor::Bypass { agent, .. } | Editor::Option { agent, .. } => *agent,
        };
        let body = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_xl().child(agent.launch_label()));
        match edit {
            Editor::Bypass { value, .. } => body
                .child(t("launcher.bypass_arguments"))
                .child(value.clone()),
            Editor::Option {
                name,
                description,
                value,
                kind,
                ..
            } => {
                let mut types = div().flex().gap_2();
                for (choice, key) in [
                    (OptionKind::Environment, "launcher.environment"),
                    (OptionKind::Argument, "launcher.argument"),
                ] {
                    types = types.child(self.button(
                        format!("launch-kind-{choice:?}"),
                        (
                            if *kind == choice {
                                UiIcon::RadioSelected
                            } else {
                                UiIcon::Radio
                            },
                            t(key),
                        ),
                        cx,
                        move |s, _, _| {
                            if let Some(Dialog::LaunchEditor(Editor::Option { kind, .. })) =
                                &mut s.dialog
                            {
                                *kind = choice;
                            }
                        },
                    ));
                }
                body.child(t("common.name"))
                    .child(name.clone())
                    .child(t("launcher.description"))
                    .child(description.clone())
                    .child(types)
                    .child(t(if *kind == OptionKind::Environment {
                        "launcher.env_value"
                    } else {
                        "launcher.argument_value"
                    }))
                    .child(value.clone())
            }
        }
    }
    pub(super) fn save_launch_editor(&mut self, cx: &mut Context<Self>) {
        let Some(Dialog::LaunchEditor(edit)) = self.dialog.clone() else {
            return;
        };
        if !edit.valid(cx) {
            return;
        }
        let saved = match &edit {
            Editor::Bypass { agent, value } => self.update_agent_launch(*agent, |config| {
                config.bypass = value.read(cx).content.to_string()
            }),
            Editor::Option { agent, index, .. } => {
                let mut option = edit.option(cx).unwrap();
                self.update_agent_launch(*agent, |config| {
                    if let Some(index) = index {
                        let old = &mut config.options[*index];
                        option.checked = old.checked;
                        *old = option;
                    } else {
                        config.options.push(option);
                    }
                })
            }
        };
        if saved {
            self.dialog = Some(Dialog::LaunchSettings);
        }
        cx.notify();
    }
}

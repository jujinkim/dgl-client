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
    /// The quick launch settings page: one group per agent.
    pub(super) fn launch_settings(&self, cx: &mut Context<Self>) -> Div {
        use settings_ui::ActionKind;
        let mut list = div()
            .flex()
            .flex_col()
            .gap(px(settings_ui::PAGE_GAP))
            .child(
                self.settings_note(t("launcher.plaintext_warning"))
                    .id("launch-plaintext-warning")
                    .debug_selector(|| "launch-plaintext-warning".into())
                    .text_color(rgb(self.palette.danger)),
            );
        for agent in LAUNCH_AGENTS {
            let config = self.agent_launch(agent);
            let bypass_id = format!("launch-bypass-edit-{agent:?}");
            let entry = bypass_id.clone();
            let mut rows = vec![
                self.settings_row_with(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(px(settings_ui::BODY))
                                .child(t("launcher.bypass_arguments")),
                        )
                        .when(!config.bypass.is_empty(), |d| {
                            d.child(
                                self.settings_note(config.bypass.clone())
                                    .font_family(platform::TERMINAL_FONT),
                            )
                        }),
                    self.settings_action(
                        bypass_id,
                        t("common.edit"),
                        ActionKind::Normal,
                        cx,
                        move |s, _, cx| {
                            s.remember_settings_entry(entry.clone());
                            let value = s.launch_field(&s.agent_launch(agent).bypass, cx);
                            s.dialog = Some(Dialog::LaunchEditor(Editor::Bypass { agent, value }));
                        },
                    ),
                )
                .into_any_element(),
            ];
            for (index, opt) in config.options.iter().enumerate() {
                let edit_id = format!("launch-option-edit-{agent:?}-{index}");
                let entry = edit_id.clone();
                rows.push(
                    self.settings_row(
                        opt.name.clone(),
                        (!opt.description.is_empty()).then(|| opt.description.clone().into()),
                        div()
                            .flex()
                            .gap_2()
                            .child(self.settings_action(
                                edit_id,
                                t("common.edit"),
                                ActionKind::Normal,
                                cx,
                                move |s, _, cx| {
                                    s.remember_settings_entry(entry.clone());
                                    s.open_launch_option(agent, Some(index), cx);
                                },
                            ))
                            .child(self.settings_action(
                                format!("launch-option-delete-{agent:?}-{index}"),
                                t("common.delete"),
                                ActionKind::Danger,
                                cx,
                                move |s, _, _| {
                                    s.update_agent_launch(agent, |config| {
                                        config.options.remove(index);
                                    });
                                },
                            )),
                    )
                    .into_any_element(),
                );
            }
            let add_id = format!("launch-option-add-{agent:?}");
            let entry = add_id.clone();
            rows.push(
                div()
                    .flex()
                    .px(px(settings_ui::ROW_PX))
                    .py(px(settings_ui::ROW_PY))
                    .child(self.settings_action(
                        add_id,
                        t("launcher.add_option"),
                        ActionKind::Normal,
                        cx,
                        move |s, _, cx| {
                            s.remember_settings_entry(entry.clone());
                            s.open_launch_option(agent, None, cx);
                        },
                    ))
                    .into_any_element(),
            );
            // Definitions sync; check states stay on this device.
            list = list.child(self.settings_group_marked(
                agent.executable(),
                Some(agent.launch_label().into()),
                None,
                rows,
                true,
            ));
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
                    types = types.child(self.settings_choice(
                        format!("launch-kind-{choice:?}"),
                        t(key),
                        *kind == choice,
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
            self.return_from_launch_editor();
        }
        cx.notify();
    }
}

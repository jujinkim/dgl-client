//! Quick launch options. Arguments are shell fragments, environment values are literals.
//! Definitions sync through `sync::SharedLaunch`; check states stay on each device.
use crate::agent::Agent;
use serde::{Deserialize, Serialize};

pub const LAUNCH_AGENTS: [Agent; 4] = [Agent::Claude, Agent::Codex, Agent::Agy, Agent::Grok];

impl Agent {
    pub fn executable(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Agy => "agy",
            Self::Grok => "grok",
        }
    }

    pub fn launch_label(self) -> &'static str {
        if self == Self::Claude {
            "Claude Code"
        } else {
            self.label()
        }
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptionKind {
    Environment,
    #[default]
    Argument,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchOption {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub kind: OptionKind,
    pub value: String,
    #[serde(default)]
    pub checked: bool,
}

pub fn single_line(text: &str) -> bool {
    !text
        .chars()
        .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
}

fn assignment(value: &str) -> Option<(&str, &str)> {
    let (key, value) = value.split_once('=')?;
    let mut chars = key.bytes();
    let first = chars.next()?;
    ((first.is_ascii_alphabetic() || first == b'_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == b'_'))
    .then_some((key, value))
}

impl LaunchOption {
    pub fn valid(&self) -> bool {
        !self.name.trim().is_empty()
            && single_line(&self.name)
            && single_line(&self.description)
            && single_line(&self.value)
            && match self.kind {
                OptionKind::Environment => assignment(&self.value).is_some(),
                OptionKind::Argument => !self.value.trim().is_empty(),
            }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentLaunch {
    pub bypass: String,
    #[serde(default, rename = "bypassChecked")]
    pub bypass_checked: bool,
    pub options: Vec<LaunchOption>,
}

impl AgentLaunch {
    pub fn defaults(agent: Agent) -> Self {
        let options = if agent == Agent::Claude {
            vec![LaunchOption {
                name: "Code Mode".into(),
                description: crate::i18n::t("launcher.code_mode_description").into(),
                kind: OptionKind::Environment,
                value: "CLAUDE_CODE_EXPERIMENTAL_CODE_MODE=1".into(),
                checked: false,
            }]
        } else {
            Vec::new()
        };
        let bypass = match agent {
            Agent::Claude | Agent::Agy => "--dangerously-skip-permissions",
            Agent::Codex => "--yolo",
            Agent::Grok => "--always-approve",
        };
        Self {
            bypass: bypass.into(),
            bypass_checked: false,
            options,
        }
    }

    pub fn command(&self, agent: Agent, powershell: bool) -> Option<Vec<u8>> {
        let mut env = Vec::new();
        let mut words = vec![agent.executable()];
        if self.bypass_checked && !self.bypass.is_empty() {
            if !single_line(&self.bypass) {
                return None;
            }
            words.push(&self.bypass);
        }
        for option in self.options.iter().filter(|o| o.checked) {
            if !option.valid() {
                return None;
            }
            match option.kind {
                OptionKind::Environment => env.push(assignment(&option.value)?),
                OptionKind::Argument => words.push(&option.value),
            }
        }
        let mut line = String::new();
        if powershell && !env.is_empty() {
            // PowerShell environment edits affect the parent process even inside a script block.
            // Snapshot before any assignment, including when the same key occurs twice.
            line.push_str("& { $dglPrevious = @(");
            for (i, (key, _)) in env.iter().enumerate() {
                if i != 0 {
                    line.push_str(", ");
                }
                line.push_str(&format!("$env:{key}"));
            }
            line.push_str("); try { ");
            for (key, value) in &env {
                line.push_str(&format!("$env:{key} = '"));
                for ch in value.chars() {
                    // PowerShell also recognizes typographic single quotes as delimiters.
                    if matches!(ch, '\'' | '\u{2018}'..='\u{201b}') {
                        line.push('\'');
                    }
                    line.push(ch);
                }
                line.push_str("'; ");
            }
            line.push_str(&words.join(" "));
            line.push_str(" } finally { ");
            for (i, (key, _)) in env.iter().enumerate() {
                line.push_str(&format!("$env:{key} = $dglPrevious[{i}]; "));
            }
            line.push_str("} }");
        } else {
            for (key, value) in env {
                line.push_str(key);
                line.push('=');
                if value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_.-/".contains(&b))
                {
                    line.push_str(value);
                } else {
                    line.push_str(&format!("'{}'", value.replace('\'', "'\\''")));
                }
                line.push(' ');
            }
            line.push_str(&words.join(" "));
        }
        line.push('\r');
        Some(line.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LocalState;

    #[test]
    fn compose_selected_options_and_restore_powershell_environment() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/launcher-options.json")).unwrap();
        let mut config: AgentLaunch = serde_json::from_value(fixture.clone()).unwrap();
        assert!(!config.bypass_checked); // Preferences saved before Bypass became a checkbox.
        for (checked, key) in [(false, "normal"), (true, "withBypass")] {
            config.bypass_checked = checked;
            assert_eq!(
                config.command(Agent::Claude, false).unwrap(),
                fixture[key].as_str().unwrap().as_bytes()
            );
        }
        let line = String::from_utf8(config.command(Agent::Claude, true).unwrap()).unwrap();
        assert_eq!(
            line,
            "& { $dglPrevious = @($env:MODE, $env:LABEL); try { $env:MODE = '1'; $env:LABEL = 'a b''c=$HOME'; claude --custom  \"two words\" --model  'my model'   --resume } finally { $env:MODE = $dglPrevious[0]; $env:LABEL = $dglPrevious[1]; } }\r"
        );
    }

    #[test]
    fn old_settings_start_unchecked_and_explicit_deletion_survives_restart() {
        let mut state = LocalState::decode(b"{}").unwrap();
        let config = state
            .preferences
            .agent_launch
            .entry(Agent::Claude)
            .or_insert_with(|| AgentLaunch::defaults(Agent::Claude));
        assert!(!config.options[0].checked);
        assert!(!config.bypass_checked);
        config.options[0].checked = true;
        assert_eq!(
            config.command(Agent::Claude, false).unwrap(),
            b"CLAUDE_CODE_EXPERIMENTAL_CODE_MODE=1 claude\r"
        );
        config.bypass_checked = true;
        let mut reloaded = LocalState::decode(&serde_json::to_vec(&state).unwrap()).unwrap();
        let config = reloaded
            .preferences
            .agent_launch
            .get_mut(&Agent::Claude)
            .unwrap();
        assert!(config.bypass_checked);
        assert_eq!(
            config.command(Agent::Claude, false).unwrap(),
            b"CLAUDE_CODE_EXPERIMENTAL_CODE_MODE=1 claude --dangerously-skip-permissions\r"
        );
        config.options.clear();
        assert!(
            LocalState::decode(&serde_json::to_vec(&reloaded).unwrap())
                .unwrap()
                .preferences
                .agent_launch[&Agent::Claude]
                .options
                .is_empty()
        );
    }

    #[test]
    fn reject_controls_and_invalid_assignments_even_in_saved_values() {
        let mut config = AgentLaunch::defaults(Agent::Claude);
        config.options[0].checked = true;
        for raw in [
            "=value",
            "9BAD=1",
            "BAD NAME=1",
            "NOVALUE",
            "A=a\rb",
            "A=\n",
            "A=\t",
            "A=\x1b",
            "A=\u{85}",
            "A=\u{2028}",
        ] {
            config.options[0].value = raw.into();
            assert!(config.command(Agent::Claude, false).is_none());
        }
        config.options[0].value = "EMPTY=".into();
        assert_eq!(
            config.command(Agent::Claude, false).unwrap(),
            b"EMPTY= claude\r"
        );
        config.bypass = "--unsafe\r".into();
        config.bypass_checked = true;
        assert!(config.command(Agent::Claude, true).is_none());
    }
}

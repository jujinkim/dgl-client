//! psmux version contract is independent of tmux's supported releases.
pub fn supported(version: &str) -> bool {
    let Some(version) = version.trim().strip_prefix("psmux ") else {
        return false;
    };
    let parts: Vec<_> = version.trim_start_matches('v').split('.').collect();
    parts.len() == 3
        && parts[0] == "3"
        && parts[1].parse::<u32>().is_ok_and(|v| v >= 4)
        && parts[2].parse::<u32>().is_ok()
}

use super::Local;
use crate::{
    Error, Result,
    command::{Command, Operation},
    protocol::Event,
};
/// Window and pane IDs are local to one psmux server. Session IDs are machine-wide.
pub fn scoped(id: &str, session: u64) -> Result<String> {
    let prefix = id.chars().next().ok_or(Error::Protocol)?;
    if !matches!(prefix, '@' | '%') || session > u32::MAX as u64 {
        return Err(Error::Protocol);
    }
    let native: u32 = id[1..].parse().map_err(|_| Error::Protocol)?;
    Ok(format!("{prefix}{}", (session << 32) | native as u64))
}
pub fn native(id: &str) -> Result<(u64, String)> {
    let prefix = id.chars().next().ok_or(Error::InvalidArgument)?;
    if !matches!(prefix, '@' | '%') {
        return Err(Error::InvalidArgument);
    }
    let n: u64 = id[1..].parse().map_err(|_| Error::InvalidArgument)?;
    Ok((n >> 32, format!("{prefix}{}", n & u32::MAX as u64)))
}
pub(crate) struct Adapter {
    pub local: Local,
    pub session: u64,
    lists: [Vec<Vec<u8>>; 3],
}
impl Adapter {
    pub fn new(local: Local, session: &str) -> Result<Self> {
        let session = session
            .strip_prefix('$')
            .ok_or(Error::InvalidArgument)?
            .parse()
            .map_err(|_| Error::InvalidArgument)?;
        Ok(Self {
            local,
            session,
            lists: Default::default(),
        })
    }
    pub fn event(&self, event: Event) -> Result<Event> {
        Ok(match event {
            Event::Output { pane, bytes } => Event::Output {
                pane: scoped(&pane, self.session)?,
                bytes,
            },
            Event::Notification { name, arguments } if name == "%pause" || name == "%continue" => {
                Event::Notification {
                    name,
                    arguments: scoped(arguments.trim(), self.session)?,
                }
            }
            other => other,
        })
    }
    pub fn operation(&self, operation: &Operation) -> Result<Command> {
        let mut value = serde_json::to_value(operation).map_err(|_| Error::InvalidArgument)?;
        if let Some(target) = value.get_mut("target") {
            if let Some(id) = target.as_str().filter(|s| s.starts_with(['@', '%'])) {
                let (session, id) = native(id)?;
                if session != self.session {
                    return Err(Error::StaleGeneration);
                }
                *target = id.into();
            }
        }
        let operation = serde_json::from_value(value).map_err(|_| Error::InvalidArgument)?;
        let mut command = Command::new(&operation)?;
        // ConPTY exposes a primary screen capture. -C/-P/-a are tmux-only capture semantics.
        if let Operation::Capture { target } = &operation {
            command.wire = format!(
                "capture-pane -p -e -N -t {}",
                crate::command::target(target, '%')?
            );
        }
        Ok(command)
    }
    pub async fn listing(&mut self, op: &Operation) -> Result<Option<Vec<Vec<u8>>>> {
        let index = match op {
            Operation::Sessions => 0,
            Operation::Windows => 1,
            Operation::Panes => 2,
            _ => return Ok(None),
        };
        if index == 0 {
            let bytes = inventory(&self.local).await?;
            self.lists = Default::default();
            for line in bytes.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
                let index = match line.first() {
                    Some(b's') => 0,
                    Some(b'w') => 1,
                    Some(b'p') => 2,
                    _ => return Err(Error::Protocol),
                };
                self.lists[index].push(line[2..].to_vec());
            }
        }
        Ok(Some(self.lists[index].clone()))
    }
}
pub(crate) async fn inventory(local: &Local) -> Result<Vec<u8>> {
    let (code, sessions) = local
        .execute_args(&[
            "list-sessions".into(),
            "-F".into(),
            "s|#{session_id}|#{q:session_name}|#{session_attached}".into(),
        ])
        .await?;
    if code != 0 {
        if sessions.is_empty() {
            return Ok(Vec::new());
        }
        return Err(Error::Protocol);
    }
    let mut out = sessions.clone();
    if !out.is_empty() && !out.ends_with(b"\n") {
        out.push(b'\n');
    }
    for row in sessions.split(|b| *b == b'\n').filter(|s| !s.is_empty()) {
        let text = std::str::from_utf8(row).map_err(|_| Error::Protocol)?;
        let id = text.split('|').nth(1).ok_or(Error::Protocol)?;
        crate::command::target(id, '$')?;
        let session: u64 = id[1..].parse().map_err(|_| Error::Protocol)?;
        for (kind, op) in [(b'w', Operation::Windows), (b'p', Operation::Panes)] {
            let mut args = Command::new(&op)?.argv()?;
            // -a means all panes on THIS server; list-windows already covers the session.
            if kind == b'w' {
                args.retain(|s| s != "-a");
            }
            args.extend(["-t".into(), id.into()]);
            let (code, bytes) = local.execute_args(&args).await?;
            if code != 0 {
                continue;
            } // A session can disappear during the inventory.
            for row in bytes.split(|b| *b == b'\n').filter(|s| !s.is_empty()) {
                let mut fields = crate::model::split_record(row, 10)?;
                if kind == b'w' {
                    fields[0] = id.into();
                    fields[1] = scoped(&fields[1], session)?;
                } else {
                    fields[0] = scoped(&fields[0], session)?;
                    fields[1] = scoped(&fields[1], session)?;
                }
                out.extend_from_slice(&[kind, b'|']);
                out.extend_from_slice(fields.join("|").as_bytes());
                out.push(b'\n');
            }
        }
    }
    if out.len() > 4 * 1024 * 1024 {
        return Err(Error::Protocol);
    }
    Ok(out)
}
/// psmux ignores capture-pane -C; encode its literal rows for the common snapshot decoder.
pub(crate) fn capture(rows: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    rows.into_iter()
        .map(|row| {
            row.into_iter()
                .flat_map(|b| {
                    if b == b'\\' || !(0x20..=0x7e).contains(&b) {
                        format!("\\{b:03o}").into_bytes()
                    } else {
                        vec![b]
                    }
                })
                .collect()
        })
        .collect()
}
/// -CC adds a DCS opener, potentially split across any read boundary.
#[derive(Default)]
pub(crate) struct Framing {
    prefix: Vec<u8>,
    started: bool,
}
impl Framing {
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<u8>> {
        if self.started {
            return Ok(bytes.to_vec());
        }
        self.prefix.extend_from_slice(bytes);
        let expected = b"\x1bP1000p";
        let n = self.prefix.len().min(expected.len());
        if self.prefix[..n] != expected[..n] {
            return Err(Error::Protocol);
        }
        if self.prefix.len() < expected.len() {
            return Ok(Vec::new());
        }
        self.started = true;
        Ok(self.prefix.split_off(expected.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_is_not_a_tmux_version_and_rejects_unverified_protocols() {
        assert!(supported("psmux 3.4.0"));
        for value in [
            "tmux 3.7c",
            "psmux 3.3.9",
            "psmux 4.0.0",
            "psmux 3.4",
            "psmux 3.4.x",
        ] {
            assert!(!supported(value));
        }
    }
    #[test]
    fn cc_startup_can_split_at_every_byte_and_pause_output_keeps_session_scope() {
        let wire = b"\x1bP1000p%begin 123 1 0\n%end 123 1 0\n%extended-output %0 7 : hello\\134world\\015\\012\n%pause %0\n";
        for split in 0..=wire.len() {
            let mut framing = Framing::default();
            let mut parser = crate::protocol::Parser::default();
            let mut events = Vec::new();
            for bytes in [&wire[..split], &wire[split..]] {
                events.extend(parser.feed(&framing.feed(bytes).unwrap()).unwrap());
            }
            assert!(matches!(events[0], Event::Response { flags: 0, .. }));
            let adapter = Adapter::new(
                Local {
                    config: Default::default(),
                    version: "psmux 3.4.0".into(),
                },
                "$8",
            )
            .unwrap();
            assert!(
                matches!(adapter.event(events[1].clone()).unwrap(), Event::Output { pane, bytes } if pane == scoped("%0", 8).unwrap() && bytes == b"hello\\world\r\n")
            );
            assert!(
                matches!(adapter.event(events[2].clone()).unwrap(), Event::Notification { arguments, .. } if arguments == scoped("%0", 8).unwrap())
            );
        }
    }
    #[test]
    fn same_native_ids_in_different_servers_cannot_target_each_other() {
        let a = scoped("%0", 1).unwrap();
        let b = scoped("%0", 2).unwrap();
        assert_ne!(a, b);
        assert_eq!(native(&b).unwrap(), (2, "%0".into()));
        let adapter = Adapter::new(
            Local {
                config: Default::default(),
                version: "psmux 3.4.0".into(),
            },
            "$1",
        )
        .unwrap();
        assert_eq!(
            adapter
                .operation(&Operation::SelectPane { target: b })
                .unwrap_err(),
            Error::StaleGeneration
        );
        assert_eq!(
            adapter
                .operation(&Operation::SelectPane { target: a })
                .unwrap()
                .wire,
            "select-pane -t '%0'"
        );
        assert_eq!(
            native(&scoped("@11", 2).unwrap()).unwrap(),
            (2, "@11".into())
        );
        let rows = capture(vec![b"a\\n\x1b[0m".to_vec()]);
        assert_eq!(
            crate::protocol::decode(&rows[0], true).unwrap(),
            b"a\\n\x1b[0m"
        );
    }
}

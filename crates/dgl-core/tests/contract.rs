use dgl_core::{
    command::{Command, Operation},
    input::{self, MacroStep, TerminalKey},
    model,
    protocol::{self, Event, Parser},
};
use serde_json::{Value, json};
fn fixtures() -> Value {
    serde_json::from_str(include_str!("../../../fixtures/migration/contract-v1.json")).unwrap()
}
fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap()
}
fn hex(b: &[u8]) -> String {
    b.iter().map(|v| format!("{v:02x}")).collect()
}
fn bytes(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
fn event(e: Event) -> Value {
    match e {
        Event::Response {
            timestamp,
            number,
            flags,
            lines,
            failed,
        } => {
            json!({"type":"response","timestamp":timestamp,"number":number,"flags":flags,"linesHex":lines.iter().map(|v|hex(v)).collect::<Vec<_>>(),"failed":failed})
        }
        Event::Output { pane, bytes } => {
            json!({"type":"output","pane":pane,"bytesHex":hex(&bytes)})
        }
        Event::Notification { name, arguments } => {
            json!({"type":"notification","name":name,"arguments":arguments})
        }
    }
}
#[test]
fn contract_125_cases() {
    let f = fixtures();
    let mut count = 0;
    for c in f["protocol"].as_array().unwrap() {
        count += 1;
        let wire = text(c, "wire").as_bytes();
        for split in 0..=wire.len() + 1 {
            let mut parser = Parser::new(
                c["maxLine"].as_u64().unwrap_or(1 << 20) as usize,
                c["maxResponse"].as_u64().unwrap_or(4 << 20) as usize,
            );
            let chunks: Vec<&[u8]> = if split > wire.len() {
                wire.chunks(1).collect()
            } else {
                vec![&wire[..split], &wire[split..]]
            };
            let result = (|| {
                let mut out = vec![];
                for chunk in chunks {
                    out.extend(parser.feed(chunk)?);
                }
                parser.finish()?;
                Ok::<_, dgl_core::Error>(out)
            })();
            if c["error"] == true {
                assert!(result.is_err(), "{} {split}", c["id"]);
                assert!(parser.feed(b"").is_err());
            } else {
                assert_eq!(
                    json!(result.unwrap().into_iter().map(event).collect::<Vec<_>>()),
                    c["events"],
                    "{} {split}",
                    c["id"]
                );
            }
        }
    }
    for c in f["escapes"].as_array().unwrap() {
        count += 1;
        let result = protocol::decode(text(c, "source").as_bytes(), c["mode"] == "capture");
        if c["error"] == true {
            assert!(result.is_err(), "{}", c["id"])
        } else {
            assert_eq!(hex(&result.unwrap()), text(c, "bytesHex"), "{}", c["id"]);
        }
    }
    for c in f["commands"].as_array().unwrap() {
        count += 1;
        let mut v = c.clone();
        if v["op"] == "send" {
            v["bytes"] = json!(bytes(text(c, "bytesHex")));
        }
        let result = serde_json::from_value::<Operation>(v)
            .map_err(|_| dgl_core::Error::InvalidArgument)
            .and_then(|o| Command::new(&o));
        if c["error"] == true {
            assert!(result.is_err(), "{}", c["id"])
        } else {
            let command = result.unwrap();
            assert_eq!(command.wire, text(c, "wire"), "{}", c["id"]);
            assert_eq!(command.shell_wire, text(c, "shellWire"), "{}", c["id"]);
            assert!(
                command
                    .shell_with_executable("/test path'/tmux")
                    .unwrap()
                    .starts_with("'/test path'\\''/tmux' ")
            );
        }
    }
    for (key, v) in f["keys"].as_object().unwrap() {
        count += 1;
        let k: TerminalKey = serde_json::from_value(json!(key)).unwrap();
        assert_eq!(hex(k.bytes()), v.as_str().unwrap(), "{key}");
    }
    for c in f["input"].as_array().unwrap() {
        count += 1;
        let steps: Vec<MacroStep> = c["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                let mut s = v.clone();
                if s["type"] == "text" {
                    s["text"] =
                        json!(text(v, "text").repeat(v["repeat"].as_u64().unwrap_or(1) as usize));
                }
                serde_json::from_value(s).unwrap()
            })
            .collect();
        let result = input::encode(&steps);
        if c["error"] == true {
            assert!(result.is_err(), "{}", c["id"])
        } else {
            let b = result.unwrap();
            if let Some(expected) = c["bytesHex"].as_str() {
                assert_eq!(hex(&b), expected, "{}", c["id"])
            } else {
                assert_eq!(b.len(), c["expectedSize"].as_u64().unwrap() as usize);
                let pattern = bytes(text(c, "expectedPatternHex"));
                for (i, x) in b.iter().enumerate() {
                    assert_eq!(*x, pattern[i % pattern.len()]);
                }
            }
        }
    }
    for c in f["settings"].as_array().unwrap() {
        count += 1;
        let result = model::LocalState::decode(c["source"].to_string().as_bytes());
        if c["error"] == true {
            assert!(result.is_err(), "{}", c["id"])
        } else {
            assert_eq!(
                serde_json::to_value(result.unwrap()).unwrap(),
                c["expected"],
                "{}",
                c["id"]
            );
        }
    }
    for c in f["state"].as_array().unwrap() {
        count += 1;
        let rows: Vec<_> = c["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().as_bytes().to_vec())
            .collect();
        let result = match text(c, "kind") {
            "sessions" => model::sessions(&rows).map(|x| json!(x)),
            "windows" => model::windows(&rows).map(|x| json!(x)),
            "panes" => model::panes(&rows).map(|x| json!(x)),
            _ => panic!(),
        };
        if c["error"] == true {
            assert!(result.is_err(), "{}", c["id"])
        } else {
            assert_eq!(result.unwrap(), c["expected"], "{}", c["id"]);
        }
    }
    assert_eq!(count, 125);
}

#[test]
fn settings_atomic_round_trip_and_invalid_original_preserved() {
    let dir = std::env::temp_dir().join(format!("dgl-rust-contract-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    for case in fixtures()["settings"].as_array().unwrap() {
        let bytes = case["source"].to_string().into_bytes();
        std::fs::write(&path, &bytes).unwrap();
        let result = model::LocalState::read(&path);
        if case["error"] == true {
            assert!(result.is_err());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        } else {
            let state = result.unwrap();
            state.save(&path).unwrap();
            assert_eq!(model::LocalState::read(&path).unwrap(), state);
        }
    }
    let mut state = model::LocalState::default();
    assert!(!state.preferences.smooth_terminal_transitions);
    state.preferences.smooth_terminal_transitions = true;
    state.save(&path).unwrap();
    assert!(model::LocalState::read(&path).unwrap().preferences.smooth_terminal_transitions);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn directional_focus_encoding_rejects_invalid_targets_and_directions() {
    for direction in ['L', 'R', 'U', 'D'] {
        let op: Operation = serde_json::from_value(
            json!({"op":"focusPane", "target":"%12", "direction":direction.to_string()}),
        )
        .unwrap();
        assert_eq!(
            Command::new(&op).unwrap().wire,
            format!("select-pane -t '%12' -{direction}")
        );
        assert_eq!(
            Command::new(&Operation::ResizePane {
                target: "%12".into(),
                direction
            })
            .unwrap()
            .wire,
            format!("resize-pane -t '%12' -{direction} 3")
        );
    }
    for (target, direction) in [("%1; kill-server", 'L'), ("@1", 'R'), ("%1", 'X')] {
        assert!(
            Command::new(&Operation::FocusPane {
                target: target.into(),
                direction
            })
            .is_err()
        );
    }
}

#![cfg(unix)]
use dgl_core::{
    command::Operation,
    connection::{Action, Connection, Phase},
    flow::{OutputEvent, OutputKind},
    local::{LocalConfig, Multiplexer},
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
async fn until(c: &Connection, f: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !f() {
            assert!(
                !matches!(c.state().phase, Phase::Failed | Phase::Closed),
                "{:?}",
                c.state()
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{:?}", c.state()));
}
#[tokio::test]
async fn psmux_process_fixture_switches_servers_restores_capture_and_preserves_flow_output() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let executable = dir.path().join("psmux-fixture");
    std::fs::write(
        &executable,
        include_str!("../../../fixtures/psmux/control_client.py"),
    )
    .unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let state = dir.path().join("state.json");
    std::fs::write(
        &state,
        r#"[{"id":"$1","name":"first"},{"id":"$2","name":"second"}]"#,
    )
    .unwrap();
    let c = Connection::start_local(LocalConfig {
        executable,
        multiplexer: Multiplexer::Psmux,
        arguments: vec![state.to_str().unwrap().into()],
        last_session: None,
        last_pane: None,
    });
    let output = c.output();
    let subscription = output.subscribe().unwrap();
    let recorded: Arc<Mutex<Vec<OutputEvent>>> = Arc::default();
    let events = recorded.clone();
    let reader = tokio::spawn(async move {
        loop {
            while let Ok(Some(event)) = output.poll(subscription) {
                output.ack(subscription, event.sequence).unwrap();
                events.lock().unwrap().push(event);
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    });
    until(&c, || c.state().phase == Phase::Selecting).await;
    assert_eq!(c.state().remote.sessions.len(), 2);
    assert_ne!(c.state().remote.panes[0].id, c.state().remote.panes[1].id);
    c.action(Action::SelectSession { id: "$1".into() }).unwrap();
    until(&c, || c.state().phase == Phase::Ready).await;
    let old = c.state().input_target().unwrap();
    c.action(Action::SelectSession { id: "$2".into() }).unwrap();
    until(&c, || {
        c.state().phase == Phase::Ready && c.state().session.as_deref() == Some("$2")
    })
    .await;
    assert_ne!(old.pane_id, c.state().pane.as_deref().unwrap());
    assert!(c.input(old, b"STALE".to_vec()).is_err());
    let rename = c
        .action(Action::Manage {
            operation: Operation::RenameSession {
                target: "$2".into(),
                name: "renamed session".into(),
            },
        })
        .unwrap();
    until(&c, || {
        c.completions()
            .into_iter()
            .any(|r| r.request_id == rename && r.error.is_none())
    })
    .await;
    assert!(
        c.state()
            .remote
            .sessions
            .iter()
            .any(|s| s.id == "$2" && s.name == "renamed session")
    );
    let foreign = dgl_core::local::psmux::scoped("@0", 1).unwrap();
    let request = c
        .action(Action::Manage {
            operation: Operation::RenameWindow {
                target: foreign.clone(),
                name: "foreign window".into(),
            },
        })
        .unwrap();
    until(&c, || {
        c.completions()
            .into_iter()
            .any(|r| r.request_id == request && r.error.is_none())
    })
    .await;
    assert!(
        c.state()
            .remote
            .windows
            .iter()
            .any(|w| w.id == foreign && w.name == "foreign window")
    );
    // Resolve the renamed session's current name when attaching again; IDs remain stable.
    for session in ["$1", "$2"] {
        c.action(Action::SelectSession { id: session.into() })
            .unwrap();
        until(&c, || {
            c.state().phase == Phase::Ready && c.state().session.as_deref() == Some(session)
        })
        .await;
    }
    let input = c
        .input(c.state().input_target().unwrap(), b"hi\n".to_vec())
        .unwrap();
    until(&c, || {
        c.completions()
            .into_iter()
            .any(|r| r.request_id == input && r.error.is_none())
    })
    .await;
    until(&c, || {
        recorded.lock().unwrap().iter().any(|e| {
            e.kind == OutputKind::Live && e.bytes.windows(13).any(|b| b == b"AFTER_CAPTURE")
        })
    })
    .await;
    assert!(
        recorded
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.bytes.windows(15).any(|b| b == b"fixture\\literal"))
    );
    {
        let events = recorded.lock().unwrap();
        for (index, event) in events
            .iter()
            .enumerate()
            .filter(|(_, e)| e.kind == OutputKind::Live)
        {
            assert!(events[..index].iter().any(|prior| prior.kind == OutputKind::ResetEnd && prior.target == event.target));
        }
    }
    // Terminate only the fixture's owned control client. The actor must reattach without replay.
    let before = c.state().generation.connection;
    let previous = c.state().input_target().unwrap();
    let entries: Vec<serde_json::Value> = std::fs::read_to_string(state.with_extension("log"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let process = entries
        .iter()
        .rev()
        .find(|e| e["args"][0] == "-CC")
        .unwrap()["pid"]
        .as_u64()
        .unwrap();
    assert!(
        std::process::Command::new("/bin/kill")
            .args(["-TERM", &process.to_string()])
            .status()
            .unwrap()
            .success()
    );
    until(&c, || {
        c.state().phase == Phase::Ready && c.state().generation.connection > before
    })
    .await;
    assert_eq!(c.state().session.as_deref(), Some("$2"));
    assert!(c.input(previous, b"STALE".to_vec()).is_err());
    c.close();
    c.join().await;
    reader.abort();
    let log = std::fs::read_to_string(state.with_extension("log")).unwrap();
    assert!(!log.contains("STALE"));
    let entries: Vec<serde_json::Value> = log
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let send = entries
        .iter()
        .find(|e| e["args"][0] == "send-keys")
        .unwrap();
    assert_eq!(
        send["args"],
        serde_json::json!(["send-keys", "-H", "-t", "%0", "68", "69", "0a"])
    );
    assert!(
        entries
            .iter()
            .any(|e| e["args"][0] == "-CC" && e["session"] == "renamed session")
    );
    assert_eq!(
        entries
            .iter()
            .filter(|e| e["args"][0] == "send-keys")
            .count(),
        1
    );
    assert!(log.contains("refresh-client"));
}

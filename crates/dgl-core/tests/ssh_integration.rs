use dgl_core::{
    Error,
    command::Operation,
    connection::{Action, Connection, Phase},
    model::{AuthKind, Credential, HostProfile},
    ssh,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
fn fixture(key: &str) -> (HostProfile, Credential, String) {
    let host = HostProfile {
        id: "fixture".into(),
        alias: "fixture".into(),
        hostname: "127.0.0.1".into(),
        port: std::env::var("DGL_TEST_PORT").unwrap().parse().unwrap(),
        username: std::env::var("DGL_TEST_USER").unwrap(),
        auth: AuthKind::PrivateKey,
        credential_ref: "fixture".into(),
        favorite: false,
        last_connected: 0,
        last_session: None,
        last_pane: None,
    };
    let c = Credential {
        private_key: std::fs::read_to_string(std::env::var(key).unwrap()).unwrap(),
        passphrase: if key == "DGL_TEST_ENCRYPTED_FILE" {
            std::env::var("DGL_TEST_PASSPHRASE").unwrap()
        } else {
            String::new()
        },
        password: String::new(),
    };
    (host, c, std::env::var("DGL_TEST_FINGERPRINT").unwrap())
}
async fn phase(c: &Connection, expected: Phase) {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let s = c.state();
            if s.phase == expected {
                return;
            }
            assert!(
                !matches!(s.phase, Phase::Failed | Phase::Closed),
                "{:?}",
                s.error
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
async fn completion(c: &Connection, id: u64) {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            for result in c.completions() {
                if result.request_id == id {
                    assert_eq!(result.error, None, "request {id}");
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "requires scripts/integration.py --agent-home; simulated hook producer, real SSH/tmux"]
async fn agent_hidden_panes_baseline_events_navigation_and_process_exit() {
    for (service, executable) in [
        (dgl_core::agent::Agent::Codex, "codex"),
        (dgl_core::agent::Agent::Claude, "claude"),
        (dgl_core::agent::Agent::Agy, "agy"),
        (dgl_core::agent::Agent::Grok, "grok"),
    ] {
        agent_hidden_pane_case(service, executable).await;
    }
}

async fn agent_hidden_pane_case(service: dgl_core::agent::Agent, executable: &str) {
    use dgl_core::agent::{self, Kind};
    let root = std::path::PathBuf::from(std::env::var("DGL_TEST_ROOT").unwrap());
    // Refuse to write an observer database into the real home in ordinary SSH tests.
    if !root.join("home").exists() {
        return;
    }
    let helper = root.join("observer.py");
    std::fs::write(&helper, agent::HELPER).unwrap();
    let trigger = root.join("emit-stop");
    let wrapper = std::env::var("DGL_TEST_TMUX_WRAPPER").unwrap();
    let payload = match service {
        agent::Agent::Agy => {
            serde_json::json!({"conversationId":"root", "transcriptPath":"/fixture/.gemini/antigravity-cli/brain/root/logs/transcript.jsonl", "terminationReason":"model_stop", "fullyIdle":true})
        }
        agent::Agent::Grok => serde_json::json!({"sessionId":"root", "hookEventName":"stop"}),
        _ => serde_json::json!({"session_id":"fixture", "hook_event_name":"Stop"}),
    };
    let command = format!(
        "printf '%s' {} | python3 {} record --agent {service:?} --event Stop --tmux {}",
        dgl_core::command::quote(&payload.to_string()).unwrap(),
        dgl_core::command::quote(helper.to_str().unwrap()).unwrap(),
        dgl_core::command::quote(&wrapper).unwrap()
    );
    let first_command = if service == agent::Agent::Agy {
        command.replace("--event Stop", "--event SessionStart")
    } else {
        command.clone()
    };
    let source = format!(
        "#include <unistd.h>\n#include <stdlib.h>\nint main(void){{ int started=0; for(;;){{ if(access({},F_OK)==0){{system(started?{}:{});started=1;unlink({});}} usleep(100000); }} }}",
        serde_json::to_string(trigger.to_str().unwrap()).unwrap(),
        serde_json::to_string(&command).unwrap(),
        serde_json::to_string(&first_command).unwrap(),
        serde_json::to_string(trigger.to_str().unwrap()).unwrap()
    );
    std::fs::write(root.join("producer.c"), source).unwrap();
    assert!(
        std::process::Command::new("cc")
            .args([
                root.join("producer.c").to_str().unwrap(),
                "-o",
                root.join(executable).to_str().unwrap()
            ])
            .status()
            .unwrap()
            .success()
    );
    let (mut host, credential, fingerprint) = fixture("DGL_TEST_KEY_FILE");
    let mut inspector = ssh::connect(
        &host,
        &credential,
        Some(fingerprint.clone()),
        Arc::new(Mutex::new(None)),
    )
    .await
    .unwrap();
    inspector.discover().await.unwrap();
    inspector
        .execute_tmux(
            &dgl_core::command::Command::new(&Operation::NewSession {
                name: "agents".into(),
            })
            .unwrap(),
        )
        .await
        .unwrap();
    let (code, rows) = inspector
        .execute(&format!(
            "{} list-panes -t agents -F '#{{session_id}}|#{{window_id}}|#{{pane_id}}'",
            dgl_core::command::quote(&wrapper).unwrap()
        ))
        .await
        .unwrap();
    assert_eq!(code, 0);
    let row = String::from_utf8(rows).unwrap();
    let ids: Vec<_> = row.trim().split('|').collect();
    host.last_session = Some(ids[0].into());
    host.last_pane = Some(ids[2].into());
    let (code, hidden) = inspector
        .execute(&format!(
            "{} new-window -t {} -P -F '#{{pane_id}}' {}",
            dgl_core::command::quote(&wrapper).unwrap(),
            dgl_core::command::target(ids[0], '$').unwrap(),
            dgl_core::command::quote(root.join(executable).to_str().unwrap()).unwrap()
        ))
        .await
        .unwrap();
    assert_eq!(code, 0);
    let hidden = String::from_utf8(hidden).unwrap().trim().to_string();
    let c = Connection::start(host.clone(), credential.clone(), Some(fingerprint.clone()));
    phase(&c, Phase::Ready).await;
    // No output subscription at all: the separate observer must still collect hidden windows.
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if c.state()
                .agents
                .observations
                .iter()
                .any(|o| o.pane == hidden)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    assert!(c.state().agents.events.is_empty());
    // Exercise the same fallback used when Python is absent. This path only
    // executes tmux and ps and cannot claim a detailed hook observation.
    let basic = agent::basic_snapshot(&inspector, ids[0]).await.unwrap();
    assert!(
        basic
            .observations
            .iter()
            .any(|o| o.pane == hidden && o.agent == service)
    );
    assert!(
        basic
            .observations
            .iter()
            .all(|o| o.last.is_none() && o.supported.is_empty())
    );
    assert!(basic.events.is_empty());
    // The first service creates the DB; later services use its existing epoch.
    let had_database = root
        .join("home/.local/share/dgl-agent/v1/events.sqlite")
        .exists();
    std::fs::write(&trigger, b"1").unwrap();
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if c.state()
                .agents
                .observations
                .iter()
                .any(|o| o.last.is_some())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    if !had_database {
        assert!(c.state().agents.events.is_empty(), "{service:?}");
    }
    let first_cursor = c.state().agents.cursor;
    std::fs::write(&trigger, b"2").unwrap();
    let event = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if let Some(e) = c
                .state()
                .agents
                .events
                .iter()
                .find(|e| e.seq > first_cursor)
            {
                break e.clone();
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(event.kind, Kind::Response);
    assert_eq!(event.agent, service);
    assert_eq!(event.pane, hidden);
    // The ordinary client selected this window while creating it; GUI follows that session selection.
    assert!(c.state().remote.panes.iter().any(|p| p.id == hidden));
    completion(
        &c,
        c.action(Action::SelectTarget {
            session: event.session.clone(),
            window: Some(event.window.clone()),
            pane: Some(event.pane.clone()),
        })
        .unwrap(),
    )
    .await;
    assert_eq!(c.state().pane.as_ref(), Some(&hidden));
    let restored = Connection::start(host, credential, Some(fingerprint));
    phase(&restored, Phase::Ready).await;
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(restored.state().agents.events.is_empty());
    let (code, _) = inspector
        .execute(&format!(
            "{} respawn-pane -k -t {} {}",
            dgl_core::command::quote(&wrapper).unwrap(),
            dgl_core::command::target(&hidden, '%').unwrap(),
            dgl_core::command::quote(root.join(executable).to_str().unwrap()).unwrap()
        ))
        .await
        .unwrap();
    assert_eq!(code, 0);
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if c.state().agents.observations.iter().any(|o| {
                o.pane == hidden && o.run != event.run && o.last.is_none() && o.supported.is_empty()
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    inspector
        .execute_tmux(
            &dgl_core::command::Command::new(&Operation::KillPane {
                target: hidden.clone(),
            })
            .unwrap(),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if !c
                .state()
                .agents
                .observations
                .iter()
                .any(|o| o.pane == hidden)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    c.close();
    restored.close();
    c.join().await;
    restored.join().await;
    inspector
        .execute_tmux(
            &dgl_core::command::Command::new(&Operation::KillSession {
                target: ids[0].into(),
            })
            .unwrap(),
        )
        .await
        .unwrap();
    inspector.close().await;
}
#[tokio::test]
#[ignore = "requires scripts/integration.py disposable OpenSSH/tmux"]
async fn isolated_auth_management_input_reconnect() {
    let (host, credential, fingerprint) = fixture("DGL_TEST_KEY_FILE");
    for trusted in [None, Some("SHA256:changed".into())] {
        let observed = Arc::new(Mutex::new(None));
        let result = ssh::connect(&host, &credential, trusted.clone(), observed.clone()).await;
        assert_eq!(
            result.err(),
            Some(if trusted.is_none() {
                Error::HostKeyUnknown
            } else {
                Error::HostKeyChanged
            })
        );
        assert_eq!(*observed.lock().unwrap(), Some(fingerprint.clone()));
    }
    for key in [
        "DGL_TEST_KEY_FILE",
        "DGL_TEST_RSA_FILE",
        "DGL_TEST_ENCRYPTED_FILE",
    ] {
        let (h, c, f) = fixture(key);
        ssh::connect(&h, &c, Some(f), Arc::new(Mutex::new(None)))
            .await
            .unwrap()
            .close()
            .await;
    }
    let mut bad = credential.clone();
    bad.private_key = "invalid private key".into();
    assert_eq!(
        ssh::connect(
            &host,
            &bad,
            Some(fingerprint.clone()),
            Arc::new(Mutex::new(None))
        )
        .await
        .err(),
        Some(Error::Authentication)
    );
    let mut inspector = ssh::connect(
        &host,
        &credential,
        Some(fingerprint.clone()),
        Arc::new(Mutex::new(None)),
    )
    .await
    .unwrap();
    assert!(inspector.discover().await.unwrap());
    // A private forwarding socket lets us interrupt transport without killing tmux.
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let mut forwarded = host.clone();
    forwarded.port = listener.local_addr().unwrap().port();
    let bridges = Arc::new(Mutex::new(Vec::<tokio::task::AbortHandle>::new()));
    let bridge_list = bridges.clone();
    let remote_port = host.port;
    let proxy = tokio::spawn(async move {
        loop {
            let (mut client, _) = listener.accept().await.unwrap();
            let task = tokio::spawn(async move {
                let mut server = tokio::net::TcpStream::connect(("127.0.0.1", remote_port))
                    .await
                    .unwrap();
                let _ = tokio::io::copy_bidirectional(&mut client, &mut server).await;
            });
            bridge_list.lock().unwrap().push(task.abort_handle());
        }
    });
    let c = Connection::start(forwarded, credential.clone(), Some(fingerprint.clone()));
    let sub = c.output().subscribe().unwrap();
    phase(&c, Phase::Selecting).await;
    completion(
        &c,
        c.action(Action::Manage {
            operation: Operation::NewSession {
                name: "rust candidate;".into(),
            },
        })
        .unwrap(),
    )
    .await;
    let session = c.state().remote.sessions[0].id.clone();
    completion(
        &c,
        c.action(Action::SelectSession {
            id: session.clone(),
        })
        .unwrap(),
    )
    .await;
    phase(&c, Phase::Ready).await;
    let target = c.target().unwrap();
    let pane = target.pane_id.clone();
    let pid_command = format!(
        "{} display-message -p -t {} '#{{pane_pid}}'",
        dgl_core::command::quote(inspector.tmux.as_deref().unwrap()).unwrap(),
        dgl_core::command::target(&pane, '%').unwrap()
    );
    let (_, pid) = inspector.execute(&pid_command).await.unwrap();
    let mut kinds = vec![];
    while let Some(e) = c.output().poll(sub).unwrap() {
        kinds.push(e.kind);
        c.output().ack(sub, e.sequence).unwrap();
    }
    assert_eq!(kinds.first(), Some(&dgl_core::flow::OutputKind::Resize));
    assert!(kinds.contains(&dgl_core::flow::OutputKind::ResetEnd));
    completion(
        &c,
        c.input(target.clone(), b"printf 'RUST_CANDIDATE_OK\\n'\r".to_vec())
            .unwrap(),
    )
    .await;
    let mut output = vec![];
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            while let Some(e) = c.output().poll(sub).unwrap() {
                output.extend(e.bytes);
                c.output().ack(sub, e.sequence).unwrap();
            }
            if String::from_utf8_lossy(&output).contains("RUST_CANDIDATE_OK") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    completion(
        &c,
        c.action(Action::Manage {
            operation: Operation::Split {
                target: pane.clone(),
                horizontal: true,
            },
        })
        .unwrap(),
    )
    .await;
    assert_eq!(c.state().remote.panes.len(), 2);
    assert_eq!(
        c.input(target.clone(), b"no replay".to_vec()),
        Err(Error::StaleGeneration)
    );
    let drain_output = c.output();
    let drain = tokio::spawn(async move {
        loop {
            while let Ok(Some(event)) = drain_output.poll(sub) {
                let _ = drain_output.ack(sub, event.sequence);
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    });
    let generation = c.state().generation.connection;
    for bridge in bridges.lock().unwrap().drain(..) {
        bridge.abort();
    }
    phase(&c, Phase::Reconnecting).await;
    assert_eq!(
        c.input(target.clone(), b"never replay this".to_vec()),
        Err(Error::NotReady)
    );
    let handle = c.state().handle;
    completion(&c, c.action(Action::StopRetrying).unwrap()).await;
    assert!(c.state().retry.stopped);
    assert_eq!(c.state().phase, Phase::Failed);
    assert_eq!(c.state().session.as_deref(), Some(session.as_str()));
    completion(&c, c.action(Action::RetryNow).unwrap()).await;
    phase(&c, Phase::Ready).await;
    assert_eq!(c.state().handle, handle);
    assert_eq!(c.state().retry.attempt, 0);
    assert!(!c.state().retry.stopped);
    assert_eq!(c.action(Action::StopRetrying), Err(Error::NotReady));
    assert!(c.state().generation.connection > generation);
    assert_eq!(c.state().session.as_deref(), Some(session.as_str()));
    assert_eq!(inspector.execute(&pid_command).await.unwrap().1, pid);
    // A subsequent outage still gets automatic recovery, with a fresh budget.
    for bridge in bridges.lock().unwrap().drain(..) {
        bridge.abort();
    }
    phase(&c, Phase::Reconnecting).await;
    assert_eq!(c.state().retry.attempt, 0);
    phase(&c, Phase::Ready).await;
    assert_eq!(c.state().retry.attempt, 0);
    assert_eq!(c.state().handle, handle);
    assert_eq!(inspector.execute(&pid_command).await.unwrap().1, pid);
    assert_eq!(
        c.input(target, b"old target".to_vec()),
        Err(Error::StaleGeneration)
    );
    // Stop acknowledging an actual SSH output stream, then close at the queue bound.
    drain.abort();
    let _ = drain.await;
    c.input(c.target().unwrap(), b"i=0; while [ $i -lt 12000 ]; do printf 'SLOW_CONSUMER_0123456789012345678901234567890123456789\\n'; i=$((i+1)); done\r".to_vec()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let (bytes, events) = c.output().metrics();
            if bytes == dgl_core::flow::MAX_OUTPUT || events == dgl_core::flow::MAX_EVENTS {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    c.close();
    tokio::time::timeout(Duration::from_secs(3), c.join())
        .await
        .unwrap();
    proxy.abort();
    for bridge in bridges.lock().unwrap().drain(..) {
        bridge.abort();
    }
    assert_eq!(c.output().metrics(), (0, 0));
    assert_eq!(inspector.execute(&pid_command).await.unwrap().1, pid);
    let mut restored = host;
    restored.last_session = Some(session.clone());
    restored.last_pane = Some(pane);
    // Repeated handles and an independent simultaneous connection do not own the remote PID.
    for _ in 0..3 {
        let first = Connection::start(
            restored.clone(),
            credential.clone(),
            Some(fingerprint.clone()),
        );
        let second = Connection::start(
            restored.clone(),
            credential.clone(),
            Some(fingerprint.clone()),
        );
        phase(&first, Phase::Ready).await;
        phase(&second, Phase::Ready).await;
        assert_ne!(first.state().handle, second.state().handle);
        first.close();
        first.join().await;
        assert_eq!(second.state().phase, Phase::Ready);
        second.close();
        second.join().await;
        assert_eq!(inspector.execute(&pid_command).await.unwrap().1, pid);
    }
    let c = Connection::start(restored, credential, Some(fingerprint));
    phase(&c, Phase::Ready).await;
    assert_eq!(inspector.execute(&pid_command).await.unwrap().1, pid);
    let (_, _) = inspector
        .execute_tmux(
            &dgl_core::command::Command::new(&Operation::KillSession {
                target: session.clone(),
            })
            .unwrap(),
        )
        .await
        .unwrap();
    phase(&c, Phase::MissingSession).await;
    assert!(c.state().remote.sessions.iter().all(|s| s.id != session));
    assert!(c.target().is_none());
    completion(
        &c,
        c.action(Action::Manage {
            operation: Operation::NewSession {
                name: "chosen replacement".into(),
            },
        })
        .unwrap(),
    )
    .await;
    assert_eq!(
        c.state().phase,
        Phase::MissingSession,
        "new sessions are never selected automatically"
    );
    let replacement = c
        .state()
        .remote
        .sessions
        .iter()
        .find(|s| s.name == "chosen replacement")
        .unwrap()
        .id
        .clone();
    completion(
        &c,
        c.action(Action::SelectSession { id: replacement }).unwrap(),
    )
    .await;
    phase(&c, Phase::Ready).await;
    c.close();
    c.join().await;
    inspector.close().await;
}

#[tokio::test]
#[ignore = "requires scripts/integration.py disposable OpenSSH/tmux"]
async fn workspace_focus_tree_and_independent_connections() {
    use dgl_core::{command::Command, flow::OutputKind};
    let (host, credential, fingerprint) = fixture("DGL_TEST_KEY_FILE");
    let mut inspector = ssh::connect(
        &host,
        &credential,
        Some(fingerprint.clone()),
        Arc::new(Mutex::new(None)),
    )
    .await
    .unwrap();
    assert!(inspector.discover().await.unwrap());
    let c = Connection::start(host.clone(), credential.clone(), Some(fingerprint.clone()));
    phase(&c, Phase::Selecting).await;
    for name in ["ux-first", "ux-other"] {
        completion(
            &c,
            c.action(Action::Manage {
                operation: Operation::NewSession { name: name.into() },
            })
            .unwrap(),
        )
        .await;
    }
    // Initial navigation already contains windows/panes for every session.
    let remote = c.state().remote;
    let first = remote
        .sessions
        .iter()
        .find(|s| s.name == "ux-first")
        .unwrap()
        .id
        .clone();
    let other = remote
        .sessions
        .iter()
        .find(|s| s.name == "ux-other")
        .unwrap()
        .id
        .clone();
    let first_window = remote
        .windows
        .iter()
        .find(|w| w.session == first)
        .unwrap()
        .id
        .clone();
    let left = remote
        .panes
        .iter()
        .find(|p| p.window == first_window)
        .unwrap()
        .id
        .clone();
    let other_window = remote
        .windows
        .iter()
        .find(|w| w.session == other)
        .unwrap()
        .id
        .clone();
    let other_pane = remote
        .panes
        .iter()
        .find(|p| p.window == other_window)
        .unwrap()
        .id
        .clone();
    let sub = c.output().subscribe().unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let recorded = events.clone();
    let output = c.output();
    let drain = tokio::spawn(async move {
        loop {
            while let Ok(Some(e)) = output.poll(sub) {
                output.ack(sub, e.sequence).unwrap();
                recorded.lock().unwrap().push(e);
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    });
    completion(
        &c,
        c.action(Action::SelectTarget {
            session: first.clone(),
            window: Some(first_window.clone()),
            pane: Some(left.clone()),
        })
        .unwrap(),
    )
    .await;
    let mut second_host = host;
    second_host.last_session = Some(first.clone());
    second_host.last_pane = Some(left.clone());
    let second = Connection::start(second_host, credential, Some(fingerprint));
    phase(&second, Phase::Ready).await;
    assert_ne!(c.state().handle, second.state().handle);
    completion(
        &c,
        c.action(Action::Manage {
            operation: Operation::Split {
                target: left.clone(),
                horizontal: true,
            },
        })
        .unwrap(),
    )
    .await;
    let right = c.state().pane.unwrap();
    assert_ne!(right, left);
    for (direction, expected) in [('L', left.clone()), ('R', right.clone())] {
        let previous = c.target().unwrap();
        completion(
            &c,
            c.action(Action::Manage {
                operation: Operation::FocusPane {
                    target: previous.pane_id.clone(),
                    direction,
                },
            })
            .unwrap(),
        )
        .await;
        assert_eq!(c.target().unwrap().pane_id, expected);
        assert_eq!(
            c.input(previous, b"stale".to_vec()),
            Err(Error::StaleGeneration)
        );
    }
    completion(
        &c,
        c.action(Action::Manage {
            operation: Operation::Split {
                target: right.clone(),
                horizontal: false,
            },
        })
        .unwrap(),
    )
    .await;
    let bottom = c.state().pane.unwrap();
    assert_ne!(bottom, right);
    for (direction, expected) in [
        ('U', right.clone()),
        ('D', bottom.clone()),
        ('L', left.clone()),
        ('R', bottom.clone()),
    ] {
        let source = c.target().unwrap().pane_id;
        completion(
            &c,
            c.action(Action::Manage {
                operation: Operation::FocusPane {
                    target: source,
                    direction,
                },
            })
            .unwrap(),
        )
        .await;
        // Right from a tall pane may choose either right-hand neighbor; ask tmux for its authoritative active pane.
        let state = c.state();
        let actual = state
            .remote
            .panes
            .iter()
            .find(|p| p.window == first_window && p.active)
            .unwrap()
            .id
            .clone();
        if direction != 'R' {
            assert_eq!(actual, expected);
        }
        assert_eq!(c.target().unwrap().pane_id, actual);
        let marker = format!("UX_FOCUS_{direction}");
        completion(
            &c,
            c.input(
                c.target().unwrap(),
                format!("printf '{marker}\\n'\r").into_bytes(),
            )
            .unwrap(),
        )
        .await;
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let (_, bytes) = inspector
                    .execute_tmux(
                        &Command::new(&Operation::Capture {
                            target: actual.clone(),
                        })
                        .unwrap(),
                    )
                    .await
                    .unwrap();
                if String::from_utf8_lossy(&bytes).contains(&marker) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while second.state().pane.as_ref() != Some(&actual) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }
    completion(
        &c,
        c.action(Action::SelectTarget {
            session: other.clone(),
            window: Some(other_window),
            pane: Some(other_pane.clone()),
        })
        .unwrap(),
    )
    .await;
    assert_eq!(c.state().session.as_ref(), Some(&other));
    assert_eq!(c.target().unwrap().pane_id, other_pane);
    tokio::time::sleep(Duration::from_millis(200)).await;
    let before_resize = c.target().unwrap();
    events.lock().unwrap().clear();
    completion(
        &c,
        c.action(Action::Resize {
            columns: 101,
            rows: 37,
        })
        .unwrap(),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let target = c.target().unwrap();
    assert_eq!(target.connection, before_resize.connection);
    assert_eq!(
        target.generation.connection,
        before_resize.generation.connection
    );
    assert_eq!(target.generation.pane, before_resize.generation.pane);
    assert_eq!(target.generation.size, before_resize.generation.size + 1);
    assert!(!c.state().resizing);
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.kind == OutputKind::ResetBegin)
            .count(),
        1,
        "one resize must not be captured again for its layout notification"
    );
    assert!(
        events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.kind == OutputKind::ResetEnd
                && e.target == c.state().render_targets[&target.pane_id])
    );
    assert!(
        events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.kind == OutputKind::Resize
                && e.target.pane_id == other_pane
                && e.size.columns == 101
                && e.size.rows == 37)
    );
    assert_eq!(second.state().session.as_ref(), Some(&first));
    assert!(
        second
            .state()
            .remote
            .panes
            .iter()
            .any(|p| Some(&p.id) == second.state().pane.as_ref() && p.window == first_window)
    );
    events.lock().unwrap().clear();
    completion(
        &c,
        c.action(Action::Resize {
            columns: 101,
            rows: 37,
        })
        .unwrap(),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        c.target().unwrap(),
        target,
        "identical dimensions keep the output generation"
    );
    assert!(
        !events
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e.kind, OutputKind::ResetBegin | OutputKind::Resize))
    );
    // Another client can constrain the actual pane while Android's viewport changes.
    let (mut constrained_host, constrained_credential, constrained_fingerprint) =
        fixture("DGL_TEST_KEY_FILE");
    constrained_host.last_session = Some(other.clone());
    constrained_host.last_pane = Some(other_pane.clone());
    let constrained = Connection::start(
        constrained_host,
        constrained_credential,
        Some(constrained_fingerprint),
    );
    phase(&constrained, Phase::Ready).await;
    let result = std::process::Command::new(std::env::var("DGL_TEST_TMUX_WRAPPER").unwrap())
        .args(["set-option", "-w", "-t", &other, "window-size", "smallest"])
        .output()
        .unwrap();
    assert!(result.status.success());
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let s = c.state();
            if s.phase == Phase::Ready
                && s.remote
                    .panes
                    .iter()
                    .any(|p| p.id == other_pane && p.width == 80 && p.height == 24)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    events.lock().unwrap().clear();
    let constrained_target = c.target().unwrap();
    completion(
        &c,
        c.action(Action::Resize {
            columns: 135,
            rows: 45,
        })
        .unwrap(),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(c.target().unwrap(), constrained_target);
    assert!(
        !events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.kind == OutputKind::ResetBegin)
    );
    assert!(
        events.lock().unwrap().is_empty(),
        "unchanged server grid needs no renderer resize"
    );
    constrained.close();
    constrained.join().await;
    c.close();
    c.join().await;
    second.close();
    second.join().await;
    drain.abort();
    for target in [first, other] {
        inspector
            .execute_tmux(&Command::new(&Operation::KillSession { target }).unwrap())
            .await
            .unwrap();
    }
    inspector.close().await;
}

#[tokio::test]
#[ignore = "requires scripts/integration.py; isolated SSH/tmux metadata subscriptions"]
async fn hierarchy_titles_all_sessions_without_screen_reset() {
    use dgl_core::{connection::State, flow::OutputKind};
    fn tmux(args: &[&str]) -> String {
        let output = std::process::Command::new(std::env::var("DGL_TEST_TMUX_WRAPPER").unwrap())
            .arg("-u")
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }
    async fn until(c: &Connection, predicate: impl Fn(&State) -> bool) {
        tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                let state = c.state();
                assert!(
                    !matches!(state.phase, Phase::Failed | Phase::Closed),
                    "{state:?}"
                );
                if predicate(&state) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("metadata timeout: {:?}", c.state()));
    }
    fn title(pane: &str, value: &str) {
        tmux(&["select-pane", "-t", pane, "-T", &value.replace('#', "##")]);
    }
    let root = std::path::PathBuf::from(std::env::var("DGL_TEST_ROOT").unwrap());
    let long = "구분하세요 tmux session 🚀 | ' \\ #{pane_id} ".repeat(8);
    let script = root.join("hierarchy-input.sh");
    let received = root.join("hierarchy-input.txt");
    std::fs::write(
        &script,
        format!(
            "printf '\\033]2;%s\\007' {}\nstty -echo\nwhile IFS= read -r line; do case \"$line\" in title:*) printf '\\033]2;%s\\007' \"${{line#title:}}\";; *) printf '%s\\n' \"$line\" >> {};; esac; done\n",
            dgl_core::command::quote(&long).unwrap(),
            dgl_core::command::quote(received.to_str().unwrap()).unwrap()
        ),
    )
    .unwrap();
    let command = format!(
        "/bin/sh {}",
        dgl_core::command::quote(script.to_str().unwrap()).unwrap()
    );
    let ids = tmux(&[
        "new-session",
        "-d",
        "-s",
        "hierarchy-a",
        "-x",
        "100",
        "-y",
        "30",
        "-P",
        "-F",
        "#{session_id}|#{window_id}|#{pane_id}",
        &command,
    ]);
    let ids: Vec<_> = ids.split('|').collect();
    let (session, window, pane) = (ids[0], ids[1], ids[2]);
    let sibling = tmux(&[
        "split-window",
        "-h",
        "-d",
        "-t",
        pane,
        "-P",
        "-F",
        "#{pane_id}",
        "sleep 120",
    ]);
    let hidden = tmux(&[
        "new-window",
        "-d",
        "-t",
        session,
        "-n",
        "hidden",
        "-P",
        "-F",
        "#{window_id}|#{pane_id}",
        "sleep 120",
    ]);
    let hidden: Vec<_> = hidden.split('|').collect();
    let other = tmux(&[
        "new-session",
        "-d",
        "-s",
        "hierarchy-b",
        "-P",
        "-F",
        "#{session_id}|#{window_id}|#{pane_id}",
        "sleep 120",
    ]);
    let other: Vec<_> = other.split('|').collect();
    let other_hidden = tmux(&[
        "new-window",
        "-d",
        "-t",
        other[0],
        "-P",
        "-F",
        "#{pane_id}",
        "sleep 120",
    ]);

    title(&sibling, "다른 패널");
    title(hidden[1], "숨겨진 창");
    title(other[2], "다른 세션");
    title(&other_hidden, "다른 세션의 숨겨진 창");
    let (host, credential, fingerprint) = fixture("DGL_TEST_KEY_FILE");
    let c = Connection::start(host.clone(), credential.clone(), Some(fingerprint.clone()));
    let sub = c.output().subscribe().unwrap();
    phase(&c, Phase::Selecting).await;
    assert_eq!(
        c.state()
            .remote
            .panes
            .iter()
            .find(|p| p.id == pane)
            .unwrap()
            .title
            .trim_end(),
        // tmux 3.7 sanitizes stored titles (including backslashes); preserve
        // exactly the server's pane_title rather than undoing its own rules.
        tmux(&["display-message", "-p", "-t", pane, "#{pane_title}"])
    );
    completion(
        &c,
        c.action(Action::SelectTarget {
            session: session.into(),
            window: Some(window.into()),
            pane: Some(pane.into()),
        })
        .unwrap(),
    )
    .await;
    phase(&c, Phase::Ready).await;
    tokio::time::sleep(Duration::from_millis(1200)).await;
    let target = c.target().unwrap();
    let pid = tmux(&["display-message", "-p", "-t", pane, "#{pane_pid}"]);
    while let Some(event) = c.output().poll(sub).unwrap() {
        c.output().ack(sub, event.sequence).unwrap();
    }
    completion(
        &c,
        c.input(target.clone(), b"exactly-once\r".to_vec()).unwrap(),
    )
    .await;
    // Isolate the OSC path from the existing one-second metadata subscription.
    let client = tmux(&["list-clients", "-t", session, "-F", "#{client_name}"]);
    assert_eq!(client.lines().count(), 1);
    tmux(&["refresh-client", "-t", &client, "-B", "dgl-metadata"]);
    tmux(&["set-window-option", "-t", window, "automatic-rename", "off"]);
    tokio::time::sleep(Duration::from_millis(300)).await;
    c.set_title_updates_enabled(true);
    for value in ["빠른 제목 🚀 | ' #{}", "마지막 제목", ""] {
        tmux(&["send-keys", "-t", pane, "-l", &format!("title:{value}")]);
        tmux(&["send-keys", "-t", pane, "Enter"]);
        until(&c, |s| {
            s.remote
                .panes
                .iter()
                .any(|p| p.id == pane && p.title == value)
        })
        .await;
        assert_eq!(c.target().unwrap(), target);
        assert_eq!(c.state().phase, Phase::Ready);
    }
    c.set_title_updates_enabled(false);
    tmux(&[
        "send-keys",
        "-t",
        pane,
        "-l",
        "title:hidden fast path disabled",
    ]);
    tmux(&["send-keys", "-t", pane, "Enter"]);
    tokio::time::sleep(Duration::from_millis(650)).await;
    assert!(
        c.state()
            .remote
            .panes
            .iter()
            .find(|p| p.id == pane)
            .unwrap()
            .title
            .is_empty()
    );
    c.set_title_updates_enabled(true);
    tokio::time::sleep(Duration::from_millis(550)).await;
    assert!(
        c.state()
            .remote
            .panes
            .iter()
            .find(|p| p.id == pane)
            .unwrap()
            .title
            .is_empty(),
        "returning alone must not poll"
    );
    tmux(&["send-keys", "-t", pane, "-l", "title:foreground again"]);
    tmux(&["send-keys", "-t", pane, "Enter"]);
    until(&c, |s| {
        s.remote
            .panes
            .iter()
            .any(|p| p.id == pane && p.title == "foreground again")
    })
    .await;
    let mut live = Vec::new();
    while let Some(event) = c.output().poll(sub).unwrap() {
        assert_ne!(
            event.kind,
            OutputKind::ResetBegin,
            "title queries must preserve the screen"
        );
        if event.kind == OutputKind::Live {
            live.extend_from_slice(&event.bytes);
        }
        c.output().ack(sub, event.sequence).unwrap();
    }
    assert!(
        live.windows(b"\x1b]2;foreground again\x07".len())
            .any(|w| w == b"\x1b]2;foreground again\x07"),
        "OSC output must be delivered verbatim"
    );
    c.set_title_updates_enabled(false);
    let subscription = dgl_core::command::Command::new(&Operation::MetadataSubscription)
        .unwrap()
        .wire;
    let format = subscription
        .strip_prefix("refresh-client -B '")
        .unwrap()
        .strip_suffix('\'')
        .unwrap();
    tmux(&["refresh-client", "-t", &client, "-B", format]);
    // A running program changes the OSC title without a tmux rename command.
    // The format subscription must update metadata without changing the input target.
    tmux(&["send-keys", "-t", pane, "-l", "title:OSC live title 🚀"]);
    tmux(&["send-keys", "-t", pane, "Enter"]);
    until(&c, |s| {
        s.remote
            .panes
            .iter()
            .any(|p| p.id == pane && p.title == "OSC live title 🚀")
    })
    .await;
    assert_eq!(c.target().unwrap(), target);
    title(pane, "제목 변경 🚀 | ' #(");
    until(&c, |s| {
        s.remote
            .panes
            .iter()
            .any(|p| p.id == pane && p.title == "제목 변경 🚀 | ' #(")
    })
    .await;
    title(hidden[1], "숨긴 창 변경");
    title(&other_hidden, "다른 세션 변경");
    until(&c, |s| {
        s.remote
            .panes
            .iter()
            .any(|p| p.id == hidden[1] && p.title == "숨긴 창 변경")
            && s.remote
                .panes
                .iter()
                .any(|p| p.id == other_hidden && p.title == "다른 세션 변경")
    })
    .await;
    tmux(&["select-pane", "-t", &sibling]);
    until(&c, |s| {
        s.remote
            .panes
            .iter()
            .any(|p| p.id == sibling && p.active && p.title == "다른 패널")
    })
    .await;
    completion(
        &c,
        c.action(Action::Manage {
            operation: Operation::RenameSession {
                target: session.into(),
                name: "renamed-session".into(),
            },
        })
        .unwrap(),
    )
    .await;
    completion(
        &c,
        c.action(Action::Manage {
            operation: Operation::RenameWindow {
                target: window.into(),
                name: "renamed-window".into(),
            },
        })
        .unwrap(),
    )
    .await;
    completion(
        &c,
        c.action(Action::Manage {
            operation: Operation::RenamePane {
                target: pane.into(),
                name: "패널 제목 변경 #S".into(),
            },
        })
        .unwrap(),
    )
    .await;
    until(&c, |s| {
        s.remote
            .panes
            .iter()
            .any(|p| p.id == pane && p.title == "패널 제목 변경 #S")
    })
    .await;
    tmux(&[
        "move-window",
        "-d",
        "-s",
        hidden[0],
        "-t",
        "renamed-session:9",
    ]);
    until(&c, |s| {
        s.remote
            .windows
            .iter()
            .any(|w| w.id == hidden[0] && w.index == 9)
    })
    .await;
    title(&sibling, "");
    until(&c, |s| {
        s.remote
            .panes
            .iter()
            .any(|p| p.id == sibling && p.title.is_empty())
            && s.remote
                .windows
                .iter()
                .any(|w| w.id == window && w.name == "renamed-window")
    })
    .await;
    // Process changes are observed even when neither title nor window name changes.
    tmux(&["respawn-pane", "-k", "-t", &other_hidden, "cat"]);
    until(&c, |s| {
        s.remote
            .panes
            .iter()
            .any(|p| p.id == other_hidden && p.command == "cat")
    })
    .await;
    completion(&c, c.action(Action::Refresh).unwrap()).await;
    assert_eq!(c.target().unwrap().pane_id, sibling);
    assert_eq!(c.state().render_targets.len(), 2);
    assert_eq!(c.state().phase, Phase::Ready);
    while let Some(event) = c.output().poll(sub).unwrap() {
        assert_eq!(
            event.kind,
            OutputKind::Live,
            "metadata caused terminal reset"
        );
        c.output().ack(sub, event.sequence).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(&received).unwrap(),
        "exactly-once\n"
    );
    let snapshot = c.state();
    c.close();
    c.join().await;
    title(pane, "재접속 제목");
    let mut reconnect_host = host;
    reconnect_host.last_session = Some(session.into());
    reconnect_host.last_pane = Some(pane.into());
    let c = Connection::start(reconnect_host, credential, Some(fingerprint));
    let sub = c.output().subscribe().unwrap();
    phase(&c, Phase::Ready).await;
    assert_eq!(c.target().unwrap().pane_id, sibling);
    assert_eq!(
        tmux(&["display-message", "-p", "-t", pane, "#{pane_pid}"]),
        pid
    );
    assert_eq!(
        c.state()
            .remote
            .panes
            .iter()
            .find(|p| p.id == pane)
            .unwrap()
            .title,
        "재접속 제목"
    );
    assert_eq!(
        c.state().remote.windows.len(),
        snapshot.remote.windows.len()
    );
    title(pane, "재접속 후 변경");
    until(&c, |s| {
        s.remote
            .panes
            .iter()
            .any(|p| p.id == pane && p.title == "재접속 후 변경")
    })
    .await;
    while let Some(event) = c.output().poll(sub).unwrap() {
        c.output().ack(sub, event.sequence).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(&received).unwrap(),
        "exactly-once\n"
    );
    // A hidden/background renderer has no output consumer. Title events still
    // reach the metadata listener for every session and window.
    c.output().unsubscribe(sub);
    let before = c.target().unwrap();
    tmux(&["send-keys", "-t", pane, "-l", "title:OSC while hidden"]);
    tmux(&["send-keys", "-t", pane, "Enter"]);
    until(&c, |s| {
        s.remote
            .panes
            .iter()
            .any(|p| p.id == pane && p.title == "OSC while hidden")
    })
    .await;
    assert_eq!(c.target().unwrap(), before);
    c.close();
    c.join().await;
    tmux(&["kill-session", "-t", session]);
    tmux(&["kill-session", "-t", other[0]]);
}

#[path = "ssh_integration/window_sync.rs"]
mod window_sync;

#[path = "ssh_integration/resume.rs"]
mod resume;

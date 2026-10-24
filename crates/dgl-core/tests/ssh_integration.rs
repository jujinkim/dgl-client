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
        assert_eq!(
            second.state().pane.as_ref(),
            Some(&left),
            "another connection keeps its displayed target"
        );
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
    completion(
        &c,
        c.action(Action::Resize {
            columns: 101,
            rows: 37,
        })
        .unwrap(),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    let target = c.target().unwrap();
    assert!(
        events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.kind == OutputKind::ResetEnd && e.target == target)
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
    assert_eq!(second.state().pane.as_ref(), Some(&left));
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

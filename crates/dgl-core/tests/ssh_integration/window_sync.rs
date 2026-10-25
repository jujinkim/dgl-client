use super::*;
use dgl_core::{
    connection::State,
    flow::{OutputEvent, OutputKind},
    model::Size,
};

async fn until(c: &Connection, predicate: impl Fn(&State) -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let s = c.state();
            assert!(!matches!(s.phase, Phase::Failed | Phase::Closed), "{s:?}");
            if s.phase == Phase::Ready && predicate(&s) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("state timeout: {:?}", c.state()));
}
async fn tmux(ssh: &ssh::Ssh, args: &str) -> String {
    let command = format!(
        "{} {args}",
        dgl_core::command::quote(ssh.tmux.as_ref().unwrap()).unwrap()
    );
    let (code, bytes) = ssh.execute(&command).await.unwrap();
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&bytes));
    String::from_utf8(bytes).unwrap().trim().to_owned()
}
fn drain(c: &Arc<Connection>) -> (Arc<Mutex<Vec<OutputEvent>>>, tokio::task::JoinHandle<()>) {
    let subscription = c.output().subscribe().unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let recorded = events.clone();
    let output = c.output();
    let task = tokio::spawn(async move {
        loop {
            while let Ok(Some(e)) = output.poll(subscription) {
                output.ack(subscription, e.sequence).unwrap();
                recorded.lock().unwrap().push(e);
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    });
    (events, task)
}
#[tokio::test]
#[ignore = "requires scripts/integration.py; real SSH PTY handoff and window-wide control output"]
async fn window_layout_output_and_ssh_handoff() {
    let (host, credential, fingerprint) = fixture("DGL_TEST_KEY_FILE");
    let mut ssh = ssh::connect(
        &host,
        &credential,
        Some(fingerprint.clone()),
        Arc::new(Mutex::new(None)),
    )
    .await
    .unwrap();
    assert!(ssh.discover().await.unwrap());
    let ids = tmux(
        &ssh,
        "new-session -d -s window-sync -x 110 -y 36 -P -F '#{session_id}|#{window_id}|#{pane_id}'",
    )
    .await;
    let ids: Vec<_> = ids.split('|').collect();
    let (session, window, left) = (ids[0], ids[1], ids[2]);
    let right = tmux(
        &ssh,
        &format!("split-window -h -d -t '{left}' -P -F '#{{pane_id}}'"),
    )
    .await;
    let bottom = tmux(
        &ssh,
        &format!("split-window -v -d -t '{right}' -P -F '#{{pane_id}}'"),
    )
    .await;
    let before = tmux(
        &ssh,
        &format!("list-panes -t '{window}' -F '#{{pane_id}}|#{{pane_pid}}|#{{pane_current_path}}'"),
    )
    .await;
    let mut profile = host.clone();
    profile.last_session = Some(session.into());
    let c = Connection::start(
        profile.clone(),
        credential.clone(),
        Some(fingerprint.clone()),
    );
    let (events, task) = drain(&c);
    until(&c, |s| s.render_targets.len() == 3).await;
    let initial = c.state();
    assert_eq!(initial.window.as_deref(), Some(window));
    let l = initial.remote.panes.iter().find(|p| p.id == left).unwrap();
    let r = initial.remote.panes.iter().find(|p| p.id == right).unwrap();
    let b = initial
        .remote
        .panes
        .iter()
        .find(|p| p.id == bottom)
        .unwrap();
    assert_eq!(l.left, 0);
    assert_eq!(r.left, l.width + 1);
    assert_eq!(b.left, r.left);
    assert_eq!(b.top, r.top + r.height + 1);
    let renders = initial.render_targets.clone();
    tokio::time::sleep(Duration::from_millis(200)).await;
    events.lock().unwrap().clear();
    for (i, pane) in [left, &right, &bottom].iter().enumerate() {
        tmux(
            &ssh,
            &format!("send-keys -t '{pane}' 'printf \"WINDOW_MARKER_{i}\\n\"' Enter"),
        )
        .await;
    }
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let found = {
                let all = events.lock().unwrap();
                [left, &right, &bottom].iter().enumerate().all(|(i, p)| {
                    let bytes: Vec<_> = all
                        .iter()
                        .filter(|e| e.kind == OutputKind::Live && &e.target.pane_id == p)
                        .flat_map(|e| e.bytes.iter().copied())
                        .collect();
                    String::from_utf8_lossy(&bytes).contains(&format!("WINDOW_MARKER_{i}"))
                })
            };
            if found {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    for pane in [right.as_str(), bottom.as_str(), left] {
        completion(
            &c,
            c.action(Action::SelectPane { id: pane.into() }).unwrap(),
        )
        .await;
        assert_eq!(
            c.state().render_targets,
            renders,
            "focus keeps every renderer identity"
        );
    }
    tmux(&ssh, &format!("select-pane -t '{right}'")).await;
    until(&c, |s| s.pane.as_deref() == Some(&right)).await;
    assert_eq!(c.state().render_targets, renders);
    assert!(
        !events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.kind == OutputKind::ResetBegin)
    );
    let old_input = c.target().unwrap();
    tmux(&ssh, &format!("select-pane -t '{bottom}'")).await;
    until(&c, |s| s.pane.as_deref() == Some(&bottom)).await;
    assert_eq!(
        c.input(old_input, b"must-not-send".to_vec()),
        Err(Error::StaleGeneration)
    );
    tmux(&ssh, &format!("resize-pane -Z -t '{bottom}'")).await;
    until(&c, |s| {
        s.remote.windows.iter().any(|w| w.id == window && w.zoomed) && s.render_targets.len() == 1
    })
    .await;
    assert!(c.state().render_targets.contains_key(&bottom));
    tmux(&ssh, &format!("resize-pane -Z -t '{bottom}'")).await;
    until(&c, |s| {
        s.remote.windows.iter().any(|w| w.id == window && !w.zoomed) && s.render_targets.len() == 3
    })
    .await;
    let unzoomed = c.state();
    let target_width = unzoomed
        .remote
        .panes
        .iter()
        .find(|p| p.id == left)
        .unwrap()
        .width
        + 2;
    completion(
        &c,
        c.action(Action::Manage {
            operation: Operation::ResizePaneTo {
                target: left.into(),
                columns: target_width,
                rows: l.height,
            },
        })
        .unwrap(),
    )
    .await;
    until(&c, |s| {
        s.remote
            .panes
            .iter()
            .any(|p| p.id == left && p.width == target_width)
    })
    .await;
    let log =
        std::path::PathBuf::from(std::env::var("DGL_TEST_ROOT").unwrap()).join("handoff-input.txt");
    let quoted_log = dgl_core::command::quote(log.to_str().unwrap()).unwrap();
    completion(
        &c,
        c.input(
            c.target().unwrap(),
            format!("printf 'GUI_ONCE\\n' >> {quoted_log}\r").into_bytes(),
        )
        .unwrap(),
    )
    .await;
    // A conventional SSH PTY attaches before the GUI leaves, preserving the same processes.
    let ordinary = ssh
        .shell(Size {
            columns: 97,
            rows: 31,
        })
        .await
        .unwrap();
    let (mut ordinary_read, ordinary) = ordinary.split();
    let ordinary_output = Arc::new(Mutex::new(Vec::new()));
    let observed = ordinary_output.clone();
    let reader = tokio::spawn(async move {
        while let Some(message) = ordinary_read.wait().await {
            if let russh::ChannelMsg::Data { data } = message {
                observed.lock().unwrap().extend_from_slice(&data);
            }
        }
    });
    ordinary
        .data(
            format!(
                "{} attach-session -t '{session}'\r",
                dgl_core::command::quote(ssh.tmux.as_ref().unwrap()).unwrap()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !tmux(
            &ssh,
            &format!("list-clients -t '{session}' -F '#{{client_control_mode}}'"),
        )
        .await
        .lines()
        .any(|s| s == "0")
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "ordinary SSH did not attach: {}",
            String::from_utf8_lossy(&ordinary_output.lock().unwrap())
        )
    });
    c.close();
    c.join().await;
    task.abort();
    ordinary
        .data(format!("printf 'SSH_ONCE\\n' >> {quoted_log}\r").as_bytes())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while std::fs::read_to_string(&log).unwrap_or_default() != "GUI_ONCE\nSSH_ONCE\n" {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        tmux(
            &ssh,
            &format!(
                "list-panes -t '{window}' -F '#{{pane_id}}|#{{pane_pid}}|#{{pane_current_path}}'"
            )
        )
        .await,
        before
    );
    tmux(&ssh, &format!("select-pane -t '{left}'")).await;
    // A stale remembered pane must not override selection made in ordinary SSH.
    profile.last_pane = Some(bottom.clone());
    let next = Connection::start(profile, credential, Some(fingerprint));
    let (_events, next_task) = drain(&next);
    until(&next, |s| {
        s.render_targets.len() == 3 && s.pane.as_deref() == Some(left)
    })
    .await;
    let next_window = tmux(
        &ssh,
        &format!("new-window -t '{session}' -n external -P -F '#{{window_id}}'"),
    )
    .await;
    until(&next, |s| {
        s.window.as_deref() == Some(&next_window) && s.render_targets.len() == 1
    })
    .await;
    tmux(&ssh, &format!("kill-window -t '{next_window}'")).await;
    until(&next, |s| {
        s.window.as_deref() == Some(window) && s.render_targets.len() == 3
    })
    .await;
    tmux(&ssh, &format!("kill-pane -t '{right}'")).await;
    until(&next, |s| {
        s.render_targets.len() == 2 && !s.remote.panes.iter().any(|p| p.id == right)
    })
    .await;
    next.close();
    next.join().await;
    next_task.abort();
    reader.abort();
    tmux(&ssh, &format!("kill-session -t '{session}'")).await;
    ssh.close().await;
}

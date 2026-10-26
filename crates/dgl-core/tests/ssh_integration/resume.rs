use super::*;
use dgl_core::{
    flow::{MAX_EVENTS, OutputKind},
    model::Size,
};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Notify,
    task::JoinSet,
};

fn tmux(args: &[&str]) -> String {
    let output = std::process::Command::new(std::env::var("DGL_TEST_TMUX_WRAPPER").unwrap())
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}

async fn until(c: &Connection, predicate: impl Fn(&dgl_core::connection::State) -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !predicate(&c.state()) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("resume timeout: {:?}", c.state()));
}

#[tokio::test]
#[ignore = "requires scripts/integration.py; real SSH through a socket-preserving blackhole proxy"]
async fn healthy_resume_and_blackhole_recovery_preserve_work_without_replaying_input() {
    let (mut host, credential, fingerprint) = fixture("DGL_TEST_KEY_FILE");
    let root = std::path::PathBuf::from(std::env::var("DGL_TEST_ROOT").unwrap());
    let received = root.join("resume-input.txt");
    let script = root.join("resume-shell.sh");
    std::fs::write(&script, format!(
        "stty -echo\nprintf 'resume-screen-ready\\n'\nwhile IFS= read -r line; do printf '%s\\n' \"$line\" >> {}; printf 'received:%s\\n' \"$line\"; done\n",
        dgl_core::command::quote(received.to_str().unwrap()).unwrap()
    )).unwrap();
    let command = format!(
        "/bin/sh {}",
        dgl_core::command::quote(script.to_str().unwrap()).unwrap()
    );
    let session = tmux(&[
        "new-session",
        "-d",
        "-s",
        "resume-fixture",
        "-P",
        "-F",
        "#{session_id}",
        &command,
    ]);
    host.last_session = Some(session.clone());

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream = host.port;
    host.port = listener.local_addr().unwrap().port();
    let count = Arc::new(AtomicU64::new(0));
    let accepted = count.clone();
    let blackhole = Arc::new(Notify::new());
    let gate = blackhole.clone();
    let proxy = tokio::spawn(async move {
        let mut connections = JoinSet::new();
        loop {
            let (mut client, _) = listener.accept().await.unwrap();
            let first = accepted.fetch_add(1, Ordering::SeqCst) == 0;
            let gate = gate.clone();
            connections.spawn(async move {
                let mut server = TcpStream::connect(("127.0.0.1", upstream)).await.unwrap();
                if first {
                    tokio::select! {
                        _ = tokio::io::copy_bidirectional(&mut client, &mut server) => {}
                        _ = gate.notified() => {
                            // Keep both sockets open, but cease forwarding every byte.
                            std::future::pending::<()>().await;
                        }
                    }
                } else {
                    let _ = tokio::io::copy_bidirectional(&mut client, &mut server).await;
                }
            });
        }
    });
    let c = Connection::start(host, credential, Some(fingerprint));
    let queue = c.output();
    let sub = queue.subscribe().unwrap();
    phase(&c, Phase::Ready).await;
    let pane = c.target().unwrap().pane_id;
    let pid = tmux(&["display-message", "-p", "-t", &pane, "#{pane_pid}"]);
    let client_pid = tmux(&["list-clients", "-t", &session, "-F", "#{client_pid}"]);
    let events = Arc::new(Mutex::new(Vec::new()));
    let collect = |sub| {
        let queue = queue.clone();
        let events = events.clone();
        tokio::spawn(async move {
            loop {
                while let Some(event) = queue.poll(sub).unwrap() {
                    queue.ack(sub, event.sequence).unwrap();
                    events.lock().unwrap().push(event);
                }
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
    };
    let drain = collect(sub);
    until(&c, |_| {
        events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.kind == OutputKind::ResetEnd)
    })
    .await;
    events.lock().unwrap().clear();
    let before = c.target().unwrap();
    let render_targets = c.state().render_targets;
    let check = c.action(Action::CheckConnection).unwrap();
    assert!(c.state().checking);
    assert_eq!(
        c.input(before.clone(), b"blocked\r".to_vec()),
        Err(Error::NotReady)
    );
    c.action(Action::CheckConnection).unwrap();
    completion(&c, check).await;
    assert!(!c.state().checking);
    assert_eq!(before, c.target().unwrap());
    assert_eq!(render_targets, c.state().render_targets);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(
        tmux(&["list-clients", "-t", &session, "-F", "#{client_pid}"]),
        client_pid
    );
    assert!(
        !events.lock().unwrap().iter().any(|e| matches!(
            e.kind,
            OutputKind::ResetBegin | OutputKind::ResetChunk | OutputKind::ResetEnd
        )),
        "healthy checks must preserve the renderer without a Reset"
    );

    // A new renderer still gets a complete capture on explicit subscription sync.
    completion(&c, c.action(Action::Snapshot).unwrap()).await;
    until(&c, |_| {
        events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.kind == OutputKind::ResetEnd)
    })
    .await;

    drain.abort();
    let _ = drain.await;
    while let Some(event) = queue.poll(sub).unwrap() {
        queue.ack(sub, event.sequence).unwrap();
    }
    // Stall the terminal actor in output delivery, with a queued input behind it.
    let render = c.state().render_targets[&pane].clone();
    for _ in 0..MAX_EVENTS {
        queue
            .try_push(
                sub,
                render.clone(),
                OutputKind::Live,
                Size::default(),
                vec![],
            )
            .unwrap();
    }
    c.action(Action::Snapshot).unwrap();
    let pending_input = c
        .input(c.target().unwrap(), b"must-not-replay\r".to_vec())
        .unwrap();
    until(&c, |s| s.phase == Phase::Restoring).await;
    blackhole.notify_one();
    let started = tokio::time::Instant::now();
    let check = c.action(Action::CheckConnection).unwrap();
    tokio::time::timeout(Duration::from_millis(3300), async {
        while c.state().generation.connection == before.generation.connection {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("a blocked terminal actor must not delay the 3-second health deadline");
    assert!(started.elapsed() >= Duration::from_secs(3));
    let results = c.completions();
    assert!(
        results
            .iter()
            .any(|r| r.request_id == check && r.error == Some(Error::Network))
    );
    assert!(
        results
            .iter()
            .any(|r| r.request_id == pending_input && r.error == Some(Error::Cancelled))
    );
    assert_eq!(
        c.input(before.clone(), b"old\r".to_vec()),
        Err(Error::NotReady)
    );
    // Replacing a stopped consumer invalidates every old output buffer.
    let current_sub = queue.subscribe().unwrap();
    assert_eq!(queue.poll(sub).unwrap_err(), Error::StaleGeneration);
    events.lock().unwrap().clear();
    let drain = collect(current_sub);
    until(&c, |s| {
        s.phase == Phase::Ready && s.generation.connection > before.generation.connection
    })
    .await;
    assert_eq!(count.load(Ordering::SeqCst), 2);
    assert_eq!(c.target().unwrap().pane_id, pane);
    assert_eq!(c.state().retry.attempt, 0);
    assert_eq!(
        tmux(&["display-message", "-p", "-t", &pane, "#{pane_pid}"]),
        pid
    );
    assert_eq!(
        c.input(before, b"stale\r".to_vec()),
        Err(Error::StaleGeneration)
    );
    completion(
        &c,
        c.input(c.target().unwrap(), b"once\r".to_vec()).unwrap(),
    )
    .await;
    until(&c, |_| {
        std::fs::read_to_string(&received).unwrap_or_default() == "once\n"
    })
    .await;
    assert!(
        events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.kind == OutputKind::ResetEnd && e.subscription == current_sub)
    );
    assert!(
        events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.kind == OutputKind::ResetChunk)
            .flat_map(|e| e.bytes.iter().copied())
            .collect::<Vec<_>>()
            .windows(b"resume-screen-ready".len())
            .any(|w| w == b"resume-screen-ready")
    );

    // Stopping while a check is pending discards its eventual result.
    let check = c.action(Action::CheckConnection).unwrap();
    c.action(Action::StopRetrying).unwrap();
    until(&c, |s| s.retry.stopped).await;
    assert!(!c.state().checking);
    assert!(
        c.completions()
            .iter()
            .any(|r| r.request_id == check && r.error == Some(Error::Cancelled))
    );
    c.action(Action::CheckConnection).unwrap();
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(c.state().retry.stopped);
    assert_eq!(count.load(Ordering::SeqCst), 2);
    assert_eq!(std::fs::read_to_string(received).unwrap(), "once\n");
    drain.abort();
    c.close();
    c.join().await;
    proxy.abort();
    let _ = proxy.await;
    tmux(&["kill-session", "-t", &session]);
}

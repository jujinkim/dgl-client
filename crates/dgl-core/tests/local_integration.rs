//! Run explicitly with DGL_LOCAL_TMUX=/absolute/tmux cargo test -p dgl-core --test local_integration -- --ignored.
use dgl_core::{
    command::Operation,
    connection::{Action, Connection, Phase},
    local::LocalConfig,
};
use std::{sync::Arc, time::Duration};
struct Fixture {
    config: LocalConfig,
}
impl Fixture {
    fn new() -> Self {
        let executable =
            std::env::var_os("DGL_LOCAL_TMUX").expect("set DGL_LOCAL_TMUX to a real tmux binary");
        Self {
            config: LocalConfig {
                executable: executable.into(),
                arguments: vec![
                    "-L".into(),
                    format!("dgl-local-test-{}", std::process::id()),
                    "-f".into(),
                    "/dev/null".into(),
                ],
                ..Default::default()
            },
        }
    }
    fn external(&self, args: &[&str]) -> String {
        let out = std::process::Command::new(&self.config.executable)
            .args(&self.config.arguments)
            .args(args)
            .env_remove("TMUX")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::process::Command::new(&self.config.executable)
            .args(&self.config.arguments)
            .arg("kill-server")
            .output();
    }
}
async fn until(c: &Connection, f: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(15), async {
        while !f() {
            assert!(
                !matches!(c.state().phase, Phase::Failed | Phase::Closed),
                "{:?}",
                c.state()
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("state timeout: {:?}", c.state()));
}
async fn action(c: &Connection, op: Operation) {
    let id = c.action(Action::Manage { operation: op }).unwrap();
    until(c, || {
        c.completions()
            .into_iter()
            .find(|v| v.request_id == id)
            .is_some_and(|v| {
                assert_eq!(v.error, None);
                true
            })
    })
    .await;
}
fn drain(c: &Arc<Connection>) -> tokio::task::JoinHandle<()> {
    let out = c.output();
    let subscription = out.subscribe().unwrap();
    tokio::spawn(async move {
        loop {
            while let Ok(Some(event)) = out.poll(subscription) {
                out.ack(subscription, event.sequence).unwrap();
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
}
#[tokio::test]
#[ignore = "requires explicit DGL_LOCAL_TMUX; uses a private socket and never the user's server"]
async fn isolated_local_inventory_management_input_detach_and_reconnect() {
    let fixture = Fixture::new();
    let c = Connection::start_local(fixture.config.clone());
    until(&c, || c.state().phase == Phase::Selecting).await;
    assert!(c.state().remote.sessions.is_empty()); // no auto-create
    action(
        &c,
        Operation::NewSession {
            name: "local fixture;".into(),
        },
    )
    .await;
    assert_eq!(c.state().remote.sessions[0].name, "local fixture;");
    let session = c.state().remote.sessions[0].id.clone();
    let reader = drain(&c);
    c.action(Action::SelectSession {
        id: session.clone(),
    })
    .unwrap();
    until(&c, || c.state().phase == Phase::Ready).await;
    let pane = c.state().pane.clone().unwrap();
    action(
        &c,
        Operation::Split {
            target: pane.clone(),
            horizontal: true,
        },
    )
    .await;
    assert_eq!(c.state().remote.panes.len(), 2);
    action(
        &c,
        Operation::ResizePane {
            target: pane.clone(),
            direction: 'R',
        },
    )
    .await;
    action(
        &c,
        Operation::RenamePane {
            target: pane.clone(),
            name: "pane fixture".into(),
        },
    )
    .await;
    action(
        &c,
        Operation::NewWindow {
            target: session.clone(),
            name: "second".into(),
        },
    )
    .await;
    let window = c
        .state()
        .remote
        .windows
        .iter()
        .find(|w| w.name == "second")
        .unwrap()
        .id
        .clone();
    action(
        &c,
        Operation::SelectWindow {
            target: window.clone(),
        },
    )
    .await;
    action(
        &c,
        Operation::RenameWindow {
            target: window,
            name: "renamed".into(),
        },
    )
    .await;
    fixture.external(&["rename-session", "-t", &session, "external rename"]);
    until(&c, || {
        c.state()
            .remote
            .sessions
            .iter()
            .any(|s| s.name == "external rename")
    })
    .await;
    let stale = c.state().input_target().unwrap();
    let id = c
        .input(
            stale.clone(),
            b"printf 'DGL_LOCAL_%s\\n' INPUT_OK\n".to_vec(),
        )
        .unwrap();
    until(&c, || {
        c.completions()
            .iter()
            .any(|v| v.request_id == id && v.error.is_none())
    })
    .await;
    until(&c, || {
        fixture
            .external(&["capture-pane", "-p", "-t", &stale.pane_id])
            .lines()
            .any(|line| line == "DGL_LOCAL_INPUT_OK")
    })
    .await;
    c.close();
    c.join().await;
    reader.abort();
    assert!(c.input(stale, b"MUST_NOT_REPLAY".to_vec()).is_err());
    assert!(
        fixture
            .external(&["list-sessions", "-F", "#{session_name}"])
            .contains("external rename")
    );
    let mut config = fixture.config.clone();
    config.last_session = Some(session);
    config.last_pane = Some(pane.clone());
    let restored = Connection::start_local(config);
    let reader = drain(&restored);
    until(&restored, || restored.state().phase == Phase::Ready).await;
    assert_eq!(restored.state().pane.as_ref(), Some(&pane));
    assert!(
        !fixture
            .external(&["capture-pane", "-p"])
            .contains("MUST_NOT_REPLAY")
    );
    let target = restored.state().pane.clone().unwrap();
    action(&restored, Operation::KillPane { target }).await;
    restored.close();
    restored.join().await;
    reader.abort();
}

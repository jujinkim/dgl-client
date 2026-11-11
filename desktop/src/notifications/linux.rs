//! freedesktop notifications, with click IDs routed back to the inbox.
use super::*;
use futures_lite::StreamExt;
use std::collections::HashMap;
use zbus::{Connection, Proxy};
pub fn initialize() -> bool {
    std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some()
}
pub fn authorize() -> bool {
    initialize()
}
pub fn show(notice: &Notice) -> bool {
    let id = notice.id();
    let title = format!("{} · {}", notice.event.agent.label(), notice.event.pane);
    let body = notice.event.kind.label().to_string();
    if !initialize() {
        return false;
    }
    std::thread::Builder::new()
        .name("dgl-notification".into())
        .spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            runtime.block_on(async move {
                let _ = tokio::time::timeout(std::time::Duration::from_secs(10), async {
                    let connection = Connection::session().await.ok()?;
                    let proxy = Proxy::new(
                        &connection,
                        "org.freedesktop.Notifications",
                        "/org/freedesktop/Notifications",
                        "org.freedesktop.Notifications",
                    )
                    .await
                    .ok()?;
                    let mut actions = proxy.receive_signal("ActionInvoked").await.ok()?;
                    let hints: HashMap<&str, zbus::zvariant::Value<'_>> = HashMap::new();
                    let notification: u32 = proxy
                        .call(
                            "Notify",
                            &(
                                "DaGaLi",
                                0u32,
                                "kim.jujin.dglc",
                                title,
                                body,
                                vec!["default", "Open DGL"],
                                hints,
                                5000i32,
                            ),
                        )
                        .await
                        .ok()?;
                    while let Some(message) = actions.next().await {
                        if let Ok((found, _)) = message.body().deserialize::<(u32, String)>() {
                            if found == notification {
                                clicked(id);
                                break;
                            }
                        }
                    }
                    Some(())
                })
                .await;
            });
        })
        .is_ok()
}

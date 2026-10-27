//! Complete tray menu startup feedback without showing/focusing a window.
//!
//! GNOME sends an activation token for every menu click, including Quit and
//! playback controls. GTK3's notify_startup_complete is a no-op on Wayland when
//! xdg_activation is available. gtk_shell1.set_startup_id completes that startup
//! sequence without activating a surface. Use a short-lived connection so we
//! never dispatch or block GTK's display queue from a worker thread.
use anyhow::{Context, Result};
use gtk::{gdk, prelude::*};
use std::time::Duration;
use wayland_client::{protocol::wl_registry, Connection, Dispatch, QueueHandle};

#[allow(dead_code, unused_imports, non_camel_case_types, clippy::all)]
mod protocol {
    use wayland_client;
    use wayland_client::protocol::*;
    pub mod __interfaces {
        use wayland_client::backend as wayland_backend;
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/gtk-shell.xml");
    }
    use self::__interfaces::*;
    wayland_scanner::generate_client_code!("protocols/gtk-shell.xml");
}
use protocol::gtk_shell1::GtkShell1;

#[cfg(test)]
#[path = "startup_tests.rs"]
mod tests;

#[derive(Default)]
struct Startup {
    shell: Option<GtkShell1>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for Startup {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == "gtk_shell1" && version >= 1 && state.shell.is_none() {
                state.shell = Some(registry.bind(name, 1, qh, ()));
            }
        }
    }
}
wayland_client::delegate_noop!(Startup: ignore GtkShell1);

fn complete_wayland(token: String) -> Result<()> {
    let connection = Connection::connect_to_env().context("连接 Wayland 显示服务失败")?;
    complete_on_connection(connection, token)
}

fn complete_on_connection(connection: Connection, token: String) -> Result<()> {
    let mut queue = connection.new_event_queue();
    let _registry = connection.display().get_registry(&queue.handle(), ());
    let mut state = Startup::default();
    queue.roundtrip(&mut state)?;
    if let Some(shell) = state.shell {
        shell.set_startup_id(Some(token));
        // Ensure the compositor handled the completion before Quit can exit.
        // A flush alone only proves the request was queued to the socket.
        connection.roundtrip()?;
    }
    Ok(())
}

/// Must be called with the token captured in the original ksni callback.
/// Returns before executing Quit, but bounds notification work so a broken
/// desktop connection cannot indefinitely delay saving/shutting down playback.
pub(super) async fn complete(app: &tauri::AppHandle, token: Option<String>) {
    let Some(token) = token else { return };
    let (tx, rx) = tokio::sync::oneshot::channel();
    let gtk_token = token.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        let wayland = if let Some(display) = gdk::Display::default() {
            let wayland = display.type_().name() == "GdkWaylandDisplay";
            if !wayland {
                display.notify_startup_complete(&gtk_token);
                display.flush();
            }
            wayland
        } else {
            false
        };
        let _ = tx.send(wayland);
    }) {
        log::warn!("结束托盘启动提示失败: {error}");
        return;
    }
    let result = tokio::time::timeout(Duration::from_secs(1), async move {
        if rx.await.context("GTK 启动通知通道已关闭")? {
            tokio::task::spawn_blocking(move || complete_wayland(token)).await??;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await;
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => log::warn!("结束托盘启动提示失败: {error:#}"),
        Err(_) => log::warn!("结束托盘启动提示超时，继续执行菜单操作"),
    }
}

//! Exercise the real Wayland wire protocol on an isolated Unix socket pair.
use super::*;
use std::{
    os::unix::net::UnixStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use wayland_server::{Client, DataInit, Display, DisplayHandle, GlobalDispatch, New};

#[allow(dead_code, unused_imports, non_camel_case_types, clippy::all)]
mod server_protocol {
    use wayland_server;
    use wayland_server::protocol::*;
    pub mod __interfaces {
        use wayland_server::backend as wayland_backend;
        use wayland_server::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/gtk-shell.xml");
    }
    use self::__interfaces::*;
    wayland_scanner::generate_server_code!("protocols/gtk-shell.xml");
}
use server_protocol::gtk_shell1;

#[derive(Default)]
struct Server {
    completed: Vec<String>,
}
impl GlobalDispatch<gtk_shell1::GtkShell1, ()> for Server {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<gtk_shell1::GtkShell1>,
        _: &(),
        data: &mut DataInit<'_, Self>,
    ) {
        data.init(resource, ());
    }
}
impl wayland_server::Dispatch<gtk_shell1::GtkShell1, ()> for Server {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &gtk_shell1::GtkShell1,
        request: gtk_shell1::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        match request {
            gtk_shell1::Request::SetStartupId {
                startup_id: Some(token),
            } => state.completed.push(token),
            _ => panic!(
                "completion must not create a surface, activate a window or notify a new launch"
            ),
        }
    }
}

fn run(advertise_shell: bool) -> Vec<String> {
    let (client_socket, server_socket) = UnixStream::pair().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let done = stop.clone();
    let server = std::thread::spawn(move || {
        let mut display = Display::<Server>::new().unwrap();
        let mut handle = display.handle();
        let _client = handle.insert_client(server_socket, Arc::new(())).unwrap();
        if advertise_shell {
            handle.create_global::<Server, gtk_shell1::GtkShell1, _>(1, ());
        }
        let mut state = Server::default();
        while !done.load(Ordering::SeqCst) {
            display.dispatch_clients(&mut state).unwrap();
            display.flush_clients().unwrap();
            std::thread::sleep(Duration::from_millis(1));
        }
        state.completed
    });
    let result = complete_on_connection(
        Connection::from_socket(client_socket).unwrap(),
        "quit-token".into(),
    );
    stop.store(true, Ordering::SeqCst);
    let completed = server.join().unwrap();
    result.unwrap();
    completed
}

#[test]
fn completes_startup_without_a_window() {
    assert_eq!(run(true), ["quit-token"]);
}

#[test]
fn other_compositors_do_not_block_menu_actions() {
    assert!(run(false).is_empty());
}

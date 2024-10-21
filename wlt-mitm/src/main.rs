use std::{env, os::unix::net::{UnixListener, UnixStream}, path::{Path, PathBuf}};
use std::ffi::{OsString, OsStr};
use std::thread;
use std::sync::Arc;

use snafu::prelude::*;
use wayland_backend::rs::client;
use wayland_backend::rs::server;

#[derive(Debug)]
struct UnixSocketServer {
    // TODO: should this be &Path ?
    path: PathBuf,
    listener: UnixListener,
}
impl UnixSocketServer {
    fn new(path: PathBuf) -> std::io::Result<Self> {
        let listener = UnixListener::bind(&path)?;
        Ok(Self { path, listener })
    }
}
impl Drop for UnixSocketServer {
    fn drop(&mut self) {
        std::fs::remove_file(&self.path).unwrap();
    }
}

#[snafu::report]
fn main() -> Result<(), snafu::Whatever> {
    let xdg_runtime_dir = env::var_os("XDG_RUNTIME_DIR")
        .whatever_context("XDG_RUNTIME_DIR not set. I don't know where to look for sockets!")?;
    // connection to the Wayland compositor we are running beneath and proxying
    let upstream_conn = connect_server(&xdg_runtime_dir);

    // the server we are creating that is proxied to the upstream compositor; the client will connect to this server
    let backend = server::Backend::<()>::new().unwrap();
    let mut srv_sock_count = 0;
    let srv_sock = loop {
        let path: PathBuf = [&xdg_runtime_dir, &OsString::from(format!("wlt-mitm-{}", srv_sock_count))].iter().collect();
        dbg!(&path);
        match UnixSocketServer::new(path.clone()) { 
            Ok(sock) => break Some(sock),
            Err(e) => {
                if srv_sock_count > 9 {
                    eprintln!("exhausted all attempts to create a socket");
                    break None;
                }
                eprintln!("error creating {}: {:?}. Will try again with different path.", path.display(), e);
                srv_sock_count += 1;
                continue;
            }
        }
    }.whatever_context("error creating socket to listen on as a Wayland server")?;

    let unix_listener_thread_handle = backend.handle();
    thread::spawn(move || {
        let hand = unix_listener_thread_handle;
        for listener in srv_sock.listener.incoming() {
            match hand.insert_client(listener, Arc::new(())) {
                Ok(id) => eprintln!("connected to client {}", id),
                Err(e) => todo!(),
            }
        }
    });

    Ok(())
}

/// connect to the Wayland server (upstream)
fn connect_server(xdg_runtime_dir: &OsStr) -> Result<client::Backend, snafu::Whatever> {
    let server_socket_name = env::var_os("WAYLAND_DISPLAY")
        .whatever_context("WAYLAND_DISPLAY not set. Are you running under Wayland?")?;
    // shouldn't panic because server_socket_name len > 0 because otherwise it would
    // be None
    let server_socket_path = if server_socket_name.as_encoded_bytes()[0] == b'/' {
        PathBuf::from(server_socket_name)
    } else {
        [xdg_runtime_dir, &server_socket_name]
            .iter()
            .collect::<PathBuf>()
    };

    let server_unix_stream = UnixStream::connect(server_socket_path)
        .whatever_context("error connecting to Wayland server's socket")?;
    client::Backend::connect(server_unix_stream)
        .whatever_context("error connecting to Wayland server")
}

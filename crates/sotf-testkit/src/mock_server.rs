//! Lightweight mock server helpers for network-protocol tests.
//!
//! Requires the `plugin` feature (kept behind the same feature gate as the rest
//! of the non-audio test helpers).

use std::net::{SocketAddr, TcpListener};

/// Bind to a random loopback port and return the address.
pub fn random_local_addr() -> SocketAddr {
    TcpListener::bind("127.0.0.1:0")
        .expect("failed to bind")
        .local_addr()
        .expect("failed to get local address")
}

/// A TCP mock server that runs a handler on each connection.
#[derive(Debug)]
pub struct MockTcpServer {
    addr: SocketAddr,
}

impl MockTcpServer {
    /// Spawn a server on a random port; `handler` receives `(reader, writer)`.
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn(std::net::TcpStream) + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind");
        let addr = listener.local_addr().expect("failed to get local address");

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if let Ok(stream) = stream {
                    handler(stream);
                }
            }
        });

        Self { addr }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

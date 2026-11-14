//! Serves a single report from a private loopback URL.

// Rust guideline compliant 2026-02-21

use anyhow::Context;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::Duration;

/// An offline report origin whose lifetime follows the report window.
#[derive(Debug)]
pub struct ReportServer {
    url: url::Url,
    report_id: String,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl ReportServer {
    /// Start serving an immutable document on an unused loopback port.
    ///
    /// # Errors
    /// Returns an error if binding or starting the server fails.
    pub fn start(html: String) -> anyhow::Result<Self> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let path = format!("/{}/index.html", uuid::Uuid::new_v4());
        let url = url::Url::parse(&format!("http://{address}{path}"))?;
        let mut report_id = String::with_capacity(64);
        for byte in Sha256::digest(html.as_bytes()) {
            write!(report_id, "{byte:02x}")?;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = std::thread::Builder::new()
            .name("autoeq-report-host".into())
            .spawn(move || {
                while !worker_stop.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            // The worker owns no error channel back to the
                            // report window; a dropped connection with no
                            // diagnostic is a blank viewer and an
                            // undebuggable test failure. These paths only
                            // fire on client misbehavior or timeouts.
                            if let Err(error) = serve(stream, &path, html.as_bytes()) {
                                eprintln!(
                                    "sotf-report-host: closing connection after serve error: {error}"
                                );
                            }
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Err(error) => {
                            eprintln!("sotf-report-host: accept loop stopping: {error}");
                            break;
                        }
                    }
                }
            })
            .context("Starting report host")?;
        Ok(Self {
            url,
            report_id,
            stop,
            worker: Some(worker),
        })
    }

    /// The only navigable document URL.
    #[must_use]
    pub fn url(&self) -> &str {
        self.url.as_str()
    }

    /// Immutable content identity used to bind viewer interactions.
    #[must_use]
    pub fn report_id(&self) -> &str {
        &self.report_id
    }

    /// Whether a navigation stays on this report's document.
    #[must_use]
    pub fn permits_navigation(&self, candidate: &str) -> bool {
        let Ok(mut candidate) = url::Url::parse(candidate) else {
            return false;
        };
        candidate.set_fragment(None);
        candidate == self.url
    }
}

impl Drop for ReportServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn serve(mut stream: TcpStream, path: &str, html: &[u8]) -> std::io::Result<()> {
    // Accepted sockets inherit the listener's nonblocking mode; the first
    // read would then race the client's request bytes (EAGAIN) and drop the
    // connection. Blocking plus timeouts keeps the slow-client bound below.
    stream.set_nonblocking(false)?;
    // Bound a slow client so closing a window cannot leave a blocked worker.
    stream.set_read_timeout(Some(Duration::from_millis(250)))?;
    stream.set_write_timeout(Some(Duration::from_millis(250)))?;
    let mut request = Vec::new();
    let mut block = [0u8; 1024];
    while request.len() < 8192 && !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
        let read = stream.read(&mut block)?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&block[..read]);
    }
    let expected = format!("GET {path} HTTP/1.");
    let allowed = std::str::from_utf8(&request).is_ok_and(|request| request.starts_with(&expected));
    let (status, body) = if allowed {
        ("200 OK", html)
    } else {
        ("404 Not Found", &b"Not found"[..])
    };
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_is_offline_scoped_and_stops_with_owner() {
        let report = ReportServer::start("<html>shared WASM report</html>".into()).unwrap();
        let url = report.url().to_owned();
        let client = reqwest::blocking::Client::new();
        assert_eq!(
            client.get(&url).send().unwrap().text().unwrap(),
            "<html>shared WASM report</html>"
        );
        let origin = url::Url::parse(&url).unwrap().join("/etc/passwd").unwrap();
        assert_eq!(client.get(origin).send().unwrap().status(), 404);
        assert!(report.permits_navigation(&format!("{url}#acoustics")));
        assert!(!report.permits_navigation("https://example.com"));
        let address = url::Url::parse(&url).unwrap();
        drop(report);
        assert!(TcpStream::connect(("127.0.0.1", address.port().unwrap())).is_err());
    }

    #[test]
    fn slow_arriving_request_is_still_served() {
        use std::io::Write as _;
        let report = ReportServer::start("<html>slow</html>".into()).unwrap();
        let url = url::Url::parse(report.url()).unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", url.port().unwrap())).unwrap();
        // Request bytes arrive after the accept (50ms: past the 10ms accept
        // poll, well inside the 250ms slow-client bound). The server must
        // wait for them instead of dropping the connection with EAGAIN.
        std::thread::sleep(Duration::from_millis(50));
        write!(
            stream,
            "GET {} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
            url.path()
        )
        .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        let response = String::from_utf8(response).unwrap();
        assert!(
            response.starts_with("HTTP/1.1 200 OK"),
            "unexpected response: {response}"
        );
        assert!(response.ends_with("<html>slow</html>"));
    }
}

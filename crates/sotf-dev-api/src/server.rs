use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::json;

use crate::auth::{RUN_ID_HEADER, RunId};
use crate::http::{HttpError, HttpRequest, HttpResponse, Method, read_request};
use crate::protocol::{
    Capabilities, DevReply, ProtocolLimits, QueueMetadata, ReplyMetadata, TimingMetadata,
};
use crate::queue::{QueueError, bounded_channel};

pub trait TargetDispatcher: Send + Sync + 'static {
    fn dispatch(&self, request: HttpRequest, context: DispatchContext) -> HttpResponse;
}

impl<F> TargetDispatcher for F
where
    F: Fn(HttpRequest, DispatchContext) -> HttpResponse + Send + Sync + 'static,
{
    fn dispatch(&self, request: HttpRequest, context: DispatchContext) -> HttpResponse {
        self(request, context)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchContext {
    pub command_sequence: u64,
    pub queue: QueueMetadata,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind: SocketAddr,
    pub run_id: RunId,
    pub capabilities: Capabilities,
    pub parser_workers: usize,
}

impl ServerConfig {
    pub fn loopback(run_id: RunId, capabilities: Capabilities) -> Self {
        Self {
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            run_id,
            capabilities,
            parser_workers: 4,
        }
    }
}

pub struct ServerHandle {
    endpoint: SocketAddr,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl ServerHandle {
    pub fn endpoint(&self) -> SocketAddr {
        self.endpoint
    }

    pub fn shutdown(mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect_timeout(&self.endpoint, Duration::from_millis(50));
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect_timeout(&self.endpoint, Duration::from_millis(50));
    }
}

struct RoutedConnection {
    stream: TcpStream,
    request: HttpRequest,
    accepted: Instant,
    command_sequence: u64,
}

pub fn start_server(
    config: ServerConfig,
    dispatcher: impl TargetDispatcher,
) -> Result<ServerHandle, io::Error> {
    if !config.bind.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "dev API may bind only to loopback",
        ));
    }
    let limits = config.capabilities.limits.clone();
    let listener = TcpListener::bind(config.bind)?;
    let endpoint = listener.local_addr()?;
    listener.set_nonblocking(true)?;
    let stop = Arc::new(AtomicBool::new(false));
    let capabilities = Arc::new(config.capabilities);
    let dispatcher: Arc<dyn TargetDispatcher> = Arc::new(dispatcher);
    let sequence = Arc::new(AtomicU64::new(0));
    let (route_sender, route_receiver) =
        bounded_channel::<RoutedConnection>(limits.command_queue.max(1));
    let mut threads = Vec::new();

    let dispatch_stop = stop.clone();
    let dispatch_limits = limits.clone();
    let dispatch_telemetry = route_sender.telemetry();
    threads.push(
        thread::Builder::new()
            .name("sotf-dev-dispatch".into())
            .spawn(move || {
                while !dispatch_stop.load(Ordering::Acquire) {
                    let routed = match route_receiver.recv_timeout(Duration::from_millis(25)) {
                        Ok(routed) => routed,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    let mut routed: RoutedConnection = routed;
                    let dispatch_started = Instant::now();
                    let queue = dispatch_telemetry.snapshot();
                    let response = dispatcher.dispatch(
                        routed.request,
                        DispatchContext {
                            command_sequence: routed.command_sequence,
                            queue: queue.clone(),
                        },
                    );
                    let response = enrich_response(
                        response,
                        routed.command_sequence,
                        routed.accepted,
                        dispatch_started,
                        queue,
                    );
                    let _ = response.write_to(&mut routed.stream, dispatch_limits.response_bytes);
                }
            })?,
    );

    let worker_count = config
        .parser_workers
        .clamp(1, limits.active_connections.max(1));
    let mut worker_senders = Vec::with_capacity(worker_count);
    for index in 0..worker_count {
        let (sender, receiver) = mpsc::sync_channel::<TcpStream>(
            limits.active_connections.div_ceil(worker_count).max(1),
        );
        worker_senders.push(sender);
        let worker_stop = stop.clone();
        let worker_limits = limits.clone();
        let worker_run_id = config.run_id.clone();
        let worker_caps = capabilities.clone();
        let worker_routes = route_sender.clone();
        let worker_sequence = sequence.clone();
        threads.push(
            thread::Builder::new()
                .name(format!("sotf-dev-parse-{index}"))
                .spawn(move || {
                    while !worker_stop.load(Ordering::Acquire) {
                        let mut stream = match receiver.recv_timeout(Duration::from_millis(25)) {
                            Ok(stream) => stream,
                            Err(mpsc::RecvTimeoutError::Timeout) => continue,
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        };
                        let accepted = Instant::now();
                        configure_stream(&stream, &worker_limits);
                        let request = match read_request(&mut stream, &worker_limits) {
                            Ok(request) => request,
                            Err(error) => {
                                let response = parser_error_response(&error);
                                let _ =
                                    response.write_to(&mut stream, worker_limits.response_bytes);
                                continue;
                            }
                        };
                        if !authenticated(&request, &worker_run_id) {
                            let response = json_reply(
                                401,
                                DevReply::failure("unauthorized", "invalid or missing run ID"),
                            );
                            let _ = response.write_to(&mut stream, worker_limits.response_bytes);
                            continue;
                        }
                        let command_sequence = worker_sequence.fetch_add(1, Ordering::Relaxed) + 1;
                        if request.method == Method::Get && request.path == "/live" {
                            let mut reply = DevReply::success(json!({
                                "live": true,
                                "protocol_version": 2,
                                "target_id": worker_caps.target_id,
                                "process_name": worker_caps.process_name,
                            }));
                            reply.meta.command_sequence = command_sequence;
                            reply.meta.timing.total_ns = elapsed_ns(accepted);
                            let response = json_reply(200, reply);
                            let _ = response.write_to(&mut stream, worker_limits.response_bytes);
                            continue;
                        }
                        if request.method == Method::Get && request.path == "/capabilities" {
                            let response = match serde_json::to_value(worker_caps.as_ref()) {
                                Ok(value) => {
                                    let mut reply = DevReply::success(value);
                                    reply.meta.command_sequence = command_sequence;
                                    reply.meta.timing.total_ns = elapsed_ns(accepted);
                                    json_reply(200, reply)
                                }
                                Err(error) => json_reply(
                                    500,
                                    DevReply::failure("serialization", error.to_string()),
                                ),
                            };
                            let _ = response.write_to(&mut stream, worker_limits.response_bytes);
                            continue;
                        }
                        let mut overload_stream = stream.try_clone().ok();
                        match worker_routes.try_send(RoutedConnection {
                            stream,
                            request,
                            accepted,
                            command_sequence,
                        }) {
                            Ok(()) => {}
                            Err(QueueError::Full) => {
                                if let Some(stream) = overload_stream.as_mut() {
                                    let _ = json_reply(
                                        429,
                                        DevReply::failure(
                                            "queue_full",
                                            "target command queue is full",
                                        ),
                                    )
                                    .write_to(stream, worker_limits.response_bytes);
                                }
                            }
                            Err(QueueError::Disconnected) => break,
                        }
                    }
                })?,
        );
    }

    let accept_stop = stop.clone();
    threads.push(
        thread::Builder::new()
            .name("sotf-dev-accept".into())
            .spawn(move || {
                let mut next_worker = 0usize;
                while !accept_stop.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((mut stream, peer)) => {
                            if !peer.ip().is_loopback() {
                                let _ = HttpResponse::text(401, "loopback only")
                                    .write_to(&mut stream, limits.response_bytes);
                                continue;
                            }
                            let sender = &worker_senders[next_worker % worker_senders.len()];
                            next_worker = next_worker.wrapping_add(1);
                            match sender.try_send(stream) {
                                Ok(()) => {}
                                Err(mpsc::TrySendError::Full(mut stream)) => {
                                    let _ = json_reply(
                                        429,
                                        DevReply::failure(
                                            "connection_limit",
                                            "connection limit reached",
                                        ),
                                    )
                                    .write_to(&mut stream, limits.response_bytes);
                                }
                                Err(mpsc::TrySendError::Disconnected(_)) => break,
                            }
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(_) => break,
                    }
                }
            })?,
    );

    Ok(ServerHandle {
        endpoint,
        stop,
        threads,
    })
}

fn configure_stream(stream: &TcpStream, limits: &ProtocolLimits) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(limits.read_timeout_ms)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(limits.write_timeout_ms)));
}

fn authenticated(request: &HttpRequest, run_id: &RunId) -> bool {
    request
        .header(RUN_ID_HEADER)
        .is_some_and(|candidate| run_id.authenticate(candidate))
}

fn json_reply(status: u16, reply: DevReply) -> HttpResponse {
    match serde_json::to_vec(&reply) {
        Ok(body) => HttpResponse::json(status, body),
        Err(error) => HttpResponse::text(500, error.to_string()),
    }
}

fn enrich_response(
    response: HttpResponse,
    command_sequence: u64,
    accepted: Instant,
    dispatch_started: Instant,
    queue: QueueMetadata,
) -> HttpResponse {
    if response.content_type != "application/json" {
        return response;
    }
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(&response.body) else {
        return response;
    };
    let Some(object) = value.as_object_mut() else {
        return response;
    };
    let total_ns = elapsed_ns(accepted);
    let queue_ns = dispatch_started
        .saturating_duration_since(accepted)
        .as_nanos()
        .min(u128::from(u64::MAX)) as u64;
    let dispatch_ns = total_ns.saturating_sub(queue_ns);
    let default_meta = ReplyMetadata {
        command_sequence,
        timing: TimingMetadata {
            accepted_ns: 0,
            queue_ns,
            dispatch_ns,
            total_ns,
        },
        queue,
        ..ReplyMetadata::default()
    };
    let mut meta = object
        .get("meta")
        .and_then(|meta| serde_json::from_value(meta.clone()).ok())
        .unwrap_or(default_meta);
    meta.command_sequence = command_sequence;
    meta.timing.queue_ns = queue_ns;
    meta.timing.dispatch_ns = dispatch_ns;
    meta.timing.total_ns = total_ns;
    object.insert(
        "meta".into(),
        serde_json::to_value(meta).unwrap_or(serde_json::Value::Null),
    );
    match serde_json::to_vec(&value) {
        Ok(body) => HttpResponse::json(response.status, body),
        Err(_) => response,
    }
}

fn elapsed_ns(started: Instant) -> u64 {
    started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

fn parser_error_response(error: &HttpError) -> HttpResponse {
    let status = match error {
        HttpError::BodyTooLarge { .. } => 413,
        HttpError::Io(io)
            if matches!(
                io.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            ) =>
        {
            408
        }
        _ => 400,
    };
    json_reply(
        status,
        DevReply::failure("invalid_request", error.to_string()),
    )
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    use super::*;

    fn get(endpoint: SocketAddr, path: &str, run_id: &str) -> String {
        raw_get(endpoint, path, Some(run_id))
    }

    fn raw_get(endpoint: SocketAddr, path: &str, run_id: Option<&str>) -> String {
        let mut stream = TcpStream::connect(endpoint).unwrap();
        let auth = run_id
            .map(|run_id| format!("X-SOTF-Dev-Run-ID: {run_id}\r\n"))
            .unwrap_or_default();
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: localhost\r\n{auth}\r\n"
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    fn response_json(response: &str) -> serde_json::Value {
        let (_, body) = response.split_once("\r\n\r\n").unwrap();
        serde_json::from_str(body).unwrap()
    }

    #[test]
    fn refuses_non_loopback_bind() {
        let run_id = RunId::parse("0123456789abcdef0123456789abcdef").unwrap();
        let mut config = ServerConfig::loopback(run_id, Capabilities::new("test", "test"));
        config.bind = "0.0.0.0:0".parse().unwrap();
        assert!(start_server(config, |_, _| HttpResponse::text(200, "ok")).is_err());
    }

    #[test]
    fn live_remains_available_while_target_dispatch_is_blocked() {
        let run_id = "0123456789abcdef0123456789abcdef";
        let entered = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let entered_dispatch = entered.clone();
        let release_dispatch = release.clone();
        let server = start_server(
            ServerConfig::loopback(
                RunId::parse(run_id).unwrap(),
                Capabilities::new("contract", "contract-target"),
            ),
            move |_, _| {
                entered_dispatch.store(true, Ordering::Release);
                while !release_dispatch.load(Ordering::Acquire) {
                    thread::sleep(Duration::from_millis(1));
                }
                HttpResponse::text(200, "done")
            },
        )
        .unwrap();

        let endpoint = server.endpoint();
        let routed = thread::spawn(move || get(endpoint, "/snapshot", run_id));
        let deadline = Instant::now() + Duration::from_secs(1);
        while !entered.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        assert!(entered.load(Ordering::Acquire));
        let response = get(server.endpoint(), "/live", run_id);
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        assert!(response.contains("\"live\":true"), "{response}");
        release.store(true, Ordering::Release);
        assert!(routed.join().unwrap().starts_with("HTTP/1.1 200"));
        server.shutdown();
    }

    #[test]
    fn shared_contract_authenticates_and_preserves_monotonic_metadata() {
        let run_id = "0123456789abcdef0123456789abcdef";
        let revision = Arc::new(AtomicU64::new(10));
        let dispatch_revision = revision.clone();
        let server = start_server(
            ServerConfig::loopback(
                RunId::parse(run_id).unwrap(),
                Capabilities::new("contract", "contract-target"),
            ),
            move |request: HttpRequest, _context: DispatchContext| {
                let before = dispatch_revision.fetch_add(1, Ordering::AcqRel);
                let mut reply = DevReply::success(serde_json::json!({
                    "path": request.path,
                }));
                reply.meta.state_revision_before = before;
                reply.meta.state_revision_after = before + 1;
                HttpResponse::json(200, serde_json::to_vec(&reply).unwrap())
            },
        )
        .unwrap();

        let missing = raw_get(server.endpoint(), "/snapshot", None);
        assert!(missing.starts_with("HTTP/1.1 401"), "{missing}");
        let wrong = get(
            server.endpoint(),
            "/snapshot",
            "fedcba9876543210fedcba9876543210",
        );
        assert!(wrong.starts_with("HTTP/1.1 401"), "{wrong}");

        let capabilities = get(server.endpoint(), "/capabilities", run_id);
        assert!(capabilities.starts_with("HTTP/1.1 200"), "{capabilities}");
        assert_eq!(
            response_json(&capabilities)["value"]["target_id"],
            "contract"
        );

        let first = response_json(&get(server.endpoint(), "/snapshot", run_id));
        let second = response_json(&get(server.endpoint(), "/snapshot", run_id));
        assert!(
            second["meta"]["command_sequence"].as_u64().unwrap()
                > first["meta"]["command_sequence"].as_u64().unwrap()
        );
        assert_eq!(first["meta"]["state_revision_before"], 10);
        assert_eq!(first["meta"]["state_revision_after"], 11);
        assert_eq!(second["meta"]["state_revision_before"], 11);
        assert_eq!(second["meta"]["state_revision_after"], 12);

        let quit = get(server.endpoint(), "/quit", run_id);
        assert!(quit.starts_with("HTTP/1.1 200"), "{quit}");
        server.shutdown();
    }
}

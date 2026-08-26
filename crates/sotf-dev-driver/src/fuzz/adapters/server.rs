use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sotf_dev_api::{Capabilities, DevReply, NamedCapability, Snapshot};

use crate::fuzz::model::{
    Action, ActionPayload, AdapterKind, CoverageDelta, EndpointSpec, FUZZ_SCHEMA_VERSION,
    Observation, ProcessObservation, TargetId, TargetSpec,
};
use crate::fuzz::supervisor::{FuzzTarget, LaunchContext, TargetError};

const LOOPBACK: &str = "127.0.0.1";
const API_TOKEN: &str = "sotf-fuzzer-loopback-token";
const MPD_PASSWORD: &str = "sotf-fuzzer";
const MAX_BODY_BYTES: usize = 64 * 1024;
const MAX_RESPONSE_BYTES: usize = 512 * 1024;
const MAX_HEADER_COUNT: usize = 64;
const MAX_HEADER_BYTES: usize = 32 * 1024;
const MAX_METHOD_BYTES: usize = 32;
const MAX_PATH_BYTES: usize = 4 * 1024;
const MAX_LOG_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone)]
pub struct ServerTargetConfig {
    pub executable: PathBuf,
    pub environment: BTreeMap<String, String>,
    pub startup_timeout: Duration,
}

impl ServerTargetConfig {
    pub fn new(executable: PathBuf) -> Self {
        Self {
            executable,
            environment: BTreeMap::new(),
            startup_timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ServerEndpoints {
    mpd: u16,
    api: u16,
    dlna: u16,
}

impl ServerEndpoints {
    fn port(self, endpoint: &str) -> Result<u16, TargetError> {
        match endpoint {
            "mpd" => Ok(self.mpd),
            "sotf-api" => Ok(self.api),
            "dlna" => Ok(self.dlna),
            _ => Err(TargetError::Protocol(format!(
                "unknown server endpoint {endpoint:?}"
            ))),
        }
    }
}

#[derive(Debug)]
struct RawResponse {
    status: Option<u16>,
    bytes: Vec<u8>,
}

pub struct ServerTarget {
    config: ServerTargetConfig,
    run_dir: Option<PathBuf>,
    endpoints: Option<ServerEndpoints>,
    child: Option<Child>,
    stdout_path: Option<PathBuf>,
    stderr_path: Option<PathBuf>,
    stdout_offset: u64,
    stderr_offset: u64,
    revision: u64,
    request_count: u64,
    last_endpoint: Option<String>,
    last_status: Option<u16>,
    last_process: ProcessObservation,
}

impl ServerTarget {
    pub fn new(config: ServerTargetConfig) -> Self {
        Self {
            config,
            run_dir: None,
            endpoints: None,
            child: None,
            stdout_path: None,
            stderr_path: None,
            stdout_offset: 0,
            stderr_offset: 0,
            revision: 0,
            request_count: 0,
            last_endpoint: None,
            last_status: None,
            last_process: ProcessObservation {
                pid: None,
                alive: false,
                exit_code: None,
                signal_or_exception: None,
            },
        }
    }

    fn endpoints(&self) -> Result<ServerEndpoints, TargetError> {
        self.endpoints
            .ok_or_else(|| TargetError::Protocol("server target is not launched".into()))
    }

    fn write_config(&self, qa_dir: &Path, endpoints: ServerEndpoints) -> Result<(), TargetError> {
        let config = json!({
            "mpd": {
                "enabled": true,
                "bind_address": LOOPBACK,
                "port": endpoints.mpd,
                "tls_enabled": false,
                "auth_mode": "Password",
                "password": MPD_PASSWORD,
                "trusted_client_fingerprints": []
            },
            "dlna": {
                "enabled": true,
                "bind_address": LOOPBACK,
                "friendly_name": "SOTF Hermetic Fuzzer",
                "port": endpoints.dlna
            },
            "api": {
                "enabled": true,
                "bind_address": LOOPBACK,
                "port": endpoints.api,
                "friendly_name": "SOTF Hermetic Fuzzer",
                "tls_enabled": false,
                "auth_token": API_TOKEN
            }
        });
        fs::write(
            qa_dir.join("servers.json"),
            serde_json::to_vec_pretty(&config)?,
        )?;
        Ok(())
    }

    fn wait_until_ready(&mut self) -> Result<(), TargetError> {
        let deadline = Instant::now() + self.config.startup_timeout;
        let mut last_error = "listeners have not accepted a connection".to_owned();
        while Instant::now() < deadline {
            if let Some(status) = self
                .child
                .as_mut()
                .and_then(|child| child.try_wait().ok())
                .flatten()
            {
                return Err(TargetError::ProcessExited(format!(
                    "headless server exited during startup with {status}"
                )));
            }

            let timeout = Duration::from_millis(500);
            let api = self.request_http(
                "sotf-api",
                "GET",
                "/api/v1/health",
                &BTreeMap::new(),
                &[],
                timeout,
            );
            let dlna = self.request_http(
                "dlna",
                "GET",
                "/description.xml",
                &BTreeMap::new(),
                &[],
                timeout,
            );
            let mpd = self.request_mpd(b"ping\n", timeout);
            match (api, dlna, mpd) {
                (Ok(api), Ok(dlna), Ok(mpd))
                    if api.status == Some(200)
                        && dlna.status == Some(200)
                        && !mpd.bytes.is_empty() =>
                {
                    return Ok(());
                }
                (api, dlna, mpd) => {
                    last_error = format!(
                        "api={}, dlna={}, mpd={}",
                        probe_result(&api),
                        probe_result(&dlna),
                        probe_result(&mpd)
                    );
                }
            }
            thread::sleep(Duration::from_millis(50));
        }
        Err(TargetError::Timeout(format!(
            "headless server did not become ready: {last_error}"
        )))
    }

    fn request_mpd(&self, body: &[u8], timeout: Duration) -> Result<RawResponse, TargetError> {
        if body.len() > MAX_BODY_BYTES {
            return Err(TargetError::Protocol(format!(
                "MPD request body is {} bytes; limit is {MAX_BODY_BYTES}",
                body.len()
            )));
        }
        let mut stream = connect_loopback(self.endpoints()?.mpd, timeout)?;
        let greeting = read_line_bounded(&mut stream, 1024)?;
        stream.write_all(body)?;
        let _ = stream.shutdown(Shutdown::Write);
        let mut bytes = greeting;
        read_to_end_bounded(&mut stream, &mut bytes)?;
        Ok(RawResponse {
            status: None,
            bytes,
        })
    }

    fn request_http(
        &self,
        endpoint: &str,
        method: &str,
        path: &str,
        headers: &BTreeMap<String, String>,
        body: &[u8],
        timeout: Duration,
    ) -> Result<RawResponse, TargetError> {
        validate_http_request(method, path, headers, body)?;
        let port = self.endpoints()?.port(endpoint)?;
        if endpoint == "mpd" {
            if method != "RAW" {
                return Err(TargetError::Protocol(
                    "MPD endpoint requires method RAW".into(),
                ));
            }
            return self.request_mpd(body, timeout);
        }

        let mut request = Vec::with_capacity(256 + body.len());
        write!(&mut request, "{method} {path} HTTP/1.1\r\n")?;
        write!(&mut request, "Host: {LOOPBACK}:{port}\r\n")?;
        request.extend_from_slice(b"Connection: close\r\n");
        let has_authorization = headers
            .keys()
            .any(|name| name.eq_ignore_ascii_case("authorization"));
        if endpoint == "sotf-api" && !has_authorization {
            write!(&mut request, "Authorization: Bearer {API_TOKEN}\r\n")?;
        }
        for (name, value) in headers {
            write!(&mut request, "{name}: {value}\r\n")?;
        }
        write!(&mut request, "Content-Length: {}\r\n\r\n", body.len())?;
        request.extend_from_slice(body);

        let mut stream = connect_loopback(port, timeout)?;
        stream.write_all(&request)?;
        let _ = stream.shutdown(Shutdown::Write);
        let mut bytes = Vec::new();
        read_to_end_bounded(&mut stream, &mut bytes)?;
        let status = parse_http_status(&bytes);
        Ok(RawResponse { status, bytes })
    }

    fn process_observation(&mut self) -> ProcessObservation {
        let observation = match self.child.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(status)) => process_from_status(child.id(), &status),
                Ok(None) => ProcessObservation {
                    pid: Some(child.id()),
                    alive: true,
                    exit_code: None,
                    signal_or_exception: None,
                },
                Err(error) => ProcessObservation {
                    pid: Some(child.id()),
                    alive: false,
                    exit_code: None,
                    signal_or_exception: Some(error.to_string()),
                },
            },
            None => self.last_process.clone(),
        };
        self.last_process = observation.clone();
        observation
    }

    fn read_new_logs(&mut self) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(path) = self.stdout_path.clone() {
            lines.extend(read_appended(&path, &mut self.stdout_offset));
        }
        if let Some(path) = self.stderr_path.clone() {
            lines.extend(read_appended(&path, &mut self.stderr_offset));
        }
        const MAX_LINES: usize = 2_000;
        if lines.len() > MAX_LINES {
            lines.drain(..lines.len() - MAX_LINES);
        }
        lines
    }

    fn crash_files(&self) -> Vec<PathBuf> {
        self.run_dir
            .as_ref()
            .map(|run_dir| run_dir.join("qa/sotf_crash.log"))
            .filter(|path| path.is_file())
            .into_iter()
            .collect()
    }
}

impl FuzzTarget for ServerTarget {
    fn target_id(&self) -> TargetId {
        TargetId::HeadlessServer
    }

    fn launch(&mut self, context: &LaunchContext<'_>) -> Result<TargetSpec, TargetError> {
        if !self.config.executable.is_file() {
            return Err(TargetError::Unsupported(
                crate::fuzz::model::StructuredSkip {
                    target_id: TargetId::HeadlessServer,
                    reason_code: "runtime_missing".into(),
                    reason: format!(
                        "executable {} does not exist; build sotf-desktop first",
                        self.config.executable.display()
                    ),
                },
            ));
        }
        self.config.executable = fs::canonicalize(&self.config.executable)?;
        let qa_dir = context.run_dir.join("qa");
        let log_dir = context.run_dir.join("logs/server");
        fs::create_dir_all(&qa_dir)?;
        fs::create_dir_all(&log_dir)?;
        fs::create_dir_all(context.run_dir.join("tmp"))?;

        let endpoints = ServerEndpoints {
            mpd: free_loopback_port()?,
            api: free_loopback_port()?,
            dlna: free_loopback_port()?,
        };
        self.endpoints = Some(endpoints);
        self.run_dir = Some(context.run_dir.to_owned());
        self.write_config(&qa_dir, endpoints)?;

        let stdout_path = log_dir.join("stdout.log");
        let stderr_path = log_dir.join("stderr.log");
        let stdout = File::create(&stdout_path)?;
        let stderr = File::create(&stderr_path)?;
        let mut command = Command::new(&self.config.executable);
        command.env_clear();
        for name in ["PATH", "RUST_BACKTRACE", "RUST_LOG", "LANG", "LC_ALL"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command
            .env("SOTF_SERVER_DISABLE_DISCOVERY", "1")
            .env("SOTF_QA_DIR", &qa_dir)
            .env("TMPDIR", context.run_dir.join("tmp"))
            .arg("--server")
            .arg("--qa")
            .arg(&qa_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        for (name, value) in &self.config.environment {
            command.env(name, value);
        }
        let child = command.spawn()?;
        self.last_process = ProcessObservation {
            pid: Some(child.id()),
            alive: true,
            exit_code: None,
            signal_or_exception: None,
        };
        self.child = Some(child);
        self.stdout_path = Some(stdout_path);
        self.stderr_path = Some(stderr_path);
        self.wait_until_ready()?;

        Ok(TargetSpec {
            schema_version: FUZZ_SCHEMA_VERSION,
            target_id: TargetId::HeadlessServer,
            adapter: AdapterKind::Server,
            executable: Some(self.config.executable.clone()),
            app_identity: Some("sotf-desktop-server".into()),
            platform: std::env::consts::OS.into(),
            fixture_profile: context.fixture_profile.into(),
            environment_names: self
                .config
                .environment
                .keys()
                .cloned()
                .chain(["SOTF_SERVER_DISABLE_DISCOVERY".into()])
                .collect(),
            endpoints: vec![
                endpoint_spec("mpd", endpoints.mpd, "mpd"),
                endpoint_spec("sotf-api", endpoints.api, "http"),
                endpoint_spec("dlna", endpoints.dlna, "http-soap"),
            ],
            run_id_hash: context.run_id.redacted_hash(),
            capability_fingerprint: String::new(),
            build_id: "development".into(),
        })
    }

    fn capabilities(&mut self) -> Result<Capabilities, TargetError> {
        let mut capabilities = Capabilities::new("headless-server", "sotf-desktop-server");
        capabilities.debug_features = vec![
            "hermetic-loopback".into(),
            "discovery-disabled".into(),
            "bounded-raw-protocols".into(),
        ];
        capabilities.actions = ["mpd", "sotf-api", "dlna-soap"]
            .into_iter()
            .map(|name| NamedCapability {
                name: name.into(),
                family: "server-protocol".into(),
                payload_schema: None,
            })
            .collect();
        capabilities.unsupported.insert(
            "network-discovery".into(),
            "SSDP and mDNS are disabled for hermetic runs".into(),
        );
        capabilities.unsupported.insert(
            "external-services".into(),
            "network accounts and external plugins require explicit opt-ins".into(),
        );
        Ok(capabilities)
    }

    fn snapshot(&mut self) -> Result<Snapshot, TargetError> {
        let process = self.process_observation();
        if !process.alive {
            return Err(TargetError::ProcessExited(format!(
                "headless server exited: code={:?}, signal={:?}",
                process.exit_code, process.signal_or_exception
            )));
        }
        let response = self.request_http(
            "sotf-api",
            "GET",
            "/api/v1/state",
            &BTreeMap::new(),
            &[],
            Duration::from_secs(2),
        )?;
        if response.status != Some(200) {
            return Err(TargetError::Protocol(format!(
                "SOTF state probe returned HTTP {:?}",
                response.status
            )));
        }
        let api_state = response_json_body(&response.bytes).unwrap_or(Value::Null);
        let endpoints = self.endpoints()?;
        let mut snapshot = Snapshot::new(
            "headless-server",
            self.revision,
            json!({
                "process_phase": "running",
                "pid": process.pid,
                "requests": self.request_count,
                "last_endpoint": self.last_endpoint,
                "last_status": self.last_status,
                "listeners": {
                    "mpd": endpoints.mpd,
                    "sotf_api": endpoints.api,
                    "dlna": endpoints.dlna,
                },
                "api": api_state,
                "discovery_enabled": false,
            }),
        )?;
        snapshot.screen = Some("headless-server".into());
        snapshot.mode = Some("loopback-hermetic".into());
        Ok(snapshot)
    }

    fn execute(&mut self, action: &Action) -> Result<Observation, TargetError> {
        let reply = match &action.payload {
            ActionPayload::Http {
                endpoint,
                method,
                path,
                headers,
                body,
            } => {
                let timeout = Duration::from_millis(action.timeout_ms.clamp(1, 30_000));
                let response = self.request_http(endpoint, method, path, headers, body, timeout)?;
                self.request_count = self.request_count.saturating_add(1);
                self.last_endpoint = Some(endpoint.clone());
                self.last_status = response.status;
                let body_preview = response_body_preview(&response.bytes);
                DevReply::success(json!({
                    "endpoint": endpoint,
                    "status": response.status,
                    "response_bytes": response.bytes.len(),
                    "body_preview": body_preview,
                }))
            }
            ActionPayload::Wait { duration_ms } => {
                thread::sleep(Duration::from_millis((*duration_ms).min(action.timeout_ms)));
                DevReply::success(Value::Null)
            }
            other => {
                return Err(TargetError::Protocol(format!(
                    "payload {other:?} is not supported by server adapter"
                )));
            }
        };
        self.revision = self.revision.saturating_add(1);
        let snapshot = self.snapshot()?;
        let process = self.process_observation();
        let status_key = self
            .last_status
            .map_or_else(|| "raw".into(), |status| status.to_string());
        let endpoint_key = self.last_endpoint.as_deref().unwrap_or("wait").to_owned();
        Ok(Observation {
            schema_version: FUZZ_SCHEMA_VERSION,
            sequence: action.sequence,
            reply: Some(reply),
            snapshot: Some(snapshot),
            process,
            resource: None,
            new_logs: self.read_new_logs(),
            crash_files: self.crash_files(),
            coverage: CoverageDelta {
                new_keys: vec![format!("server:{endpoint_key}:status:{status_key}")],
                counters: BTreeMap::new(),
            },
            screenshot: None,
            failure_candidate: None,
        })
    }

    fn live(&mut self) -> Result<bool, TargetError> {
        if !self.process_observation().alive {
            return Ok(false);
        }
        Ok(self
            .request_http(
                "sotf-api",
                "GET",
                "/api/v1/health",
                &BTreeMap::new(),
                &[],
                Duration::from_secs(2),
            )
            .is_ok_and(|response| response.status == Some(200)))
    }

    fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    fn capture_hang(&mut self, directory: &Path) -> Result<Vec<PathBuf>, TargetError> {
        fs::create_dir_all(directory)?;
        let Some(pid) = self.pid() else {
            return Ok(vec![]);
        };
        let output_path = directory.join("threads.txt");
        let output = capture_threads(pid)?;
        fs::write(&output_path, output)?;
        Ok(vec![output_path])
    }

    fn shutdown(&mut self) -> Result<(), TargetError> {
        let Some(mut child) = self.child.take() else {
            return Ok(());
        };
        let pid = child.id();
        if child.try_wait()?.is_none() {
            send_interrupt(pid)?;
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if let Some(status) = child.try_wait()? {
                    self.last_process = process_from_status(pid, &status);
                    return Ok(());
                }
                thread::sleep(Duration::from_millis(25));
            }
            child.kill()?;
        }
        let status = child.wait()?;
        self.last_process = process_from_status(pid, &status);
        Ok(())
    }
}

fn validate_http_request(
    method: &str,
    path: &str,
    headers: &BTreeMap<String, String>,
    body: &[u8],
) -> Result<(), TargetError> {
    if method.is_empty()
        || method.len() > MAX_METHOD_BYTES
        || !method.bytes().all(|byte| byte.is_ascii_uppercase())
    {
        return Err(TargetError::Protocol("invalid bounded HTTP method".into()));
    }
    if !path.starts_with('/')
        || path.len() > MAX_PATH_BYTES
        || path
            .bytes()
            .any(|byte| matches!(byte, b'\r' | b'\n' | b' '))
    {
        return Err(TargetError::Protocol("invalid bounded HTTP path".into()));
    }
    if body.len() > MAX_BODY_BYTES {
        return Err(TargetError::Protocol(format!(
            "HTTP body is {} bytes; limit is {MAX_BODY_BYTES}",
            body.len()
        )));
    }
    if headers.len() > MAX_HEADER_COUNT {
        return Err(TargetError::Protocol(format!(
            "HTTP request has {} headers; limit is {MAX_HEADER_COUNT}",
            headers.len()
        )));
    }
    let header_bytes = headers.iter().try_fold(0usize, |total, (name, value)| {
        if name.is_empty()
            || name
                .bytes()
                .any(|byte| matches!(byte, b':' | b'\r' | b'\n'))
            || value.bytes().any(|byte| matches!(byte, b'\r' | b'\n'))
        {
            return Err(TargetError::Protocol("invalid bounded HTTP header".into()));
        }
        total
            .checked_add(name.len() + value.len() + 4)
            .ok_or_else(|| TargetError::Protocol("HTTP header size overflowed".into()))
    })?;
    if header_bytes > MAX_HEADER_BYTES {
        return Err(TargetError::Protocol(format!(
            "HTTP headers are {header_bytes} bytes; limit is {MAX_HEADER_BYTES}"
        )));
    }
    Ok(())
}

fn endpoint_spec(name: &str, port: u16, protocol: &str) -> EndpointSpec {
    EndpointSpec {
        name: name.into(),
        address: format!("{LOOPBACK}:{port}"),
        protocol: protocol.into(),
    }
}

fn connect_loopback(port: u16, timeout: Duration) -> Result<TcpStream, TargetError> {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let stream = TcpStream::connect_timeout(&address, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    Ok(stream)
}

fn free_loopback_port() -> Result<u16, TargetError> {
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))?;
    Ok(listener.local_addr()?.port())
}

fn read_line_bounded(stream: &mut TcpStream, limit: usize) -> Result<Vec<u8>, TargetError> {
    let mut bytes = Vec::new();
    let mut byte = [0_u8; 1];
    while bytes.len() <= limit {
        let count = stream.read(&mut byte)?;
        if count == 0 {
            break;
        }
        bytes.push(byte[0]);
        if byte[0] == b'\n' {
            return Ok(bytes);
        }
    }
    if bytes.len() > limit {
        return Err(TargetError::Protocol(format!(
            "response line exceeded {limit} bytes"
        )));
    }
    Ok(bytes)
}

fn read_to_end_bounded(stream: &mut TcpStream, output: &mut Vec<u8>) -> Result<(), TargetError> {
    let remaining = MAX_RESPONSE_BYTES.saturating_sub(output.len());
    let mut limited = stream.take(remaining.saturating_add(1) as u64);
    limited.read_to_end(output)?;
    if output.len() > MAX_RESPONSE_BYTES {
        return Err(TargetError::Protocol(format!(
            "response exceeded {MAX_RESPONSE_BYTES} bytes"
        )));
    }
    Ok(())
}

fn parse_http_status(response: &[u8]) -> Option<u16> {
    let line_end = response.windows(2).position(|window| window == b"\r\n")?;
    let line = std::str::from_utf8(&response[..line_end]).ok()?;
    line.split_whitespace().nth(1)?.parse().ok()
}

fn response_body(response: &[u8]) -> &[u8] {
    response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map_or(&[], |position| &response[position + 4..])
}

fn response_json_body(response: &[u8]) -> Option<Value> {
    serde_json::from_slice(response_body(response)).ok()
}

fn response_body_preview(response: &[u8]) -> String {
    const PREVIEW_BYTES: usize = 2 * 1024;
    let body = response_body(response);
    String::from_utf8_lossy(&body[..body.len().min(PREVIEW_BYTES)]).into_owned()
}

fn probe_result(result: &Result<RawResponse, TargetError>) -> String {
    match result {
        Ok(response) => response.status.map_or_else(
            || format!("raw/{}B", response.bytes.len()),
            |status| status.to_string(),
        ),
        Err(error) => error.to_string(),
    }
}

fn read_appended(path: &Path, offset: &mut u64) -> Vec<String> {
    let Ok(mut file) = File::open(path) else {
        return vec![];
    };
    if file.seek(SeekFrom::Start(*offset)).is_err() {
        return vec![];
    }
    let mut bytes = Vec::new();
    let _ = std::io::Read::by_ref(&mut file)
        .take(MAX_LOG_BYTES)
        .read_to_end(&mut bytes);
    *offset = offset.saturating_add(bytes.len() as u64);
    String::from_utf8_lossy(&bytes)
        .lines()
        .take(2_000)
        .map(str::to_owned)
        .collect()
}

fn process_from_status(pid: u32, status: &ExitStatus) -> ProcessObservation {
    ProcessObservation {
        pid: Some(pid),
        alive: false,
        exit_code: status.code(),
        signal_or_exception: status_signal(status),
    }
}

#[cfg(unix)]
fn status_signal(status: &ExitStatus) -> Option<String> {
    use std::os::unix::process::ExitStatusExt;
    status.signal().map(|signal| format!("signal {signal}"))
}

#[cfg(not(unix))]
fn status_signal(_status: &ExitStatus) -> Option<String> {
    None
}

#[cfg(unix)]
fn send_interrupt(pid: u32) -> Result<(), TargetError> {
    let status = Command::new("kill")
        .arg("-INT")
        .arg(pid.to_string())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(TargetError::Protocol(format!(
            "could not interrupt server process {pid}: {status}"
        )))
    }
}

#[cfg(not(unix))]
fn send_interrupt(_pid: u32) -> Result<(), TargetError> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn capture_threads(pid: u32) -> Result<Vec<u8>, TargetError> {
    let output = Command::new("/usr/bin/sample")
        .arg(pid.to_string())
        .arg("1")
        .arg("1")
        .output()?;
    Ok([output.stdout, output.stderr].concat())
}

#[cfg(target_os = "linux")]
fn capture_threads(pid: u32) -> Result<Vec<u8>, TargetError> {
    let output = Command::new("ps")
        .args([
            "-L",
            "-p",
            &pid.to_string(),
            "-o",
            "pid,tid,stat,wchan:32,comm",
        ])
        .output()?;
    Ok([output.stdout, output.stderr].concat())
}

#[cfg(target_os = "windows")]
fn capture_threads(pid: u32) -> Result<Vec<u8>, TargetError> {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/V"])
        .output()?;
    Ok([output.stdout, output.stderr].concat())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn capture_threads(_pid: u32) -> Result<Vec<u8>, TargetError> {
    Ok(b"thread sampling is unavailable on this platform\n".to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_validation_rejects_injection_and_oversized_bodies() {
        let mut headers = BTreeMap::new();
        headers.insert("X-Test".into(), "ok\r\ninjected: true".into());
        assert!(validate_http_request("GET", "/", &headers, &[]).is_err());
        assert!(
            validate_http_request("GET", "/", &BTreeMap::new(), &vec![0; MAX_BODY_BYTES + 1])
                .is_err()
        );
        assert!(validate_http_request("GET", "/ok", &BTreeMap::new(), &[]).is_ok());
    }

    #[test]
    fn parses_status_and_json_body() {
        let response = b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\n\r\n{\"ok\":true}";
        assert_eq!(parse_http_status(response), Some(200));
        assert_eq!(response_json_body(response), Some(json!({"ok": true})));
    }
}

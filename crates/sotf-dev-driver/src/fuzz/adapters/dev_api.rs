use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use reqwest::blocking::{Client, RequestBuilder};
use serde_json::{Value, json};
use sotf_dev_api::{Capabilities, DevReply, RUN_ID_HEADER, Snapshot};

use crate::fuzz::model::{
    Action, ActionPayload, AdapterKind, CoverageDelta, EndpointSpec, FUZZ_SCHEMA_VERSION,
    Observation, ProcessObservation, TargetId, TargetSpec,
};
use crate::fuzz::supervisor::{FuzzTarget, LaunchContext, TargetError};

#[derive(Debug, Clone)]
pub struct DevApiTargetConfig {
    pub target: TargetId,
    pub url: Option<String>,
    pub executable: Option<PathBuf>,
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub pass_qa_argument: bool,
    pub startup_timeout: Duration,
}

impl DevApiTargetConfig {
    pub fn desktop(executable: Option<PathBuf>, url: Option<String>) -> Self {
        Self {
            target: TargetId::DesktopGpui,
            url,
            executable,
            arguments: vec![],
            environment: BTreeMap::new(),
            pass_qa_argument: true,
            startup_timeout: Duration::from_secs(30),
        }
    }

    pub fn tui(executable: Option<PathBuf>, url: Option<String>) -> Self {
        Self {
            target: TargetId::Tui,
            url,
            executable,
            arguments: vec![],
            environment: BTreeMap::new(),
            pass_qa_argument: true,
            startup_timeout: Duration::from_secs(15),
        }
    }
}

pub struct DevApiTarget {
    config: DevApiTargetConfig,
    client: Client,
    base_url: Option<String>,
    run_id: Option<String>,
    run_dir: Option<PathBuf>,
    child: Option<Child>,
    stdout_path: Option<PathBuf>,
    stderr_path: Option<PathBuf>,
    stdout_offset: u64,
    stderr_offset: u64,
    capabilities: Option<Capabilities>,
    last_snapshot_revision: u64,
}

impl DevApiTarget {
    pub fn new(config: DevApiTargetConfig) -> Result<Self, TargetError> {
        Ok(Self {
            config,
            client: Client::builder().timeout(Duration::from_secs(5)).build()?,
            base_url: None,
            run_id: None,
            run_dir: None,
            child: None,
            stdout_path: None,
            stderr_path: None,
            stdout_offset: 0,
            stderr_offset: 0,
            capabilities: None,
            last_snapshot_revision: 0,
        })
    }

    fn authenticated(&self, request: RequestBuilder) -> Result<RequestBuilder, TargetError> {
        let run_id = self
            .run_id
            .as_deref()
            .ok_or_else(|| TargetError::Protocol("dev API target is not launched".into()))?;
        Ok(request.header(RUN_ID_HEADER, run_id))
    }

    fn base(&self) -> Result<&str, TargetError> {
        self.base_url
            .as_deref()
            .ok_or_else(|| TargetError::Protocol("dev API endpoint is unavailable".into()))
    }

    fn get_reply(&self, path: &str, timeout: Duration) -> Result<DevReply, TargetError> {
        let request = self.client.get(format!("{}{path}", self.base()?));
        let response = self.authenticated(request)?.timeout(timeout).send()?;
        parse_reply(response)
    }

    fn post_reply(
        &self,
        path: &str,
        body: &Value,
        timeout: Duration,
    ) -> Result<DevReply, TargetError> {
        let request = self
            .client
            .post(format!("{}{path}", self.base()?))
            .json(body);
        let response = self.authenticated(request)?.timeout(timeout).send()?;
        parse_reply(response)
    }

    fn wait_until_live(&self) -> Result<(), TargetError> {
        let deadline = Instant::now() + self.config.startup_timeout;
        let mut last_error = None;
        while Instant::now() < deadline {
            match self.get_reply("/live", Duration::from_secs(2)) {
                Ok(reply) if reply.ok => return Ok(()),
                Ok(reply) => last_error = reply.error,
                Err(error) => last_error = Some(error.to_string()),
            }
            thread::sleep(Duration::from_millis(50));
        }
        Err(TargetError::Timeout(format!(
            "dev API did not become live: {}",
            last_error.unwrap_or_else(|| "no response".into())
        )))
    }

    fn execute_payload(&self, action: &Action) -> Result<DevReply, TargetError> {
        let timeout = Duration::from_millis(action.timeout_ms.max(1));
        match &action.payload {
            ActionPayload::DevAction { name, payload } => self.post_reply(
                "/action",
                &json!({"name": name, "payload": payload}),
                timeout,
            ),
            ActionPayload::Query { path } => {
                let mut url = reqwest::Url::parse(&format!("{}/query", self.base()?))
                    .map_err(|error| TargetError::Protocol(error.to_string()))?;
                url.query_pairs_mut().append_pair("path", path);
                let request = self.client.get(url);
                parse_reply(self.authenticated(request)?.timeout(timeout).send()?)
            }
            ActionPayload::Key { keystroke } => {
                self.post_reply("/key", &json!({"keystroke": keystroke}), timeout)
            }
            ActionPayload::Text { text } => {
                let mut last = DevReply::success(Value::Null);
                for character in text.chars() {
                    last = self.post_reply(
                        "/key",
                        &json!({"keystroke": character.to_string()}),
                        timeout,
                    )?;
                    if !last.ok {
                        break;
                    }
                }
                Ok(last)
            }
            ActionPayload::Selector {
                operation,
                selector,
            } => self.post_reply(
                &format!("/{operation}"),
                &json!({"selector": selector}),
                timeout,
            ),
            ActionPayload::Coordinate { input } => {
                let mut input = input.clone();
                match &mut input {
                    sotf_dev_api::CoordinateInput::Pointer {
                        viewport_revision, ..
                    }
                    | sotf_dev_api::CoordinateInput::Touch {
                        viewport_revision, ..
                    }
                    | sotf_dev_api::CoordinateInput::Scroll {
                        viewport_revision, ..
                    } if *viewport_revision == 0 => {
                        *viewport_revision = self.last_snapshot_revision;
                    }
                    _ => {}
                }
                self.post_reply("/input", &serde_json::to_value(input)?, timeout)
            }
            ActionPayload::Wait { duration_ms } => {
                thread::sleep(Duration::from_millis(*duration_ms));
                Ok(DevReply::success(Value::Null))
            }
            ActionPayload::Restart => Err(TargetError::Protocol(
                "restart must be implemented as an adapter lifecycle action".into(),
            )),
            _ => Err(TargetError::Protocol(format!(
                "payload {:?} is not supported by the dev API adapter",
                action.payload
            ))),
        }
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

    fn process_observation(&mut self) -> ProcessObservation {
        match self.child.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(status)) => ProcessObservation {
                    pid: Some(child.id()),
                    alive: false,
                    exit_code: status.code(),
                    signal_or_exception: exit_signal(&status),
                },
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
            None => ProcessObservation {
                pid: None,
                alive: true,
                exit_code: None,
                signal_or_exception: None,
            },
        }
    }
}

impl FuzzTarget for DevApiTarget {
    fn target_id(&self) -> TargetId {
        self.config.target
    }

    fn launch(&mut self, context: &LaunchContext<'_>) -> Result<TargetSpec, TargetError> {
        self.run_id = Some(context.run_id.as_str().to_owned());
        self.run_dir = Some(context.run_dir.to_owned());
        let endpoint = if let Some(url) = &self.config.url {
            url.trim_end_matches('/').to_owned()
        } else {
            let executable = self.config.executable.as_ref().ok_or_else(|| {
                TargetError::Protocol("dev API target needs --url or --executable".into())
            })?;
            let port = free_loopback_port()?;
            let stdout_path = context.run_dir.join("logs/stdout.log");
            let stderr_path = context.run_dir.join("logs/stderr.log");
            let stdout = File::create(&stdout_path)?;
            let stderr = File::create(&stderr_path)?;
            let mut command = Command::new(executable);
            command.env_clear();
            for name in [
                "PATH",
                "RUST_BACKTRACE",
                "RUST_LOG",
                "LANG",
                "LC_ALL",
                "TMPDIR",
            ] {
                if let Some(value) = std::env::var_os(name) {
                    command.env(name, value);
                }
            }
            command
                .env("SOTF_DEV_API_PORT", port.to_string())
                .env("SOTF_DEV_API_RUN_ID", context.run_id.as_str())
                .env("SOTF_QA_DIR", context.run_dir.join("qa"))
                .env("TMPDIR", context.run_dir.join("tmp"));
            for (name, value) in &self.config.environment {
                command.env(name, value);
            }
            if self.config.pass_qa_argument {
                command.arg("--qa").arg(context.run_dir.join("qa"));
            }
            command
                .args(&self.config.arguments)
                .stdout(Stdio::from(stdout))
                .stderr(Stdio::from(stderr));
            self.child = Some(command.spawn()?);
            self.stdout_path = Some(stdout_path);
            self.stderr_path = Some(stderr_path);
            format!("http://127.0.0.1:{port}")
        };
        self.base_url = Some(endpoint.clone());
        self.wait_until_live()?;
        let capabilities_reply = self.get_reply("/capabilities", Duration::from_secs(5))?;
        let capabilities: Capabilities = serde_json::from_value(
            capabilities_reply
                .value
                .ok_or_else(|| TargetError::Protocol("capabilities reply has no value".into()))?,
        )?;
        if capabilities.target_id != self.config.target.as_str() {
            return Err(TargetError::Protocol(format!(
                "target identity mismatch: expected {}, got {}",
                self.config.target, capabilities.target_id
            )));
        }
        self.capabilities = Some(capabilities.clone());
        Ok(TargetSpec {
            schema_version: FUZZ_SCHEMA_VERSION,
            target_id: self.config.target,
            adapter: AdapterKind::DevApi,
            executable: self.config.executable.clone(),
            app_identity: Some(capabilities.process_name.clone()),
            platform: capabilities.platform.clone(),
            fixture_profile: context.fixture_profile.to_owned(),
            environment_names: self.config.environment.keys().cloned().collect(),
            endpoints: vec![EndpointSpec {
                name: "dev-api".into(),
                address: endpoint,
                protocol: "sotf-dev-api-v2".into(),
            }],
            run_id_hash: context.run_id.redacted_hash(),
            capability_fingerprint: capabilities.fingerprint()?,
            build_id: capabilities.build_id.clone(),
        })
    }

    fn capabilities(&mut self) -> Result<Capabilities, TargetError> {
        self.capabilities
            .clone()
            .ok_or_else(|| TargetError::Protocol("capabilities are unavailable".into()))
    }

    fn snapshot(&mut self) -> Result<Snapshot, TargetError> {
        let reply = self.get_reply("/snapshot", Duration::from_secs(5))?;
        if !reply.ok {
            return Err(TargetError::Protocol(
                reply.error.unwrap_or_else(|| "snapshot failed".into()),
            ));
        }
        let snapshot: Snapshot = serde_json::from_value(
            reply
                .value
                .ok_or_else(|| TargetError::Protocol("snapshot reply has no value".into()))?,
        )
        .map_err(TargetError::from)?;
        self.last_snapshot_revision = snapshot.state_revision;
        Ok(snapshot)
    }

    fn execute(&mut self, action: &Action) -> Result<Observation, TargetError> {
        let state_revision_before = self.last_snapshot_revision;
        let mut reply = self.execute_payload(action)?;
        let snapshot = self.snapshot().ok();
        reply.meta.state_revision_before = state_revision_before;
        reply.meta.state_revision_after = snapshot
            .as_ref()
            .map_or(state_revision_before, |snapshot| snapshot.state_revision);
        reply.meta.render_revision = snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.render_revision);
        reply.meta.accessibility_revision = snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.accessibility_revision);
        let process = self.process_observation();
        let new_logs = self.read_new_logs();
        Ok(Observation {
            schema_version: FUZZ_SCHEMA_VERSION,
            sequence: action.sequence,
            reply: Some(reply),
            snapshot,
            process,
            resource: None,
            new_logs,
            crash_files: vec![],
            coverage: CoverageDelta::default(),
            screenshot: None,
            failure_candidate: None,
        })
    }

    fn live(&mut self) -> Result<bool, TargetError> {
        Ok(self
            .get_reply("/live", Duration::from_secs(2))
            .is_ok_and(|reply| reply.ok))
    }

    fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    fn capture_hang(&mut self, directory: &Path) -> Result<Vec<PathBuf>, TargetError> {
        fs::create_dir_all(directory)?;
        let Some(pid) = self.pid() else {
            return Ok(vec![]);
        };
        let output = directory.join("threads.txt");
        #[cfg(target_os = "macos")]
        let status = Command::new("/usr/bin/sample")
            .arg(pid.to_string())
            .arg("3")
            .arg("-file")
            .arg(&output)
            .status();
        #[cfg(target_os = "linux")]
        let status = Command::new("gstack")
            .arg(pid.to_string())
            .stdout(Stdio::from(File::create(&output)?))
            .status();
        #[cfg(target_os = "windows")]
        let status: Result<std::process::ExitStatus, std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "no default Windows stack provider",
        ));
        match status {
            Ok(status) if status.success() => Ok(vec![output]),
            Ok(status) => {
                fs::write(
                    &output,
                    format!("thread sampler exited with status {status}"),
                )?;
                Ok(vec![output])
            }
            Err(error) => {
                fs::write(&output, format!("thread sampling unavailable: {error}"))?;
                Ok(vec![output])
            }
        }
    }

    fn shutdown(&mut self) -> Result<(), TargetError> {
        let _ = self.post_reply("/quit", &json!({}), Duration::from_secs(2));
        let Some(mut child) = self.child.take() else {
            return Ok(());
        };
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if child.try_wait()?.is_some() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(25));
        }
        child.kill()?;
        let _ = child.wait();
        Err(TargetError::Protocol(
            "target did not stop after /quit and required termination".into(),
        ))
    }
}

fn parse_reply(response: reqwest::blocking::Response) -> Result<DevReply, TargetError> {
    let status = response.status();
    let bytes = response.bytes()?;
    let reply: DevReply = serde_json::from_slice(&bytes).map_err(|error| {
        TargetError::Protocol(format!(
            "invalid dev API response ({status}): {error}; body={}",
            String::from_utf8_lossy(&bytes)
        ))
    })?;
    Ok(reply)
}

fn free_loopback_port() -> Result<u16, TargetError> {
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))?;
    Ok(listener.local_addr()?.port())
}

fn read_appended(path: &Path, offset: &mut u64) -> Vec<String> {
    let Ok(mut file) = File::open(path) else {
        return vec![];
    };
    if file.seek(SeekFrom::Start(*offset)).is_err() {
        return vec![];
    }
    let mut bytes = Vec::new();
    if file
        .by_ref()
        .take(1024 * 1024)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return vec![];
    }
    *offset = offset.saturating_add(bytes.len() as u64);
    String::from_utf8_lossy(&bytes)
        .lines()
        .map(str::to_owned)
        .collect()
}

#[cfg(unix)]
fn exit_signal(status: &std::process::ExitStatus) -> Option<String> {
    use std::os::unix::process::ExitStatusExt;
    status.signal().map(|signal| format!("signal {signal}"))
}

#[cfg(not(unix))]
fn exit_signal(_status: &std::process::ExitStatus) -> Option<String> {
    None
}

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::json;
use sotf_dev_api::{RunId, Snapshot};
use uuid::Uuid;

use super::adapters::{
    DevApiTarget, DevApiTargetConfig, ProcessTarget, ProcessTargetConfig, ServerTarget,
    ServerTargetConfig, UnsupportedTarget,
};
#[cfg(unix)]
use super::adapters::{SystemwideTarget, SystemwideTargetConfig};
use super::artifact::ArtifactStore;
use super::failure::{normalize_signature, observation_failures};
use super::minimize::{ReplayOracle, confirm_two_of_three, minimize_actions};
use super::model::{
    Action, FUZZ_SCHEMA_VERSION, FailureClass, FailureSignature, ReplayConfig, TargetId, TraceEvent,
};
use super::report::RunSummary;
use super::supervisor::{
    FuzzConfig, FuzzRunResult, FuzzTarget, LaunchContext, TargetError, classify_timeout, run_fuzz,
};
use super::trace::{TraceWriter, read_trace, resolved_actions};
use crate::fuzz::SurfaceManifest;

#[derive(Debug, Clone)]
pub struct FuzzCommandOptions {
    pub target: TargetId,
    pub seed: u64,
    pub steps: u64,
    pub time_budget: Option<Duration>,
    pub workers: u32,
    pub fixture_profile: String,
    pub artifact_root: PathBuf,
    pub manifest: Option<PathBuf>,
    pub executable: Option<PathBuf>,
    pub url: Option<String>,
    /// Run ID of an externally managed `--url` target; rejected without `--url`.
    pub run_id: Option<String>,
    pub durable_trace: bool,
    pub opt_ins: BTreeSet<String>,
}

pub fn run_fuzz_command(options: FuzzCommandOptions) -> Result<Vec<FuzzRunResult>> {
    if options.workers == 0 {
        bail!("worker count must be at least one");
    }
    if options.workers > 1 && options.url.is_some() {
        bail!("multiple workers cannot share one externally managed --url target");
    }
    if options.run_id.is_some() && options.url.is_none() {
        bail!("--run-id is only valid together with --url");
    }
    let manifest_path = options
        .manifest
        .clone()
        .unwrap_or_else(|| default_manifest_path(options.target));
    let manifest_source = fs::read_to_string(&manifest_path)
        .with_context(|| format!("reading surface manifest {}", manifest_path.display()))?;
    let manifest = SurfaceManifest::parse_toml(&manifest_source)
        .with_context(|| format!("parsing surface manifest {}", manifest_path.display()))?;

    if options.workers == 1 {
        return Ok(vec![run_worker(&options, &manifest, 0)?]);
    }
    let results = std::thread::scope(|scope| {
        let handles = (0..options.workers)
            .map(|worker| {
                let options = options.clone();
                let manifest = manifest.clone();
                scope.spawn(move || run_worker(&options, &manifest, worker))
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| handle.join().map_err(|_| anyhow!("fuzz worker panicked"))?)
            .collect::<Result<Vec<_>>>()
    })?;
    Ok(results)
}

fn run_worker(
    options: &FuzzCommandOptions,
    manifest: &SurfaceManifest,
    worker: u32,
) -> Result<FuzzRunResult> {
    let config = FuzzConfig {
        target: options.target,
        seed: options.seed,
        worker,
        steps: options.steps,
        time_budget: options.time_budget,
        fixture_profile: options.fixture_profile.clone(),
        artifact_root: options.artifact_root.clone(),
        durable_trace: options.durable_trace,
        opt_ins: options.opt_ins.clone(),
        run_id: options.run_id.clone(),
    };
    let mut target = make_target(
        options.target,
        options.executable.clone(),
        options.url.clone(),
    )?;
    run_fuzz(&config, manifest, target.as_mut()).map_err(anyhow::Error::from)
}

fn make_target(
    target: TargetId,
    executable: Option<PathBuf>,
    url: Option<String>,
) -> Result<Box<dyn FuzzTarget>> {
    match target {
        TargetId::DesktopGpui | TargetId::Tui => {
            let default_executable = match target {
                TargetId::DesktopGpui => PathBuf::from("target/debug/sotf-desktop"),
                TargetId::Tui => PathBuf::from("target/debug/sotf-tui"),
                _ => unreachable!(),
            };
            let executable = executable.or_else(|| url.is_none().then_some(default_executable));
            if url.is_none() && executable.as_ref().is_some_and(|path| !path.is_file()) {
                return Ok(Box::new(UnsupportedTarget::new(
                    target,
                    "feature_missing",
                    format!(
                        "{} is not built; run the target's dev-api build recipe first",
                        executable.as_ref().unwrap().display()
                    ),
                )));
            }
            let config = if target == TargetId::DesktopGpui {
                DevApiTargetConfig::desktop(executable, url)
            } else {
                DevApiTargetConfig::tui(executable, url)
            };
            Ok(Box::new(DevApiTarget::new(config)?))
        }
        TargetId::PlayerCli | TargetId::RecorderCli => {
            let default_executable = match target {
                TargetId::PlayerCli => PathBuf::from("target/debug/player-cli"),
                TargetId::RecorderCli => PathBuf::from("target/debug/sotf-recorder-cli"),
                _ => unreachable!(),
            };
            Ok(Box::new(ProcessTarget::new(ProcessTargetConfig {
                target,
                executable: executable.unwrap_or(default_executable),
                environment: Default::default(),
            })))
        }
        TargetId::HeadlessServer => {
            if url.is_some() {
                bail!("headless-server currently requires a managed --executable, not --url");
            }
            Ok(Box::new(ServerTarget::new(ServerTargetConfig::new(
                executable.unwrap_or_else(|| PathBuf::from("target/debug/sotf-desktop")),
            ))))
        }
        #[cfg(unix)]
        TargetId::SystemwideDaemon => {
            if url.is_some() {
                bail!("systemwide-daemon requires managed --executable, not --url");
            }
            Ok(Box::new(SystemwideTarget::new(
                SystemwideTargetConfig::new(
                    executable.unwrap_or_else(|| PathBuf::from("target/debug/sotf-daemon")),
                ),
            )))
        }
        TargetId::IosSim | TargetId::TvosSim if std::env::consts::OS != "macos" => Ok(Box::new(
            UnsupportedTarget::new(target, "platform", "Apple simulator targets require macOS"),
        )),
        TargetId::Configbar if std::env::consts::OS != "macos" => Ok(Box::new(
            UnsupportedTarget::new(target, "platform", "ConfigBar requires macOS"),
        )),
        _ => Ok(Box::new(UnsupportedTarget::new(
            target,
            "feature_missing",
            "the target adapter is not available in this build",
        ))),
    }
}

pub fn default_manifest_path(target: TargetId) -> PathBuf {
    let name = format!("{}.toml", target.as_str());
    let checked_in = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fuzz")
        .join(&name);
    if checked_in.is_file() {
        checked_in
    } else {
        PathBuf::from("crates/sotf-dev-driver/fuzz").join(name)
    }
}

#[derive(Debug, Clone)]
pub struct ReplayCommandOptions {
    pub replay: PathBuf,
    pub executable: Option<PathBuf>,
    pub url: Option<String>,
    /// Run ID of an externally managed `--url` target; rejected without `--url`.
    pub run_id: Option<String>,
    pub best_effort_capabilities: bool,
}

pub fn run_replay_command(options: &ReplayCommandOptions) -> Result<Option<FailureSignature>> {
    if options.run_id.is_some() && options.url.is_none() {
        bail!("--run-id is only valid together with --url");
    }
    let replay = load_replay(&options.replay)?;
    let events = read_trace(&resolve_replay_path(&options.replay, &replay.trace_path))?;
    let actions = resolved_actions(&events);
    let manifest = load_replay_manifest(&options.replay, &replay);
    let mut target = make_target(
        replay.target.target_id,
        options
            .executable
            .clone()
            .or_else(|| replay.target.executable.clone()),
        options.url.clone(),
    )?;
    replay_actions(
        &replay,
        &actions,
        target.as_mut(),
        options.best_effort_capabilities,
        None,
        options.run_id.as_deref(),
        manifest.as_ref(),
    )
}

#[allow(clippy::too_many_arguments)]
fn replay_actions(
    replay: &ReplayConfig,
    actions: &[Action],
    target: &mut dyn FuzzTarget,
    best_effort_capabilities: bool,
    attempt_root: Option<&Path>,
    run_id: Option<&str>,
    manifest: Option<&SurfaceManifest>,
) -> Result<Option<FailureSignature>> {
    let parent = attempt_root.unwrap_or(&replay.artifact_dir);
    fs::create_dir_all(parent)?;
    let name = format!("replay-{}", Uuid::new_v4().simple());
    let store = ArtifactStore::create(parent, &name)?;
    for directory in ["qa", "tmp", "logs", "hang"] {
        store.create_dir(directory)?;
    }
    let raw_run_id = run_id
        .map(str::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().simple().to_string());
    let run_id = RunId::parse(raw_run_id)?;
    let context = LaunchContext {
        run_id: &run_id,
        run_dir: store.run_dir(),
        fixture_profile: &replay.target.fixture_profile,
        opt_ins: &replay.opt_ins.iter().cloned().collect(),
    };
    target.launch(&context)?;
    let capabilities = target.capabilities()?;
    let fingerprint = capabilities.fingerprint()?;
    if !best_effort_capabilities && fingerprint != replay.target.capability_fingerprint {
        let _ = target.shutdown();
        bail!(
            "capability fingerprint drift: recorded {}, current {}",
            replay.target.capability_fingerprint,
            fingerprint
        );
    }
    let initial = target.snapshot()?;
    let mut snapshot = initial;
    let mut trace = TraceWriter::create(store.path("trace.ndjson")?, false)?;
    for action in actions {
        trace.append(&TraceEvent::ActionIntent {
            action: Box::new(action.clone()),
            preceding_revision: snapshot.state_revision,
            preceding_state_hash: snapshot.state_hash.clone(),
        })?;
        let observation = match target.execute(action) {
            Ok(observation) => observation,
            // Replay routes timeouts through the same hang state machine as
            // capture so stall/hang classes reproduce under minimize.
            Err(TargetError::Timeout(message)) => {
                let failure = classify_timeout(target, &store, action, &replay.target, message)?;
                let _ = target.shutdown();
                return Ok(Some(failure.signature));
            }
            Err(TargetError::ProcessExited(message)) => {
                let signature = FailureSignature {
                    class: FailureClass::UnexpectedExit,
                    normalized: normalize_signature(&message, store.run_dir().to_str()),
                };
                let _ = target.shutdown();
                return Ok(Some(signature));
            }
            Err(error) => {
                let signature = FailureSignature {
                    class: FailureClass::UnexpectedExit,
                    normalized: normalize_signature(&error.to_string(), store.run_dir().to_str()),
                };
                let _ = target.shutdown();
                return Ok(Some(signature));
            }
        };
        let failure = observation_signature(action, &observation, store.run_dir().to_str());
        trace.append(&TraceEvent::Observation {
            observation: Box::new(observation.clone()),
        })?;
        if failure.is_some() {
            let _ = target.shutdown();
            return Ok(failure);
        }
        if let Some(next) = observation.snapshot {
            if let Some(manifest) = manifest
                && let Some(signature) =
                    replay_invariant_signature(target, manifest, &snapshot, &next, &store)?
            {
                let _ = target.shutdown();
                return Ok(Some(signature));
            }
            snapshot = next;
        }
    }
    target.shutdown()?;
    Ok(None)
}

/// Re-evaluate manifest invariants during replay so invariant signatures
/// reproduce under replay/minimize, using the same confirmation rule as
/// capture (two consistent snapshots, or immediately when terminal).
fn replay_invariant_signature(
    target: &mut dyn FuzzTarget,
    manifest: &SurfaceManifest,
    previous: &Snapshot,
    current: &Snapshot,
    store: &ArtifactStore,
) -> Result<Option<FailureSignature>> {
    let run_dir = store.run_dir().to_str();
    if current.state_revision < previous.state_revision {
        return Ok(Some(FailureSignature {
            class: FailureClass::InvariantViolation,
            normalized: normalize_signature("state_revision_monotonic", run_dir),
        }));
    }
    for invariant in &manifest.invariants {
        if invariant.condition.evaluate(current) {
            continue;
        }
        let confirmed = if invariant.terminal {
            true
        } else {
            match target.snapshot() {
                Ok(confirmation) => !invariant.condition.evaluate(&confirmation),
                Err(_) => false,
            }
        };
        if confirmed {
            return Ok(Some(FailureSignature {
                class: FailureClass::InvariantViolation,
                normalized: normalize_signature(&invariant.id, run_dir),
            }));
        }
    }
    Ok(None)
}

/// Shared replay signature pipeline: same classification and normalization as
/// capture (`observation_failures` + `normalize_signature`), so signatures
/// are comparable with the recorded run.
fn observation_signature(
    action: &Action,
    observation: &super::model::Observation,
    run_dir: Option<&str>,
) -> Option<FailureSignature> {
    observation_failures(action, observation)
        .into_iter()
        .next()
        .map(|observed| FailureSignature {
            class: observed.class,
            normalized: normalize_signature(&observed.signature, run_dir),
        })
        .or_else(|| {
            observation
                .failure_candidate
                .as_ref()
                .map(|failure| failure.signature.clone())
        })
}

#[derive(Debug, Clone)]
pub struct MinimizeCommandOptions {
    pub replay: PathBuf,
    pub executable: Option<PathBuf>,
    pub url: Option<String>,
    /// Run ID of an externally managed `--url` target; rejected without `--url`.
    pub run_id: Option<String>,
}

pub fn run_minimize_command(options: &MinimizeCommandOptions) -> Result<PathBuf> {
    if options.run_id.is_some() && options.url.is_none() {
        bail!("--run-id is only valid together with --url");
    }
    let replay = load_replay(&options.replay)?;
    let expected = load_recorded_signature(&replay)?;
    if !matches!(
        expected.class,
        FailureClass::SignalOrException
            | FailureClass::UnexpectedExit
            | FailureClass::MainLoopStall
            | FailureClass::WholeProcessHang
            | FailureClass::InvariantViolation
            | FailureClass::PanicOrErrorLog
            | FailureClass::ValidActionRejection
    ) {
        bail!(
            "automatic minimization is not supported for {:?}",
            expected.class
        );
    }
    let events = read_trace(&resolve_replay_path(&options.replay, &replay.trace_path))?;
    let actions = resolved_actions(&events);
    let mut oracle = TargetOracle {
        manifest: load_replay_manifest(&options.replay, &replay),
        replay: replay.clone(),
        executable: options
            .executable
            .clone()
            .or_else(|| replay.target.executable.clone()),
        url: options.url.clone(),
        run_id: options.run_id.clone(),
    };
    let minimized = minimize_actions(&mut oracle, &actions, &expected);
    let confirmation = confirm_two_of_three(&mut oracle, &minimized, &expected);
    if confirmation.matches < 2 {
        bail!("minimized candidate did not reproduce two of three times");
    }
    let trace_path = replay.artifact_dir.join("trace.min.ndjson");
    let mut writer = TraceWriter::create(&trace_path, true)?;
    for action in &minimized {
        writer.append(&TraceEvent::ActionIntent {
            action: Box::new(action.clone()),
            preceding_revision: 0,
            preceding_state_hash: String::new(),
        })?;
    }
    let mut minimized_replay = replay;
    minimized_replay.trace_path = PathBuf::from("trace.min.ndjson");
    let replay_path = minimized_replay.artifact_dir.join("replay.min.toml");
    fs::write(&replay_path, toml::to_string_pretty(&minimized_replay)?)?;
    fs::write(
        minimized_replay
            .artifact_dir
            .join("minimize-confirmation.json"),
        serde_json::to_vec_pretty(&json!({
            "expected": expected,
            "matches": confirmation.matches,
            "outcomes": confirmation.outcomes,
            "original_actions": actions.len(),
            "minimized_actions": minimized.len(),
        }))?,
    )?;
    Ok(replay_path)
}

struct TargetOracle {
    replay: ReplayConfig,
    executable: Option<PathBuf>,
    url: Option<String>,
    run_id: Option<String>,
    manifest: Option<SurfaceManifest>,
}

impl ReplayOracle for TargetOracle {
    fn replay(&mut self, actions: &[Action]) -> Option<FailureSignature> {
        let mut target = make_target(
            self.replay.target.target_id,
            self.executable.clone(),
            self.url.clone(),
        )
        .ok()?;
        replay_actions(
            &self.replay,
            actions,
            target.as_mut(),
            false,
            Some(&self.replay.artifact_dir.join("minimize-attempts")),
            self.run_id.as_deref(),
            self.manifest.as_ref(),
        )
        .ok()
        .flatten()
    }
}

/// The expected failure signature for minimization comes from run.json, which
/// stores the unredacted normalized signature. summary.json is the redacted,
/// portable report and is only a fallback for older artifacts.
fn load_recorded_signature(replay: &ReplayConfig) -> Result<FailureSignature> {
    let run_path = replay.artifact_dir.join("run.json");
    if let Ok(bytes) = fs::read(&run_path) {
        let metadata: serde_json::Value = serde_json::from_slice(&bytes)
            .with_context(|| format!("parsing run metadata {}", run_path.display()))?;
        if let Some(signature) = metadata
            .get("failures")
            .and_then(|failures| failures.as_array())
            .and_then(|failures| failures.first())
            .and_then(|failure| failure.get("signature"))
        {
            return serde_json::from_value(signature.clone())
                .context("parsing recorded failure signature from run.json");
        }
    }
    let summary_path = replay.artifact_dir.join("summary.json");
    let summary: RunSummary = serde_json::from_slice(
        &fs::read(&summary_path)
            .with_context(|| format!("reading failure summary {}", summary_path.display()))?,
    )?;
    summary
        .failures
        .first()
        .map(|failure| failure.signature.clone())
        .ok_or_else(|| anyhow!("recorded run has no failure to minimize"))
}

fn load_replay_manifest(replay_file: &Path, replay: &ReplayConfig) -> Option<SurfaceManifest> {
    let path = resolve_replay_path(replay_file, &replay.manifest_path);
    let source = fs::read_to_string(path).ok()?;
    SurfaceManifest::parse_toml(&source).ok()
}

fn load_replay(path: &Path) -> Result<ReplayConfig> {
    let replay: ReplayConfig = toml::from_str(
        &fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?,
    )
    .with_context(|| format!("parsing {}", path.display()))?;
    if replay.schema_version != FUZZ_SCHEMA_VERSION {
        bail!(
            "replay schema version {} is unsupported (expected {FUZZ_SCHEMA_VERSION}); \
             convert the artifact explicitly instead of reinterpreting it",
            replay.schema_version
        );
    }
    Ok(replay)
}

fn resolve_replay_path(replay_file: &Path, recorded: &Path) -> PathBuf {
    if recorded.is_absolute() {
        recorded.to_owned()
    } else {
        replay_file
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(recorded)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::{Value, json};
    use sotf_dev_api::{Capabilities, DevReply, NamedCapability, Snapshot};
    use tempfile::tempdir;

    use super::*;
    use crate::fuzz::manifest::{Condition, ManifestAction, ManifestInvariant, SurfaceManifest};
    use crate::fuzz::model::{
        ActionClass, ActionPayload, AdapterKind, CoverageDelta, EndpointSpec, Observation,
        ProcessObservation, TargetSpec,
    };

    /// Synthetic target that dies with signal 6 when it executes "boom".
    struct CrashTarget {
        revision: u64,
        exited: bool,
        ready: bool,
    }

    impl CrashTarget {
        fn new() -> Self {
            Self {
                revision: 0,
                exited: false,
                ready: true,
            }
        }
    }

    fn synthetic_capabilities() -> Capabilities {
        let mut capabilities = Capabilities::new("tui", "synthetic-crash");
        capabilities.actions.push(NamedCapability {
            name: "boom".into(),
            family: "state".into(),
            payload_schema: None,
        });
        capabilities
    }

    fn target_spec(fingerprint: String) -> TargetSpec {
        TargetSpec {
            schema_version: FUZZ_SCHEMA_VERSION,
            target_id: TargetId::Tui,
            adapter: AdapterKind::Synthetic,
            executable: None,
            app_identity: Some("synthetic-crash".into()),
            platform: std::env::consts::OS.into(),
            fixture_profile: "none".into(),
            environment_names: vec![],
            endpoints: vec![EndpointSpec {
                name: "synthetic".into(),
                address: "in-memory".into(),
                protocol: "synthetic".into(),
            }],
            run_id_hash: String::new(),
            capability_fingerprint: fingerprint,
            build_id: "test-build".into(),
        }
    }

    impl FuzzTarget for CrashTarget {
        fn target_id(&self) -> TargetId {
            TargetId::Tui
        }

        fn launch(&mut self, _context: &LaunchContext<'_>) -> Result<TargetSpec, TargetError> {
            Ok(target_spec(String::new()))
        }

        fn capabilities(&mut self) -> Result<Capabilities, TargetError> {
            Ok(synthetic_capabilities())
        }

        fn snapshot(&mut self) -> Result<Snapshot, TargetError> {
            if self.exited {
                return Err(TargetError::ProcessExited("synthetic exit".into()));
            }
            Ok(Snapshot::new("tui", self.revision, json!({"ready": self.ready})).unwrap())
        }

        fn execute(&mut self, action: &Action) -> Result<Observation, TargetError> {
            self.revision += 1;
            if action.id == "boom" {
                self.exited = true;
            }
            Ok(Observation {
                schema_version: FUZZ_SCHEMA_VERSION,
                sequence: action.sequence,
                reply: Some(DevReply::success(Value::Null)),
                snapshot: if self.exited {
                    None
                } else {
                    Some(self.snapshot()?)
                },
                process: ProcessObservation {
                    pid: None,
                    alive: !self.exited,
                    exit_code: self.exited.then_some(0),
                    signal_or_exception: self.exited.then(|| "signal 6".to_owned()),
                },
                resource: None,
                new_logs: vec![],
                crash_files: vec![],
                coverage: CoverageDelta::default(),
                screenshot: None,
                failure_candidate: None,
            })
        }

        fn live(&mut self) -> Result<bool, TargetError> {
            if self.exited {
                return Err(TargetError::ProcessExited("synthetic exit".into()));
            }
            Ok(true)
        }

        fn shutdown(&mut self) -> Result<(), TargetError> {
            Ok(())
        }
    }

    fn boom_manifest() -> SurfaceManifest {
        SurfaceManifest {
            schema_version: 1,
            version: 1,
            target: TargetId::Tui,
            fixture_profiles: vec!["none".into()],
            actions: vec![ManifestAction {
                id: "boom".into(),
                family: "state".into(),
                weight: 100,
                precondition_id: None,
                precondition: Condition::Always,
                recovery: false,
                chaos_only: false,
                payload: ActionPayload::DevAction {
                    name: "boom".into(),
                    payload: Value::Null,
                },
                timeout_ms: 100,
                coverage: vec![],
            }],
            invariants: vec![],
            workflows: vec![],
        }
    }

    fn fuzz_config(root: &Path) -> FuzzConfig {
        FuzzConfig {
            target: TargetId::Tui,
            seed: 3,
            worker: 0,
            steps: 3,
            time_budget: None,
            fixture_profile: "none".into(),
            artifact_root: root.to_owned(),
            durable_trace: false,
            opt_ins: BTreeSet::new(),
            run_id: None,
        }
    }

    #[test]
    fn minimize_confirmation_uses_the_shared_signature_path() {
        let root = tempdir().unwrap();
        let manifest = boom_manifest();
        // Capture a failing run through the real supervisor.
        let result = run_fuzz(
            &fuzz_config(root.path()),
            &manifest,
            &mut CrashTarget::new(),
        )
        .unwrap();
        assert_eq!(result.summary.outcome, "failed");

        // The expected signature comes from run.json (unredacted, normalized).
        let replay: ReplayConfig =
            toml::from_str(&fs::read_to_string(result.run_dir.join("replay.toml")).unwrap())
                .unwrap();
        let expected = load_recorded_signature(&replay).unwrap();
        assert_eq!(expected.class, FailureClass::UnexpectedExit);

        // Replaying the recorded trace against a fresh target reproduces the
        // exact same normalized signature.
        let events = read_trace(&result.run_dir.join("trace.ndjson")).unwrap();
        let actions = resolved_actions(&events);
        let replayed = replay_actions(
            &replay,
            &actions,
            &mut CrashTarget::new(),
            false,
            None,
            None,
            Some(&manifest),
        )
        .unwrap();
        assert_eq!(replayed.as_ref(), Some(&expected));

        // Two-of-three confirmation over the actual signature path matches 3/3.
        struct SyntheticOracle {
            replay: ReplayConfig,
            manifest: SurfaceManifest,
        }
        impl ReplayOracle for SyntheticOracle {
            fn replay(&mut self, actions: &[Action]) -> Option<FailureSignature> {
                replay_actions(
                    &self.replay,
                    actions,
                    &mut CrashTarget::new(),
                    false,
                    Some(&self.replay.artifact_dir.join("minimize-attempts")),
                    None,
                    Some(&self.manifest),
                )
                .ok()
                .flatten()
            }
        }
        let mut oracle = SyntheticOracle {
            replay: replay.clone(),
            manifest,
        };
        let confirmation = confirm_two_of_three(&mut oracle, &actions, &expected);
        assert_eq!(confirmation.matches, 3);
    }

    #[test]
    fn replay_rejects_capability_drift_unless_best_effort() {
        let root = tempdir().unwrap();
        let replay = ReplayConfig {
            schema_version: FUZZ_SCHEMA_VERSION,
            target: target_spec("recorded-fingerprint".into()),
            capabilities: synthetic_capabilities(),
            fixture_digest: "f".into(),
            manifest_path: PathBuf::from("surface-manifest.json"),
            trace_path: PathBuf::from("trace.ndjson"),
            artifact_dir: root.path().join("run"),
            opt_ins: vec![],
        };
        let error = replay_actions(
            &replay,
            &[],
            &mut CrashTarget::new(),
            false,
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("capability fingerprint drift"),
            "{error}"
        );

        let outcome = replay_actions(
            &replay,
            &[],
            &mut CrashTarget::new(),
            true,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(outcome, None);
    }

    #[test]
    fn replay_re_evaluates_manifest_invariants() {
        let root = tempdir().unwrap();
        let mut manifest = boom_manifest();
        manifest.invariants.push(ManifestInvariant {
            id: "ready-invariant".into(),
            condition: Condition::Equals {
                path: "state.ready".into(),
                value: json!(true),
            },
            terminal: false,
        });
        let replay = ReplayConfig {
            schema_version: FUZZ_SCHEMA_VERSION,
            target: target_spec(synthetic_capabilities().fingerprint().unwrap()),
            capabilities: synthetic_capabilities(),
            fixture_digest: "f".into(),
            manifest_path: PathBuf::from("surface-manifest.json"),
            trace_path: PathBuf::from("trace.ndjson"),
            artifact_dir: root.path().join("run"),
            opt_ins: vec![],
        };
        let mut target = CrashTarget {
            ready: false,
            ..CrashTarget::new()
        };
        let signature = replay_actions(
            &replay,
            &[Action {
                schema_version: FUZZ_SCHEMA_VERSION,
                sequence: 1,
                id: "noop".into(),
                family: "state".into(),
                class: ActionClass::StateValid,
                precondition_id: None,
                precondition_satisfied: true,
                payload: ActionPayload::Wait { duration_ms: 1 },
                timeout_ms: 100,
                rng_cursor: 1,
            }],
            &mut target,
            false,
            None,
            None,
            Some(&manifest),
        )
        .unwrap()
        .expect("invariant violation must surface during replay");
        assert_eq!(signature.class, FailureClass::InvariantViolation);
        assert_eq!(signature.normalized, "ready-invariant");
    }

    #[test]
    fn load_replay_rejects_a_mismatched_schema_version() {
        let root = tempdir().unwrap();
        let replay = ReplayConfig {
            schema_version: FUZZ_SCHEMA_VERSION,
            target: target_spec(String::new()),
            capabilities: synthetic_capabilities(),
            fixture_digest: "f".into(),
            manifest_path: PathBuf::from("surface-manifest.json"),
            trace_path: PathBuf::from("trace.ndjson"),
            artifact_dir: root.path().join("run"),
            opt_ins: vec![],
        };
        let path = root.path().join("replay.toml");
        let text = toml::to_string_pretty(&replay).unwrap();
        fs::write(
            &path,
            text.replace("schema_version = 1", "schema_version = 99"),
        )
        .unwrap();
        let error = load_replay(&path).unwrap_err();
        assert!(error.to_string().contains("schema version 99"), "{error}");

        fs::write(&path, toml::to_string_pretty(&replay).unwrap()).unwrap();
        assert!(load_replay(&path).is_ok());
    }
}

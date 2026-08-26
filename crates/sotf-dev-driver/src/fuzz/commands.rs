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
use super::minimize::{ReplayOracle, confirm_two_of_three, minimize_actions};
use super::model::{
    Action, ActionClass, FailureClass, FailureSignature, ReplayConfig, TargetId, TraceEvent,
};
use super::report::RunSummary;
use super::supervisor::{
    FuzzConfig, FuzzRunResult, FuzzTarget, LaunchContext, TargetError, run_fuzz,
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
    PathBuf::from(format!(
        "crates/sotf-dev-driver/fuzz/{}.toml",
        target.as_str()
    ))
}

#[derive(Debug, Clone)]
pub struct ReplayCommandOptions {
    pub replay: PathBuf,
    pub executable: Option<PathBuf>,
    pub url: Option<String>,
    pub best_effort_capabilities: bool,
}

pub fn run_replay_command(options: &ReplayCommandOptions) -> Result<Option<FailureSignature>> {
    let replay = load_replay(&options.replay)?;
    let events = read_trace(&resolve_replay_path(&options.replay, &replay.trace_path))?;
    let actions = resolved_actions(&events);
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
    )
}

fn replay_actions(
    replay: &ReplayConfig,
    actions: &[Action],
    target: &mut dyn FuzzTarget,
    best_effort_capabilities: bool,
    attempt_root: Option<&Path>,
) -> Result<Option<FailureSignature>> {
    let parent = attempt_root.unwrap_or(&replay.artifact_dir);
    fs::create_dir_all(parent)?;
    let name = format!("replay-{}", Uuid::new_v4().simple());
    let store = ArtifactStore::create(parent, &name)?;
    for directory in ["qa", "tmp", "logs", "hang"] {
        store.create_dir(directory)?;
    }
    let raw_run_id = Uuid::new_v4().simple().to_string();
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
            Err(TargetError::Timeout(message)) => {
                let signature = FailureSignature {
                    class: FailureClass::CommandTimeout,
                    normalized: message,
                };
                let _ = target.shutdown();
                return Ok(Some(signature));
            }
            Err(error) => {
                let signature = FailureSignature {
                    class: FailureClass::UnexpectedExit,
                    normalized: error.to_string(),
                };
                let _ = target.shutdown();
                return Ok(Some(signature));
            }
        };
        let failure = observation_signature(action, &observation);
        trace.append(&TraceEvent::Observation {
            observation: Box::new(observation.clone()),
        })?;
        if let Some(next) = observation.snapshot {
            snapshot = next;
        }
        if failure.is_some() {
            let _ = target.shutdown();
            return Ok(failure);
        }
    }
    target.shutdown()?;
    Ok(None)
}

fn observation_signature(
    action: &Action,
    observation: &super::model::Observation,
) -> Option<FailureSignature> {
    if let Some(failure) = &observation.failure_candidate {
        return Some(failure.signature.clone());
    }
    if let Some(signal) = &observation.process.signal_or_exception {
        return Some(FailureSignature {
            class: FailureClass::SignalOrException,
            normalized: signal.clone(),
        });
    }
    if !observation.process.alive {
        return Some(FailureSignature {
            class: FailureClass::UnexpectedExit,
            normalized: format!("exit {:?}", observation.process.exit_code),
        });
    }
    if action.class == ActionClass::StateValid
        && action.precondition_satisfied
        && observation.reply.as_ref().is_some_and(|reply| !reply.ok)
    {
        return Some(FailureSignature {
            class: FailureClass::ValidActionRejection,
            normalized: action.id.clone(),
        });
    }
    observation.new_logs.iter().find_map(|line| {
        let lower = line.to_ascii_lowercase();
        (lower.contains("panic") || lower.contains("error")).then(|| FailureSignature {
            class: FailureClass::PanicOrErrorLog,
            normalized: line.clone(),
        })
    })
}

#[derive(Debug, Clone)]
pub struct MinimizeCommandOptions {
    pub replay: PathBuf,
    pub executable: Option<PathBuf>,
    pub url: Option<String>,
}

pub fn run_minimize_command(options: &MinimizeCommandOptions) -> Result<PathBuf> {
    let replay = load_replay(&options.replay)?;
    let summary_path = replay.artifact_dir.join("summary.json");
    let summary: RunSummary = serde_json::from_slice(
        &fs::read(&summary_path)
            .with_context(|| format!("reading failure summary {}", summary_path.display()))?,
    )?;
    let expected = summary
        .failures
        .first()
        .map(|failure| failure.signature.clone())
        .ok_or_else(|| anyhow!("recorded run has no failure to minimize"))?;
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
        replay: replay.clone(),
        executable: options
            .executable
            .clone()
            .or_else(|| replay.target.executable.clone()),
        url: options.url.clone(),
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
        )
        .ok()
        .flatten()
    }
}

fn load_replay(path: &Path) -> Result<ReplayConfig> {
    toml::from_str(
        &fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?,
    )
    .with_context(|| format!("parsing {}", path.display()))
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

#[allow(dead_code)]
fn _snapshot_type_check(_snapshot: Snapshot) {}

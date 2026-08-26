use std::collections::{BTreeSet, VecDeque};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::json;
use sotf_dev_api::{Capabilities, RunId, Snapshot};
use thiserror::Error;
use uuid::Uuid;

use super::artifact::{ArtifactError, ArtifactStore, Redactor};
use super::failure::{
    HangEvidence, ProbeResult, RetainedCountGrowth, classify_hang, classify_memory_growth,
    classify_retained_count_growth, normalize_signature,
};
use super::generator::{GENERATOR_VERSION, Generator, GeneratorError, derive_worker_seed};
use super::manifest::{ManifestError, SurfaceManifest, intersect_capabilities};
use super::model::{
    Action, ActionClass, FUZZ_SCHEMA_VERSION, Failure, FailureClass, FailureSignature, Observation,
    ProcessObservation, ReplayConfig, StructuredSkip, TargetId, TargetSpec, TraceEvent,
};
use super::report::{ReportError, RunSummary, write_reports};
use super::resource::{NullResourceSampler, ResourceSampler, SystemResourceSampler};
use super::trace::{TraceError, TraceWriter};

#[derive(Debug, Clone)]
pub struct FuzzConfig {
    pub target: TargetId,
    pub seed: u64,
    pub worker: u32,
    pub steps: u64,
    pub time_budget: Option<Duration>,
    pub fixture_profile: String,
    pub artifact_root: PathBuf,
    pub durable_trace: bool,
    pub opt_ins: BTreeSet<String>,
}

impl FuzzConfig {
    pub fn worker_seed(&self) -> u64 {
        derive_worker_seed(self.seed, self.worker)
    }
}

pub struct LaunchContext<'a> {
    pub run_id: &'a RunId,
    pub run_dir: &'a Path,
    pub fixture_profile: &'a str,
    pub opt_ins: &'a BTreeSet<String>,
}

pub trait FuzzTarget {
    fn target_id(&self) -> TargetId;
    fn launch(&mut self, context: &LaunchContext<'_>) -> Result<TargetSpec, TargetError>;
    fn capabilities(&mut self) -> Result<Capabilities, TargetError>;
    fn snapshot(&mut self) -> Result<Snapshot, TargetError>;
    fn execute(&mut self, action: &Action) -> Result<Observation, TargetError>;
    fn live(&mut self) -> Result<bool, TargetError>;
    fn pid(&self) -> Option<u32> {
        None
    }
    fn capture_hang(&mut self, _directory: &Path) -> Result<Vec<PathBuf>, TargetError> {
        Ok(Vec::new())
    }
    fn shutdown(&mut self) -> Result<(), TargetError>;
}

#[derive(Debug)]
pub struct FuzzRunResult {
    pub run_dir: PathBuf,
    pub summary: RunSummary,
}

pub fn run_fuzz<T: FuzzTarget + ?Sized>(
    config: &FuzzConfig,
    manifest: &SurfaceManifest,
    target: &mut T,
) -> Result<FuzzRunResult, SupervisorError> {
    if target.target_id() != config.target || manifest.target != config.target {
        return Err(SupervisorError::TargetMismatch {
            config: config.target,
            adapter: target.target_id(),
            manifest: manifest.target,
        });
    }
    manifest.validate()?;
    let raw_run_id = Uuid::new_v4().simple().to_string();
    let run_id = RunId::parse(raw_run_id.clone())?;
    let directory_name = format!("{}-{raw_run_id}", config.target);
    let store = ArtifactStore::create(&config.artifact_root, &directory_name)?;
    for directory in [
        "qa",
        "runtime",
        "library",
        "tmp",
        "logs",
        "snapshots",
        "screenshots",
        "crash",
        "hang",
        "leaks",
    ] {
        store.create_dir(directory)?;
    }
    let started = Instant::now();
    let launch_context = LaunchContext {
        run_id: &run_id,
        run_dir: store.run_dir(),
        fixture_profile: &config.fixture_profile,
        opt_ins: &config.opt_ins,
    };
    let mut target_spec = match target.launch(&launch_context) {
        Ok(spec) => spec,
        Err(TargetError::Unsupported(skip)) => {
            let summary = RunSummary {
                schema_version: FUZZ_SCHEMA_VERSION,
                target: config.target,
                seed: config.seed,
                outcome: "skipped".into(),
                steps: 0,
                duration_ms: started.elapsed().as_millis() as u64,
                failures: vec![],
                skip: Some(skip),
                coverage_keys: 0,
                opt_ins: config.opt_ins.iter().cloned().collect(),
            };
            let summary = portable_summary(&summary, &run_id)?;
            write_reports(store.run_dir(), &summary)?;
            return Ok(FuzzRunResult {
                run_dir: store.run_dir().to_owned(),
                summary,
            });
        }
        Err(error) => return Err(error.into()),
    };
    let capabilities = target.capabilities()?;
    target_spec.capability_fingerprint = capabilities.fingerprint()?;
    target_spec.run_id_hash = run_id.redacted_hash();
    fs::write(
        store.path("run.json")?,
        serde_json::to_vec_pretty(&json!({
            "schema_version": FUZZ_SCHEMA_VERSION,
            "target": target_spec,
            "seed": config.seed,
            "worker": config.worker,
            "worker_seed": config.worker_seed(),
            "fixture_profile": config.fixture_profile,
            "opt_ins": config.opt_ins,
        }))?,
    )?;
    fs::write(
        store.path("capabilities.json")?,
        serde_json::to_vec_pretty(&capabilities)?,
    )?;
    fs::write(
        store.path("surface-manifest.json")?,
        serde_json::to_vec_pretty(manifest)?,
    )?;
    let manifest_digest = manifest.digest()?;
    let fixture_digest = digest_text(&config.fixture_profile);
    let replay_config = ReplayConfig {
        schema_version: FUZZ_SCHEMA_VERSION,
        target: target_spec.clone(),
        capabilities: capabilities.clone(),
        fixture_digest: fixture_digest.clone(),
        manifest_path: PathBuf::from("surface-manifest.json"),
        trace_path: PathBuf::from("trace.ndjson"),
        artifact_dir: store.run_dir().to_owned(),
        opt_ins: config.opt_ins.iter().cloned().collect(),
    };
    fs::write(
        store.path("replay.toml")?,
        toml::to_string_pretty(&replay_config)?,
    )?;

    let mut trace = TraceWriter::create(store.path("trace.ndjson")?, config.durable_trace)?;
    trace.append(&TraceEvent::RunStart {
        schema_version: FUZZ_SCHEMA_VERSION,
        target: Box::new(target_spec.clone()),
        seed: config.seed,
        worker: config.worker,
        worker_seed: config.worker_seed(),
        generator_version: GENERATOR_VERSION,
        capabilities: Box::new(capabilities.clone()),
        manifest_digest,
        fixture_digest,
    })?;
    let intersection = intersect_capabilities(manifest, &capabilities);
    fs::write(
        store.path("coverage.json")?,
        serde_json::to_vec_pretty(&json!({
            "missing_manifest_actions": intersection.missing_manifest_actions,
            "unclassified_runtime_actions": intersection.unclassified_runtime_actions,
            "counters": {},
        }))?,
    )?;
    let mut generator = Generator::new(config.worker_seed());
    let mut snapshot = target.snapshot()?;
    let mut snapshots = VecDeque::with_capacity(32);
    snapshots.push_back(snapshot.clone());
    let mut resource_sampler: Box<dyn ResourceSampler> = match target.pid() {
        Some(pid) => Box::new(SystemResourceSampler::new(pid)),
        None => Box::new(NullResourceSampler::default()),
    };
    let metrics_path = store.path("metrics.ndjson")?;
    let mut metrics = BufWriter::new(File::create(&metrics_path)?);
    let mut samples = Vec::new();
    let mut failures = Vec::new();
    let mut completed_steps = 0;

    for _ in 0..config.steps {
        if config
            .time_budget
            .is_some_and(|budget| started.elapsed() >= budget)
        {
            break;
        }
        let action =
            match generator.next_action(manifest, &intersection.supported_actions, &snapshot) {
                Ok(action) => action,
                Err(GeneratorError::NoValidAction) => {
                    failures.push(make_failure(
                        config.target,
                        &target_spec.build_id,
                        FailureClass::InvariantViolation,
                        completed_steps,
                        "no state-valid or declared recovery action",
                        vec!["generator reached a state without a valid recovery".into()],
                    ));
                    break;
                }
                Err(error) => return Err(error.into()),
            };
        trace.append(&TraceEvent::ActionIntent {
            action: Box::new(action.clone()),
            preceding_revision: snapshot.state_revision,
            preceding_state_hash: snapshot.state_hash.clone(),
        })?;
        let (mut observation, execution_failed) = match target.execute(&action) {
            Ok(observation) => (observation, false),
            Err(TargetError::Timeout(message)) => {
                let failure = classify_timeout(target, &store, &action, &target_spec, message)?;
                let observation = error_observation(&action, failure.clone());
                (observation, true)
            }
            Err(error) => {
                let failure = make_failure(
                    config.target,
                    &target_spec.build_id,
                    FailureClass::UnexpectedExit,
                    action.sequence,
                    &error.to_string(),
                    vec![error.to_string()],
                );
                let observation = error_observation(&action, failure.clone());
                (observation, true)
            }
        };
        let resource = resource_sampler.sample();
        serde_json::to_writer(&mut metrics, &resource)?;
        metrics.write_all(b"\n")?;
        metrics.flush()?;
        samples.push(resource.clone());
        observation.resource = Some(resource);
        collect_observation_failures(
            config.target,
            &target_spec.build_id,
            &action,
            &observation,
            &mut failures,
        );
        trace.append(&TraceEvent::Observation {
            observation: Box::new(observation.clone()),
        })?;
        completed_steps = action.sequence;

        // The execution failure is already preserved in the trace. Avoid a
        // second probe that would promote an expected target failure into a
        // top-level supervisor error and prevent report generation.
        if execution_failed || !observation.process.alive {
            break;
        }

        let next_snapshot = match observation.snapshot {
            Some(next_snapshot) => next_snapshot,
            None => match target.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    failures.push(make_failure(
                        config.target,
                        &target_spec.build_id,
                        FailureClass::UnexpectedExit,
                        action.sequence,
                        &error.to_string(),
                        vec![error.to_string()],
                    ));
                    break;
                }
            },
        };
        check_revisions_and_invariants(
            target,
            manifest,
            &snapshot,
            &next_snapshot,
            config.target,
            &target_spec.build_id,
            action.sequence,
            &mut failures,
        )?;
        snapshot = next_snapshot;
        if snapshots.len() == 32 {
            snapshots.pop_front();
        }
        snapshots.push_back(snapshot.clone());
        generator
            .coverage
            .record_state_hash(snapshot.state_hash.clone());
        generator.coverage.record(
            std::iter::once(format!("action:{}", action.id))
                .chain(std::iter::once(format!("family:{}", action.family))),
        );
        if !failures.is_empty() {
            break;
        }
    }

    if let Some(memory) = classify_memory_growth(&samples)
        && memory.suspected
    {
        fs::write(
            store.path("leaks/portable-memory.json")?,
            serde_json::to_vec_pretty(&json!({
                "slope_bytes_per_minute": memory.slope_bytes_per_minute,
                "baseline_bytes": memory.baseline_bytes,
                "final_bytes": memory.final_bytes,
                "retained_growth_bytes": memory.retained_growth_bytes,
                "retained_threshold_bytes": memory.retained_threshold_bytes,
            }))?,
        )?;
        failures.push(make_failure(
            config.target,
            &target_spec.build_id,
            FailureClass::SuspectedMemoryLeak,
            completed_steps,
            "portable RSS growth",
            vec![format!(
                "slope={:.0} B/min retained={} B threshold={} B",
                memory.slope_bytes_per_minute,
                memory.retained_growth_bytes,
                memory.retained_threshold_bytes
            )],
        ));
    }

    let resource_leaks = classify_resource_leaks(config.target, &samples);
    if !resource_leaks.is_empty() {
        let evidence: Vec<_> = resource_leaks
            .iter()
            .map(|leak| {
                json!({
                    "resource": leak.resource,
                    "baseline": leak.growth.baseline,
                    "final": leak.growth.final_value,
                    "retained_growth": leak.growth.retained_growth,
                    "absolute_budget": leak.growth.absolute_budget,
                })
            })
            .collect();
        fs::write(
            store.path("leaks/portable-resource-counts.json")?,
            serde_json::to_vec_pretty(&evidence)?,
        )?;
        for leak in resource_leaks {
            failures.push(make_failure(
                config.target,
                &target_spec.build_id,
                FailureClass::ResourceLeak,
                completed_steps,
                leak.resource,
                vec![format!(
                    "{} baseline={} final={} retained={} budget={}",
                    leak.resource,
                    leak.growth.baseline,
                    leak.growth.final_value,
                    leak.growth.retained_growth,
                    leak.growth.absolute_budget,
                )],
            ));
        }
    }
    for (index, snapshot) in snapshots.iter().enumerate() {
        fs::write(
            store.path(format!("snapshots/recent-{index:02}.json"))?,
            serde_json::to_vec_pretty(snapshot)?,
        )?;
    }
    fs::write(
        store.path("coverage.json")?,
        serde_json::to_vec_pretty(&json!({
            "missing_manifest_actions": intersection.missing_manifest_actions,
            "unclassified_runtime_actions": intersection.unclassified_runtime_actions,
            "counters": generator.coverage.counters(),
        }))?,
    )?;
    let shutdown = target.shutdown();
    let graceful = shutdown.is_ok();
    if let Err(error) = shutdown {
        failures.push(make_failure(
            config.target,
            &target_spec.build_id,
            FailureClass::CleanupInvariant,
            completed_steps,
            "target_cleanup",
            vec![error.to_string()],
        ));
    }
    trace.append(&TraceEvent::Cleanup {
        graceful,
        details: if graceful {
            "target stopped cleanly".into()
        } else {
            "target required adapter cleanup".into()
        },
    })?;
    trace.append(&TraceEvent::RunEnd {
        outcome: if failures.is_empty() {
            "passed".into()
        } else {
            "failed".into()
        },
        steps: completed_steps,
    })?;
    let summary = RunSummary {
        schema_version: FUZZ_SCHEMA_VERSION,
        target: config.target,
        seed: config.seed,
        outcome: if failures.is_empty() {
            "passed".into()
        } else {
            "failed".into()
        },
        steps: completed_steps,
        duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        failures,
        skip: None,
        coverage_keys: generator.coverage.counters().len(),
        opt_ins: config.opt_ins.iter().cloned().collect(),
    };
    let summary = portable_summary(&summary, &run_id)?;
    write_reports(store.run_dir(), &summary)?;
    Ok(FuzzRunResult {
        run_dir: store.run_dir().to_owned(),
        summary,
    })
}

fn portable_summary(summary: &RunSummary, run_id: &RunId) -> Result<RunSummary, serde_json::Error> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let redactor = Redactor::new(home.as_deref(), [run_id.as_str().to_owned()]);
    serde_json::from_value(redactor.json(&serde_json::to_value(summary)?))
}

struct ResourceLeakEvidence {
    resource: &'static str,
    growth: RetainedCountGrowth,
}

fn classify_resource_leaks(
    target: TargetId,
    samples: &[super::model::ResourceSample],
) -> Vec<ResourceLeakEvidence> {
    let (thread_budget, descriptor_budget, child_budget) = match target {
        TargetId::PlayerCli | TargetId::RecorderCli => (2, 4, 0),
        TargetId::Tui | TargetId::Configbar => (4, 8, 0),
        TargetId::DesktopGpui
        | TargetId::IosSim
        | TargetId::TvosSim
        | TargetId::HeadlessServer
        | TargetId::SystemwideDaemon => (8, 16, 0),
    };
    let fields = [
        (
            "threads",
            samples
                .iter()
                .filter_map(|sample| sample.threads)
                .collect::<Vec<_>>(),
            thread_budget,
        ),
        (
            "descriptors_or_handles",
            samples
                .iter()
                .filter_map(|sample| sample.descriptors_or_handles)
                .collect::<Vec<_>>(),
            descriptor_budget,
        ),
        (
            "children",
            samples
                .iter()
                .filter_map(|sample| sample.children)
                .collect::<Vec<_>>(),
            child_budget,
        ),
    ];

    fields
        .into_iter()
        .filter_map(|(resource, values, budget)| {
            classify_retained_count_growth(&values, budget)
                .filter(|growth| growth.suspected)
                .map(|growth| ResourceLeakEvidence { resource, growth })
        })
        .collect()
}

fn collect_observation_failures(
    target: TargetId,
    build_id: &str,
    action: &Action,
    observation: &Observation,
    failures: &mut Vec<Failure>,
) {
    if !observation.process.alive {
        failures.push(make_failure(
            target,
            build_id,
            FailureClass::UnexpectedExit,
            action.sequence,
            "process exited unexpectedly",
            vec![],
        ));
    }
    if let Some(signal) = &observation.process.signal_or_exception {
        failures.push(make_failure(
            target,
            build_id,
            FailureClass::SignalOrException,
            action.sequence,
            signal,
            vec![signal.clone()],
        ));
    }
    if action.class == ActionClass::StateValid
        && action.precondition_satisfied
        && observation.reply.as_ref().is_some_and(|reply| !reply.ok)
    {
        failures.push(make_failure(
            target,
            build_id,
            FailureClass::ValidActionRejection,
            action.sequence,
            &action.id,
            observation
                .reply
                .as_ref()
                .and_then(|reply| reply.error.clone())
                .into_iter()
                .collect(),
        ));
    }
    if let Some(line) = observation.new_logs.iter().find(|line| {
        let lower = line.to_ascii_lowercase();
        lower.contains("panic") || lower.contains("error")
    }) {
        failures.push(make_failure(
            target,
            build_id,
            FailureClass::PanicOrErrorLog,
            action.sequence,
            line,
            vec![line.clone()],
        ));
    }
    if let Some(candidate) = &observation.failure_candidate {
        failures.push(candidate.clone());
    }
}

#[allow(clippy::too_many_arguments)]
fn check_revisions_and_invariants<T: FuzzTarget + ?Sized>(
    target: &mut T,
    manifest: &SurfaceManifest,
    previous: &Snapshot,
    current: &Snapshot,
    target_id: TargetId,
    build_id: &str,
    sequence: u64,
    failures: &mut Vec<Failure>,
) -> Result<(), SupervisorError> {
    if current.state_revision < previous.state_revision {
        failures.push(make_failure(
            target_id,
            build_id,
            FailureClass::InvariantViolation,
            sequence,
            "state_revision_monotonic",
            vec![format!(
                "revision regressed {} -> {}",
                previous.state_revision, current.state_revision
            )],
        ));
    }
    for invariant in &manifest.invariants {
        if invariant.condition.evaluate(current) {
            continue;
        }
        let confirmed = if invariant.terminal {
            true
        } else {
            let confirmation = target.snapshot()?;
            !invariant.condition.evaluate(&confirmation)
        };
        if confirmed {
            failures.push(make_failure(
                target_id,
                build_id,
                FailureClass::InvariantViolation,
                sequence,
                &invariant.id,
                vec![format!("invariant {} evaluated false", invariant.id)],
            ));
        }
    }
    Ok(())
}

fn classify_timeout<T: FuzzTarget + ?Sized>(
    target: &mut T,
    store: &ArtifactStore,
    action: &Action,
    spec: &TargetSpec,
    message: String,
) -> Result<Failure, SupervisorError> {
    let mut live_result = ProbeResult::TimedOut;
    let mut snapshot_result = ProbeResult::TimedOut;
    let mut misses = 0;
    for _ in 0..3 {
        live_result = match target.live() {
            Ok(true) => ProbeResult::Responsive,
            Ok(false) | Err(TargetError::Timeout(_)) => ProbeResult::TimedOut,
            Err(TargetError::ProcessExited(_)) => ProbeResult::ProcessExited,
            Err(_) => ProbeResult::TimedOut,
        };
        snapshot_result = match target.snapshot() {
            Ok(_) => ProbeResult::Responsive,
            Err(TargetError::ProcessExited(_)) => ProbeResult::ProcessExited,
            Err(_) => ProbeResult::TimedOut,
        };
        if matches!(live_result, ProbeResult::TimedOut)
            || matches!(snapshot_result, ProbeResult::TimedOut)
        {
            misses += 1;
        }
    }
    let class = classify_hang(HangEvidence {
        action_timed_out: true,
        live: live_result,
        snapshot: snapshot_result,
        consecutive_misses: misses,
        process_progressed: false,
    })
    .unwrap_or(FailureClass::CommandTimeout);
    let paths = target.capture_hang(&store.path("hang")?)?;
    Ok(Failure {
        schema_version: FUZZ_SCHEMA_VERSION,
        class,
        signature: FailureSignature {
            class,
            normalized: normalize_signature(&message, store.run_dir().to_str()),
        },
        first_sequence: action.sequence,
        evidence: vec![message],
        target_id: spec.target_id,
        build_id: spec.build_id.clone(),
        confirmations: 1,
        artifacts: paths,
    })
}

fn make_failure(
    target_id: TargetId,
    build_id: &str,
    class: FailureClass,
    sequence: u64,
    signature: &str,
    evidence: Vec<String>,
) -> Failure {
    Failure {
        schema_version: FUZZ_SCHEMA_VERSION,
        class,
        signature: FailureSignature {
            class,
            normalized: normalize_signature(signature, None),
        },
        first_sequence: sequence,
        evidence,
        target_id,
        build_id: build_id.to_owned(),
        confirmations: 1,
        artifacts: Vec::new(),
    }
}

fn error_observation(action: &Action, failure: Failure) -> Observation {
    Observation {
        schema_version: FUZZ_SCHEMA_VERSION,
        sequence: action.sequence,
        reply: None,
        snapshot: None,
        process: ProcessObservation {
            pid: None,
            alive: true,
            exit_code: None,
            signal_or_exception: None,
        },
        resource: None,
        new_logs: vec![],
        crash_files: vec![],
        coverage: Default::default(),
        screenshot: None,
        failure_candidate: Some(failure),
    }
}

fn digest_text(text: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(text.as_bytes()))
}

#[derive(Debug, Error)]
pub enum TargetError {
    #[error("target is unsupported: {0:?}")]
    Unsupported(StructuredSkip),
    #[error("target command timed out: {0}")]
    Timeout(String),
    #[error("target process exited: {0}")]
    ProcessExited(String),
    #[error("target protocol failed: {0}")]
    Protocol(String),
    #[error("target I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("target JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("target HTTP failed: {0}")]
    Http(#[from] reqwest::Error),
}

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("fuzz target mismatch: config={config}, adapter={adapter}, manifest={manifest}")]
    TargetMismatch {
        config: TargetId,
        adapter: TargetId,
        manifest: TargetId,
    },
    #[error(transparent)]
    Target(#[from] TargetError),
    #[error(transparent)]
    Artifact(#[from] ArtifactError),
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error(transparent)]
    Generator(#[from] GeneratorError),
    #[error(transparent)]
    Trace(#[from] TraceError),
    #[error(transparent)]
    Report(#[from] ReportError),
    #[error("run ID failed: {0}")]
    RunId(#[from] sotf_dev_api::RunIdError),
    #[error("JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("TOML serialization failed: {0}")]
    Toml(#[from] toml::ser::Error),
    #[error("I/O failed: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use serde_json::{Value, json};
    use sotf_dev_api::DevReply;
    use tempfile::tempdir;

    use super::*;
    use crate::fuzz::manifest::{Condition, ManifestAction};
    use crate::fuzz::model::{ActionPayload, AdapterKind, CoverageDelta, EndpointSpec};

    struct SyntheticTarget {
        revision: u64,
        launched: bool,
        exit_after_execute: bool,
        exited: bool,
    }

    impl FuzzTarget for SyntheticTarget {
        fn target_id(&self) -> TargetId {
            TargetId::Tui
        }

        fn launch(&mut self, _context: &LaunchContext<'_>) -> Result<TargetSpec, TargetError> {
            self.launched = true;
            Ok(TargetSpec {
                schema_version: 1,
                target_id: TargetId::Tui,
                adapter: AdapterKind::Synthetic,
                executable: None,
                app_identity: Some("synthetic".into()),
                platform: std::env::consts::OS.into(),
                fixture_profile: "none".into(),
                environment_names: vec![],
                endpoints: vec![EndpointSpec {
                    name: "synthetic".into(),
                    address: "in-memory".into(),
                    protocol: "synthetic".into(),
                }],
                run_id_hash: String::new(),
                capability_fingerprint: String::new(),
                build_id: "test-build".into(),
            })
        }

        fn capabilities(&mut self) -> Result<Capabilities, TargetError> {
            let mut capabilities = Capabilities::new("tui", "synthetic");
            capabilities.actions.push(sotf_dev_api::NamedCapability {
                name: "advance".into(),
                family: "state".into(),
                payload_schema: None,
            });
            Ok(capabilities)
        }

        fn snapshot(&mut self) -> Result<Snapshot, TargetError> {
            if self.exited {
                return Err(TargetError::Io(std::io::Error::new(
                    std::io::ErrorKind::ConnectionRefused,
                    "synthetic target exited",
                )));
            }
            Ok(Snapshot::new("tui", self.revision, json!({"ready": true})).unwrap())
        }

        fn execute(&mut self, action: &Action) -> Result<Observation, TargetError> {
            self.revision += 1;
            if self.exit_after_execute {
                self.exited = true;
            }
            Ok(Observation {
                schema_version: 1,
                sequence: action.sequence,
                reply: Some(DevReply::success(Value::Null)),
                snapshot: if self.exited {
                    None
                } else {
                    Some(self.snapshot()?)
                },
                process: ProcessObservation {
                    pid: None,
                    alive: true,
                    exit_code: None,
                    signal_or_exception: None,
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
            Ok(self.launched)
        }

        fn shutdown(&mut self) -> Result<(), TargetError> {
            self.launched = false;
            Ok(())
        }
    }

    #[test]
    fn writes_a_complete_hermetic_synthetic_run() {
        let root = tempdir().unwrap();
        let manifest = SurfaceManifest {
            schema_version: 1,
            version: 1,
            target: TargetId::Tui,
            fixture_profiles: vec!["none".into()],
            actions: vec![ManifestAction {
                id: "advance".into(),
                family: "state".into(),
                weight: 100,
                precondition_id: Some("ready".into()),
                precondition: Condition::Equals {
                    path: "state.ready".into(),
                    value: json!(true),
                },
                recovery: false,
                chaos_only: false,
                payload: ActionPayload::DevAction {
                    name: "advance".into(),
                    payload: Value::Null,
                },
                timeout_ms: 100,
                coverage: vec!["state:advanced".into()],
            }],
            invariants: vec![],
            workflows: vec![],
        };
        let config = FuzzConfig {
            target: TargetId::Tui,
            seed: 42,
            worker: 0,
            steps: 5,
            time_budget: None,
            fixture_profile: "none".into(),
            artifact_root: root.path().into(),
            durable_trace: false,
            opt_ins: BTreeSet::new(),
        };
        let result = run_fuzz(
            &config,
            &manifest,
            &mut SyntheticTarget {
                revision: 0,
                launched: false,
                exit_after_execute: false,
                exited: false,
            },
        )
        .unwrap();
        assert_eq!(result.summary.outcome, "passed");
        assert_eq!(result.summary.steps, 5);
        for file in [
            "run.json",
            "capabilities.json",
            "trace.ndjson",
            "metrics.ndjson",
            "coverage.json",
            "replay.toml",
            "summary.json",
            "junit.xml",
            "summary.html",
        ] {
            assert!(result.run_dir.join(file).is_file(), "missing {file}");
        }
    }

    #[test]
    fn target_exit_still_finalizes_reports_and_trace() {
        let root = tempdir().unwrap();
        let manifest = SurfaceManifest {
            schema_version: 1,
            version: 1,
            target: TargetId::Tui,
            fixture_profiles: vec!["none".into()],
            actions: vec![ManifestAction {
                id: "advance".into(),
                family: "state".into(),
                weight: 100,
                precondition_id: None,
                precondition: Condition::Always,
                recovery: false,
                chaos_only: false,
                payload: ActionPayload::DevAction {
                    name: "advance".into(),
                    payload: Value::Null,
                },
                timeout_ms: 100,
                coverage: vec![],
            }],
            invariants: vec![],
            workflows: vec![],
        };
        let config = FuzzConfig {
            target: TargetId::Tui,
            seed: 7,
            worker: 0,
            steps: 3,
            time_budget: None,
            fixture_profile: "none".into(),
            artifact_root: root.path().into(),
            durable_trace: false,
            opt_ins: BTreeSet::new(),
        };

        let result = run_fuzz(
            &config,
            &manifest,
            &mut SyntheticTarget {
                revision: 0,
                launched: false,
                exit_after_execute: true,
                exited: false,
            },
        )
        .unwrap();

        assert_eq!(result.summary.outcome, "failed");
        assert_eq!(result.summary.steps, 1);
        assert!(result.run_dir.join("summary.json").is_file());
        assert!(result.run_dir.join("replay.toml").is_file());
        let trace = fs::read_to_string(result.run_dir.join("trace.ndjson")).unwrap();
        assert!(trace.contains("\"event\":\"run_end\""));
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod desktop {
    use std::path::PathBuf;
    use std::time::Duration;

    use clap::Parser;
    use sotf_host::{
        ExternalPlugin, ExternalPluginSandboxMode, ExternalPluginSandboxPolicy,
        ExternalPluginSandboxStatus, ExternalPluginSandboxTiming, ExternalPluginState,
        ExternalPluginWorker, ExternalPluginWorkerStep, PluginDescriptor, PluginSandboxBackendCode,
        PluginSandboxPolicy, PluginSandboxStatusCode, SecurePluginSharedMemory,
        enter_external_plugin_sandbox,
    };

    #[derive(Debug, Parser)]
    #[command(
        name = "sotf-external-plugin-worker",
        about = "Isolated SOTF external audio plugin worker"
    )]
    pub struct Args {
        /// Path to the secure shared-memory segment created by the host.
        #[arg(long)]
        shared_memory: Option<PathBuf>,

        /// External plugin descriptor as JSON.
        #[arg(long, conflicts_with = "descriptor_file")]
        descriptor_json: Option<String>,

        /// Path to a JSON file containing the external plugin descriptor.
        #[arg(long, conflicts_with = "descriptor_json")]
        descriptor_file: Option<PathBuf>,

        /// Path to the validated external-plugin state envelope to restore.
        #[arg(long)]
        external_state_file: Option<PathBuf>,

        /// Process at most one available block, then exit.
        #[arg(long)]
        once: bool,

        /// Sleep duration when no block is ready.
        #[arg(long, default_value_t = 0)]
        idle_sleep_micros: u64,

        /// When to enter the worker sandbox.
        #[arg(long, default_value = "before-plugin-load")]
        sandbox_timing: String,

        /// Portable sandbox policy as JSON.
        #[arg(long, conflicts_with = "sandbox_policy_file")]
        sandbox_policy_json: Option<String>,

        /// Path to a JSON file containing the portable sandbox policy.
        #[arg(long, conflicts_with = "sandbox_policy_json")]
        sandbox_policy_file: Option<PathBuf>,

        /// Treat missing platform sandbox support as a worker startup error.
        #[arg(long)]
        sandbox_required: bool,

        /// Allow network access inside the sandbox.
        #[arg(long)]
        sandbox_allow_network: bool,

        /// Allow child process creation inside the sandbox when supported.
        #[arg(long)]
        sandbox_allow_child_processes: bool,

        /// Additional read/execute path to allow inside the sandbox.
        #[arg(long = "sandbox-read-path")]
        sandbox_read_paths: Vec<PathBuf>,

        /// Additional read/write path to allow inside the sandbox.
        #[arg(long = "sandbox-write-path")]
        sandbox_write_paths: Vec<PathBuf>,
    }

    pub fn main() {
        let args = Args::parse();
        if let Err(err) = run(args) {
            eprintln!("sotf-external-plugin-worker: {err}");
            std::process::exit(1);
        }
    }

    fn run(args: Args) -> Result<(), String> {
        let descriptor = load_descriptor(&args)?;
        let external_state = load_external_state(&args)?;
        if let Some(state) = external_state.as_ref()
            && state.descriptor != descriptor
        {
            return Err(format!(
                "external plugin state targets '{}' at {}, not '{}' at {}",
                state.descriptor.id,
                state.descriptor.path.display(),
                descriptor.id,
                descriptor.path.display()
            ));
        }
        let shared_memory = shared_memory_path(&args)?;
        let shared = SecurePluginSharedMemory::open_existing(&shared_memory)
            .map_err(|err| format!("failed to open shared memory: {err}"))?;
        let sample_rate = shared.layout().sample_rate;
        let max_block_frames = shared.layout().max_frames as usize;

        let sandbox_policy = sandbox_policy(&args)?;
        if sandbox_policy.timing == ExternalPluginSandboxTiming::BeforePluginLoad {
            let status =
                enter_external_plugin_sandbox(&sandbox_policy, &descriptor, &shared_memory)
                    .map_err(|err| format!("failed to enter pre-load worker sandbox: {err}"))?;
            publish_sandbox_runtime_status(&shared, &status);
        }

        let worker_state = external_state
            .as_ref()
            .map(worker_restore_state)
            .transpose()?;
        let plugin = match worker_state.as_ref() {
            Some(state) if !state.opaque_state.is_empty() => {
                ExternalPlugin::from_placeholder_state_with_max_block_frames(
                    state,
                    sample_rate,
                    max_block_frames,
                )
            }
            _ => ExternalPlugin::new_with_max_block_frames(
                &descriptor,
                sample_rate,
                max_block_frames,
            ),
        }
        .map_err(|err| format!("failed to create external plugin wrapper: {err}"))?;

        if sandbox_policy.timing == ExternalPluginSandboxTiming::AfterPluginLoad {
            let status =
                enter_external_plugin_sandbox(&sandbox_policy, &descriptor, &shared_memory)
                    .map_err(|err| format!("failed to enter post-load worker sandbox: {err}"))?;
            publish_sandbox_runtime_status(&shared, &status);
        }
        if sandbox_policy.timing == ExternalPluginSandboxTiming::Disabled {
            publish_sandbox_runtime_status(&shared, &ExternalPluginSandboxStatus::Disabled);
        }

        let mut worker = ExternalPluginWorker::new(shared, Box::new(plugin))?;
        let idle_sleep = Duration::from_micros(args.idle_sleep_micros);

        loop {
            match worker.process_one()? {
                ExternalPluginWorkerStep::Processed { .. } => {
                    if args.once {
                        return Ok(());
                    }
                }
                ExternalPluginWorkerStep::Controlled => {}
                ExternalPluginWorkerStep::NoRequest => {
                    if args.once {
                        return Ok(());
                    }
                    if idle_sleep.is_zero() {
                        std::thread::yield_now();
                    } else {
                        std::thread::sleep(idle_sleep);
                    }
                }
            }
        }
    }

    fn worker_restore_state(state: &ExternalPluginState) -> Result<ExternalPluginState, String> {
        state.validate()?;
        if state.sandbox_mode != ExternalPluginSandboxMode::Isolated {
            return Err(format!(
                "external plugin worker expected isolated state, got {:?}",
                state.sandbox_mode
            ));
        }
        Ok(ExternalPluginState::new(
            state.descriptor.clone(),
            ExternalPluginSandboxMode::InProcess,
            state.opaque_state.clone(),
        ))
    }

    fn sandbox_policy(args: &Args) -> Result<ExternalPluginSandboxPolicy, String> {
        if let Some(json) = &args.sandbox_policy_json {
            return parse_sandbox_policy_json(json).and_then(portable_sandbox_policy_to_legacy);
        }

        if let Some(path) = &args.sandbox_policy_file {
            let json = std::fs::read_to_string(path).map_err(|err| {
                format!(
                    "failed to read external plugin sandbox policy file '{}': {err}",
                    path.display()
                )
            })?;
            return parse_sandbox_policy_json(&json).and_then(portable_sandbox_policy_to_legacy);
        }

        let timing = args.sandbox_timing.parse::<ExternalPluginSandboxTiming>()?;
        Ok(ExternalPluginSandboxPolicy {
            timing,
            require_platform_sandbox: args.sandbox_required,
            allow_network: args.sandbox_allow_network,
            allow_child_processes: args.sandbox_allow_child_processes,
            extra_read_paths: args.sandbox_read_paths.clone(),
            extra_write_paths: args.sandbox_write_paths.clone(),
        })
    }

    fn parse_sandbox_policy_json(json: &str) -> Result<PluginSandboxPolicy, String> {
        serde_json::from_str(json)
            .map_err(|err| format!("failed to parse external plugin sandbox policy JSON: {err}"))
    }

    fn portable_sandbox_policy_to_legacy(
        policy: PluginSandboxPolicy,
    ) -> Result<ExternalPluginSandboxPolicy, String> {
        policy.validate_legacy_worker_adapter()?;
        Ok(policy.to_legacy_policy())
    }

    fn load_descriptor(args: &Args) -> Result<PluginDescriptor, String> {
        if let Some(json) = &args.descriptor_json {
            return parse_descriptor_json(json);
        }

        if let Some(path) = &args.descriptor_file {
            let json = std::fs::read_to_string(path).map_err(|err| {
                format!(
                    "failed to read external plugin descriptor file '{}': {err}",
                    path.display()
                )
            })?;
            return parse_descriptor_json(&json);
        }

        Err("missing --descriptor-json or --descriptor-file".to_string())
    }

    fn load_external_state(args: &Args) -> Result<Option<ExternalPluginState>, String> {
        let Some(path) = args.external_state_file.as_ref() else {
            return Ok(None);
        };
        let bytes = std::fs::read(path).map_err(|error| {
            format!(
                "failed to read external plugin state '{}': {error}",
                path.display()
            )
        })?;
        let state: ExternalPluginState = serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "failed to parse external plugin state '{}': {error}",
                path.display()
            )
        })?;
        state.validate()?;
        Ok(Some(state))
    }

    fn shared_memory_path(args: &Args) -> Result<PathBuf, String> {
        if let Some(path) = &args.shared_memory {
            return Ok(path.clone());
        }

        std::env::var_os("SOTF_PLUGIN_SHARED_MEMORY")
            .map(PathBuf::from)
            .ok_or_else(|| {
                "missing --shared-memory or SOTF_PLUGIN_SHARED_MEMORY environment variable"
                    .to_string()
            })
    }

    fn parse_descriptor_json(json: &str) -> Result<PluginDescriptor, String> {
        serde_json::from_str(json)
            .map_err(|err| format!("failed to parse external plugin descriptor JSON: {err}"))
    }

    fn publish_sandbox_runtime_status(
        shared: &SecurePluginSharedMemory,
        status: &ExternalPluginSandboxStatus,
    ) {
        let (status_code, backend_code) = sandbox_status_codes(status);
        shared.publish_worker_sandbox_status(status_code, backend_code);
    }

    fn sandbox_status_codes(
        status: &ExternalPluginSandboxStatus,
    ) -> (PluginSandboxStatusCode, PluginSandboxBackendCode) {
        match status {
            ExternalPluginSandboxStatus::Disabled => (
                PluginSandboxStatusCode::Disabled,
                PluginSandboxBackendCode::Unknown,
            ),
            ExternalPluginSandboxStatus::Enforced { backend } => (
                PluginSandboxStatusCode::Enforced,
                sandbox_backend_code(backend),
            ),
            ExternalPluginSandboxStatus::Unsupported { backend, .. } => (
                PluginSandboxStatusCode::Unsupported,
                sandbox_backend_code(backend),
            ),
        }
    }

    fn sandbox_backend_code(backend: &str) -> PluginSandboxBackendCode {
        match backend {
            "linux-landlock" => PluginSandboxBackendCode::LinuxLandlock,
            "macos-app-sandbox-helper" => PluginSandboxBackendCode::MacosAppSandboxHelper,
            "macos-process-isolation" => PluginSandboxBackendCode::MacosProcessIsolation,
            "windows-process-isolation" => PluginSandboxBackendCode::WindowsProcessIsolation,
            _ => PluginSandboxBackendCode::Unknown,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use sotf_host::{ExternalPluginSandboxMode, PluginFormat};
        use std::path::Path;

        #[test]
        fn worker_default_does_not_sleep_for_one_millisecond_between_requests() {
            let args = Args::try_parse_from(["worker", "--once"]).unwrap();
            assert_eq!(args.idle_sleep_micros, 0);
        }

        fn args_with_external_state_file(path: Option<PathBuf>) -> Args {
            Args {
                shared_memory: Some(PathBuf::from("/tmp/fake.shm")),
                descriptor_json: None,
                descriptor_file: None,
                external_state_file: path,
                once: true,
                idle_sleep_micros: 1,
                sandbox_timing: "disabled".to_string(),
                sandbox_policy_json: None,
                sandbox_policy_file: None,
                sandbox_required: false,
                sandbox_allow_network: false,
                sandbox_allow_child_processes: false,
                sandbox_read_paths: Vec::new(),
                sandbox_write_paths: Vec::new(),
            }
        }

        fn state_descriptor(path: &Path) -> PluginDescriptor {
            parse_descriptor_json(
                &serde_json::json!({
                    "id": "clap.fake",
                    "name": "Fake",
                    "vendor": "Test",
                    "version": "1.0",
                    "format": "Clap",
                    "path": path,
                    "audio_inputs": 2,
                    "audio_outputs": 2,
                    "is_instrument": false,
                    "categories": [],
                })
                .to_string(),
            )
            .unwrap()
        }

        #[test]
        fn parse_descriptor_json_accepts_descriptor() {
            let json = r#"{
                "id": "clap.fake",
                "name": "Fake",
                "vendor": "Test",
                "version": "1.0",
                "format": "Clap",
                "path": "/tmp/fake.clap",
                "audio_inputs": 2,
                "audio_outputs": 2,
                "is_instrument": false,
                "categories": []
            }"#;

            let descriptor = parse_descriptor_json(json).unwrap();
            assert_eq!(descriptor.name, "Fake");
            assert_eq!(descriptor.format, PluginFormat::Clap);
            assert_eq!(descriptor.audio_inputs, 2);
            assert_eq!(descriptor.audio_outputs, 2);
        }

        #[test]
        fn load_descriptor_requires_descriptor_source() {
            let args = Args {
                shared_memory: Some(PathBuf::from("/tmp/fake.shm")),
                descriptor_json: None,
                descriptor_file: None,
                external_state_file: None,
                once: true,
                idle_sleep_micros: 1,
                sandbox_timing: "disabled".to_string(),
                sandbox_policy_json: None,
                sandbox_policy_file: None,
                sandbox_required: false,
                sandbox_allow_network: false,
                sandbox_allow_child_processes: false,
                sandbox_read_paths: Vec::new(),
                sandbox_write_paths: Vec::new(),
            };

            assert!(load_descriptor(&args).unwrap_err().contains("missing"));
        }

        #[test]
        fn shared_memory_path_prefers_argument() {
            let args = Args {
                shared_memory: Some(PathBuf::from("/tmp/from-arg.shm")),
                descriptor_json: None,
                descriptor_file: None,
                external_state_file: None,
                once: true,
                idle_sleep_micros: 1,
                sandbox_timing: "disabled".to_string(),
                sandbox_policy_json: None,
                sandbox_policy_file: None,
                sandbox_required: false,
                sandbox_allow_network: false,
                sandbox_allow_child_processes: false,
                sandbox_read_paths: Vec::new(),
                sandbox_write_paths: Vec::new(),
            };

            assert_eq!(
                shared_memory_path(&args).unwrap(),
                PathBuf::from("/tmp/from-arg.shm")
            );
        }

        #[test]
        fn sandbox_policy_parses_pre_load_required_mode() {
            let args = Args {
                shared_memory: Some(PathBuf::from("/tmp/fake.shm")),
                descriptor_json: None,
                descriptor_file: None,
                external_state_file: None,
                once: true,
                idle_sleep_micros: 1,
                sandbox_timing: "before-plugin-load".to_string(),
                sandbox_policy_json: None,
                sandbox_policy_file: None,
                sandbox_required: true,
                sandbox_allow_network: false,
                sandbox_allow_child_processes: false,
                sandbox_read_paths: vec![PathBuf::from("/tmp/plugin.clap")],
                sandbox_write_paths: vec![PathBuf::from("/tmp/plugin-cache")],
            };

            let policy = sandbox_policy(&args).unwrap();
            assert_eq!(policy.timing, ExternalPluginSandboxTiming::BeforePluginLoad);
            assert!(policy.require_platform_sandbox);
            assert_eq!(
                policy.extra_read_paths,
                vec![PathBuf::from("/tmp/plugin.clap")]
            );
            assert_eq!(
                policy.extra_write_paths,
                vec![PathBuf::from("/tmp/plugin-cache")]
            );
        }

        #[test]
        fn sandbox_policy_parses_portable_policy_json() {
            let portable = PluginSandboxPolicy::strict_with_preset_dir("/tmp/sotf-presets");
            let args = Args {
                shared_memory: Some(PathBuf::from("/tmp/fake.shm")),
                descriptor_json: None,
                descriptor_file: None,
                external_state_file: None,
                once: true,
                idle_sleep_micros: 1,
                sandbox_timing: "disabled".to_string(),
                sandbox_policy_json: Some(serde_json::to_string(&portable).unwrap()),
                sandbox_policy_file: None,
                sandbox_required: false,
                sandbox_allow_network: true,
                sandbox_allow_child_processes: true,
                sandbox_read_paths: Vec::new(),
                sandbox_write_paths: Vec::new(),
            };

            let policy = sandbox_policy(&args).unwrap();
            assert_eq!(policy.timing, ExternalPluginSandboxTiming::BeforePluginLoad);
            assert!(!policy.allow_network);
            assert!(!policy.allow_child_processes);
            assert_eq!(
                policy.extra_write_paths,
                vec![PathBuf::from("/tmp/sotf-presets")]
            );
        }

        #[test]
        fn sandbox_policy_rejects_unrepresentable_portable_policy_json() {
            let mut portable = PluginSandboxPolicy::strict_with_preset_dir("/tmp/sotf-presets");
            portable.network = sotf_host::PluginSandboxNetworkGrant::LoopbackOnly;
            let args = Args {
                shared_memory: Some(PathBuf::from("/tmp/fake.shm")),
                descriptor_json: None,
                descriptor_file: None,
                external_state_file: None,
                once: true,
                idle_sleep_micros: 1,
                sandbox_timing: "disabled".to_string(),
                sandbox_policy_json: Some(serde_json::to_string(&portable).unwrap()),
                sandbox_policy_file: None,
                sandbox_required: false,
                sandbox_allow_network: false,
                sandbox_allow_child_processes: false,
                sandbox_read_paths: Vec::new(),
                sandbox_write_paths: Vec::new(),
            };

            let err = sandbox_policy(&args).unwrap_err();
            assert!(err.contains("cannot be represented"));
        }

        #[test]
        fn sandbox_backend_code_maps_macos_app_sandbox_helper() {
            assert_eq!(
                sandbox_backend_code("macos-app-sandbox-helper"),
                PluginSandboxBackendCode::MacosAppSandboxHelper
            );
        }

        #[test]
        fn load_external_state_reads_and_validates_envelope() {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("external-state.json");
            let plugin_path = temp.path().join("fake.clap");
            std::fs::write(&plugin_path, []).unwrap();
            let descriptor = state_descriptor(&plugin_path);
            let state = ExternalPluginState::new(
                descriptor,
                ExternalPluginSandboxMode::Isolated,
                vec![1, 3, 3, 7],
            );
            std::fs::write(&path, serde_json::to_vec(&state).unwrap()).unwrap();

            assert_eq!(
                load_external_state(&args_with_external_state_file(Some(path))).unwrap(),
                Some(state)
            );
        }

        #[test]
        fn worker_restore_state_translates_isolated_envelope_for_native_loader() {
            let temp = tempfile::tempdir().unwrap();
            let plugin_path = temp.path().join("fake.clap");
            std::fs::write(&plugin_path, []).unwrap();
            let state = ExternalPluginState::new(
                state_descriptor(&plugin_path),
                ExternalPluginSandboxMode::Isolated,
                vec![1, 3, 3, 7],
            );

            let worker_state = worker_restore_state(&state).unwrap();
            assert_eq!(
                worker_state.sandbox_mode,
                ExternalPluginSandboxMode::InProcess
            );
            assert_eq!(worker_state.descriptor, state.descriptor);
            assert_eq!(worker_state.opaque_state, state.opaque_state);
        }

        #[test]
        fn worker_restore_state_rejects_non_isolated_envelope() {
            let temp = tempfile::tempdir().unwrap();
            let plugin_path = temp.path().join("fake.clap");
            std::fs::write(&plugin_path, []).unwrap();
            let state = ExternalPluginState::new(
                state_descriptor(&plugin_path),
                ExternalPluginSandboxMode::InProcess,
                vec![],
            );

            let error = worker_restore_state(&state).unwrap_err();
            assert!(error.contains("expected isolated state"));
        }

        #[test]
        fn load_external_state_rejects_inconsistent_envelope() {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("external-state.json");
            let plugin_path = temp.path().join("fake.clap");
            std::fs::write(&plugin_path, []).unwrap();
            let descriptor = state_descriptor(&plugin_path);
            let mut state = ExternalPluginState::new(
                descriptor,
                ExternalPluginSandboxMode::Isolated,
                vec![1, 3, 3, 7],
            );
            state.plugin_id = "different.plugin".to_string();
            std::fs::write(&path, serde_json::to_vec(&state).unwrap()).unwrap();

            let error =
                load_external_state(&args_with_external_state_file(Some(path))).unwrap_err();
            assert!(error.contains("descriptor fields are inconsistent"));
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn main() {
    desktop::main();
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn main() {
    eprintln!("sotf-external-plugin-worker is only supported on Linux, macOS, and Windows");
    std::process::exit(2);
}

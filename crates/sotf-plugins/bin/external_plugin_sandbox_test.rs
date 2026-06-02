use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::{Parser, ValueEnum};
use serde_json::json;
use sotf_plugins::{
    ExternalPluginProcessEvent, ExternalPluginWorkerCommand, IsolatedExternalPlugin,
    IsolatedExternalPluginConfig, IsolatedExternalPluginWorkerReport, PluginDescriptor,
    PluginFormat, PluginHost, PluginSandboxAuthorizationGrant, PluginSandboxGrantStore,
    PluginSandboxIdentity, PluginSandboxLaunchBackend, PluginSandboxLifecycleMode,
    PluginSandboxNetworkGrant, PluginSandboxPermission, PluginSandboxPolicy,
    PluginSandboxStatusCode, PluginSandboxUserGrant, current_plugin_sandbox_launch_backend,
    current_plugin_sandbox_launcher_command, default_plugin_sandbox_launcher_command_for_backend,
    default_plugin_sandbox_protected_media_paths,
};

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Run an external plugin through the SOTF sandbox harness"
)]
struct Args {
    /// External plugin file/bundle or directory to scan.
    #[arg(long)]
    path: Option<PathBuf>,

    /// Descriptor JSON file. When set, --path scanning is skipped.
    #[arg(long)]
    descriptor_json: Option<PathBuf>,

    /// Only scan for this plugin format.
    #[arg(long, value_enum)]
    format: Option<CliPluginFormat>,

    /// List discovered plugins and exit.
    #[arg(long)]
    list: bool,

    /// Select discovered plugin by zero-based index.
    #[arg(long, default_value_t = 0)]
    index: usize,

    /// Select discovered plugin by exact id.
    #[arg(long)]
    plugin_id: Option<String>,

    /// Select discovered plugin by case-insensitive display name.
    #[arg(long)]
    plugin_name: Option<String>,

    /// Audio sample rate used for instantiation.
    #[arg(long, default_value_t = 48_000)]
    sample_rate: u32,

    /// Host input channel count.
    #[arg(long, default_value_t = 2)]
    channels: usize,

    /// Process block size in frames.
    #[arg(long, default_value_t = 512)]
    frames: usize,

    /// Number of silence blocks to process.
    #[arg(long, default_value_t = 4)]
    blocks: usize,

    /// Milliseconds to wait for the worker to publish sandbox status before processing.
    #[arg(long, default_value_t = 2_000)]
    startup_timeout_ms: u64,

    /// Root directory used for per-plugin preset sandbox write access.
    #[arg(long)]
    preset_root: Option<PathBuf>,

    /// Sandbox lifecycle policy to test.
    #[arg(long, value_enum, default_value_t = CliLifecycleMode::Import)]
    lifecycle: CliLifecycleMode,

    /// Media/audio root to expose only in authorized-runtime mode.
    #[arg(long = "media-path")]
    media_paths: Vec<PathBuf>,

    /// Media/audio root that import mode must never expose.
    #[arg(long = "protected-media-path")]
    protected_media_paths: Vec<PathBuf>,

    /// Optional persisted grant-store JSON file.
    #[arg(long)]
    grant_store: Option<PathBuf>,

    /// Worker binary. Defaults to a sibling sotf-external-plugin-worker binary.
    #[arg(long)]
    worker_binary: Option<PathBuf>,

    /// macOS sandbox helper binary. Defaults to a sibling sotf-macos-sandbox-helper binary.
    #[arg(long)]
    macos_helper_binary: Option<PathBuf>,

    /// Force a launch backend instead of using the current platform default.
    #[arg(long, value_enum, default_value_t = CliSandboxBackend::Current)]
    backend: CliSandboxBackend,

    /// Add a read-only filesystem grant.
    #[arg(long)]
    allow_read: Vec<PathBuf>,

    /// Add a read-write filesystem grant.
    #[arg(long)]
    allow_write: Vec<PathBuf>,

    /// Allow outbound network access.
    #[arg(long, value_enum)]
    allow_network: Option<CliNetworkGrant>,

    /// Allow a local authorization profile such as PACE/iLok.
    #[arg(long, value_enum)]
    allow_authorization: Vec<CliAuthorizationGrant>,

    /// Print machine-readable JSON summary.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliPluginFormat {
    Clap,
    Vst3,
    Au,
}

impl From<CliPluginFormat> for PluginFormat {
    fn from(value: CliPluginFormat) -> Self {
        match value {
            CliPluginFormat::Clap => Self::Clap,
            CliPluginFormat::Vst3 => Self::Vst3,
            CliPluginFormat::Au => Self::AudioUnit,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliSandboxBackend {
    Current,
    LinuxLandlock,
    MacosHelper,
    WindowsAppcontainer,
    ProcessOnly,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliLifecycleMode {
    Import,
    AuthorizedRuntime,
}

impl From<CliLifecycleMode> for PluginSandboxLifecycleMode {
    fn from(value: CliLifecycleMode) -> Self {
        match value {
            CliLifecycleMode::Import => Self::Import,
            CliLifecycleMode::AuthorizedRuntime => Self::AuthorizedRuntime,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliNetworkGrant {
    Loopback,
    Any,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliAuthorizationGrant {
    Pace,
    Ilok,
    SystemKeychain,
    Any,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("external-plugin-sandbox-test: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    validate_lifecycle_args(&args)?;
    let descriptor = if let Some(path) = &args.descriptor_json {
        let json = std::fs::read_to_string(path)
            .map_err(|err| format!("failed to read descriptor JSON {}: {err}", path.display()))?;
        serde_json::from_str::<PluginDescriptor>(&json)
            .map_err(|err| format!("failed to parse descriptor JSON {}: {err}", path.display()))?
    } else {
        let path = args
            .path
            .as_ref()
            .ok_or_else(|| "--path is required unless --descriptor-json is set".to_string())?;
        let mut scanner = sotf_plugins::PluginScanner::new();
        scanner.scan_path(path, args.format.map(Into::into))?;
        if args.list {
            print_discovered_plugins(scanner.list(), args.json)?;
            return Ok(());
        }
        select_plugin(scanner.list(), &args)?.clone()
    };

    let preset_root = args.preset_root.clone().unwrap_or_else(default_preset_root);
    std::fs::create_dir_all(&preset_root).map_err(|err| {
        format!(
            "failed to create preset root {}: {err}",
            preset_root.display()
        )
    })?;

    let mut grants = load_grant_store(args.grant_store.as_deref())?;
    if matches!(args.lifecycle, CliLifecycleMode::Import) {
        add_cli_grants(&mut grants, &descriptor, &args);
    }
    let policy = lifecycle_policy(&grants, &descriptor, &preset_root, &args)?;

    let backend = sandbox_backend(&args);
    let launcher = sandbox_launcher(&args, backend);
    let worker = args
        .worker_binary
        .as_ref()
        .map(ExternalPluginWorkerCommand::new)
        .unwrap_or_else(ExternalPluginWorkerCommand::default_worker_binary);

    let plugin = IsolatedExternalPlugin::new(
        descriptor.clone(),
        args.sample_rate,
        IsolatedExternalPluginConfig {
            worker_command: worker,
            capability_sandbox_policy: Some(policy),
            sandbox_launch_backend: backend,
            sandbox_launcher_command: launcher,
            ..Default::default()
        },
    )?;

    let mut host = PluginHost::new(args.channels, args.sample_rate);
    host.add_plugin(Box::new(plugin))?;
    host.build()?;
    let initial_reports = host.ensure_isolated_external_plugin_workers_running();
    let startup_reports =
        wait_for_worker_sandbox_status(&mut host, Duration::from_millis(args.startup_timeout_ms));

    let input = vec![0.0_f32; args.frames * args.channels];
    let mut output = vec![0.0_f32; args.frames * host.output_channels()];
    let mut processed = 0usize;
    for _ in 0..args.blocks {
        processed += host.process(&input, &mut output)?;
    }
    std::thread::sleep(Duration::from_millis(25));
    let final_reports = host.poll_isolated_external_plugin_workers();

    if args.json {
        print_json_summary(
            &descriptor,
            backend,
            &preset_root,
            args.lifecycle,
            processed,
            host.output_channels(),
            &initial_reports,
            &startup_reports,
            &final_reports,
        )?;
    } else {
        print_text_summary(
            &descriptor,
            backend,
            &preset_root,
            args.lifecycle,
            processed,
            host.output_channels(),
            &initial_reports,
            &startup_reports,
            &final_reports,
        );
    }
    Ok(())
}

fn validate_lifecycle_args(args: &Args) -> Result<(), String> {
    if !matches!(args.lifecycle, CliLifecycleMode::AuthorizedRuntime) {
        return Ok(());
    }

    if !args.allow_read.is_empty()
        || !args.allow_write.is_empty()
        || args.allow_network.is_some()
        || !args.allow_authorization.is_empty()
    {
        return Err(
            "authorized-runtime mode rejects import/external grants; use --media-path for audio roots"
                .to_string(),
        );
    }
    Ok(())
}

fn lifecycle_policy(
    grants: &PluginSandboxGrantStore,
    descriptor: &PluginDescriptor,
    preset_root: &Path,
    args: &Args,
) -> Result<PluginSandboxPolicy, String> {
    let policy = match PluginSandboxLifecycleMode::from(args.lifecycle) {
        PluginSandboxLifecycleMode::Import => {
            grants.import_policy_for_plugin(descriptor, preset_root, protected_media_paths(args))
        }
        PluginSandboxLifecycleMode::AuthorizedRuntime => grants
            .authorized_runtime_policy_for_plugin(
                descriptor,
                preset_root,
                runtime_media_paths(args),
            ),
    };
    policy.validate_protected_media_paths()?;
    Ok(policy)
}

fn protected_media_paths(args: &Args) -> Vec<PathBuf> {
    let mut paths = default_plugin_sandbox_protected_media_paths();
    paths.extend(args.media_paths.iter().cloned());
    paths.extend(args.protected_media_paths.iter().cloned());
    paths
}

fn runtime_media_paths(args: &Args) -> Vec<PathBuf> {
    if !args.media_paths.is_empty() {
        return args.media_paths.clone();
    }
    default_plugin_sandbox_protected_media_paths()
        .into_iter()
        .filter(|path| path.exists())
        .collect()
}

fn select_plugin<'a>(
    plugins: &'a [PluginDescriptor],
    args: &Args,
) -> Result<&'a PluginDescriptor, String> {
    if let Some(id) = &args.plugin_id {
        return plugins
            .iter()
            .find(|plugin| plugin.id == *id)
            .ok_or_else(|| format!("no scanned plugin has id '{id}'"));
    }
    if let Some(name) = &args.plugin_name {
        let name = name.to_lowercase();
        return plugins
            .iter()
            .find(|plugin| plugin.name.to_lowercase() == name)
            .ok_or_else(|| format!("no scanned plugin has name '{name}'"));
    }
    plugins.get(args.index).ok_or_else(|| {
        format!(
            "no plugin at index {}; scan found {}",
            args.index,
            plugins.len()
        )
    })
}

fn load_grant_store(path: Option<&Path>) -> Result<PluginSandboxGrantStore, String> {
    let Some(path) = path else {
        return Ok(PluginSandboxGrantStore::default());
    };
    if !path.exists() {
        return Ok(PluginSandboxGrantStore::default());
    }
    let json = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read grant store {}: {err}", path.display()))?;
    serde_json::from_str(&json)
        .map_err(|err| format!("failed to parse grant store {}: {err}", path.display()))
}

fn add_cli_grants(
    grants: &mut PluginSandboxGrantStore,
    descriptor: &PluginDescriptor,
    args: &Args,
) {
    let identity = PluginSandboxIdentity::from_descriptor(descriptor);
    for path in &args.allow_read {
        grants.remember(PluginSandboxUserGrant {
            identity: identity.clone(),
            permission: PluginSandboxPermission::ReadPath { path: path.clone() },
        });
    }
    for path in &args.allow_write {
        grants.remember(PluginSandboxUserGrant {
            identity: identity.clone(),
            permission: PluginSandboxPermission::WritePath { path: path.clone() },
        });
    }
    if let Some(network) = args.allow_network {
        grants.remember(PluginSandboxUserGrant {
            identity: identity.clone(),
            permission: PluginSandboxPermission::Network(match network {
                CliNetworkGrant::Loopback => PluginSandboxNetworkGrant::LoopbackOnly,
                CliNetworkGrant::Any => PluginSandboxNetworkGrant::AnyOutbound,
            }),
        });
    }
    for grant in &args.allow_authorization {
        grants.remember(PluginSandboxUserGrant {
            identity: identity.clone(),
            permission: PluginSandboxPermission::LocalAuthorization(match grant {
                CliAuthorizationGrant::Pace => PluginSandboxAuthorizationGrant::Pace,
                CliAuthorizationGrant::Ilok => PluginSandboxAuthorizationGrant::Ilok,
                CliAuthorizationGrant::SystemKeychain => {
                    PluginSandboxAuthorizationGrant::SystemKeychain
                }
                CliAuthorizationGrant::Any => PluginSandboxAuthorizationGrant::Any,
            }),
        });
    }
}

fn sandbox_backend(args: &Args) -> PluginSandboxLaunchBackend {
    match args.backend {
        CliSandboxBackend::Current => current_plugin_sandbox_launch_backend(),
        CliSandboxBackend::LinuxLandlock => PluginSandboxLaunchBackend::LinuxLandlockWorker,
        CliSandboxBackend::MacosHelper => PluginSandboxLaunchBackend::MacosAppSandboxHelper,
        CliSandboxBackend::WindowsAppcontainer => {
            PluginSandboxLaunchBackend::WindowsAppContainerWorker
        }
        CliSandboxBackend::ProcessOnly => PluginSandboxLaunchBackend::ProcessIsolationOnly {
            platform: "external-plugin-sandbox-test-process-only",
        },
    }
}

fn sandbox_launcher(
    args: &Args,
    backend: PluginSandboxLaunchBackend,
) -> Option<ExternalPluginWorkerCommand> {
    match backend {
        PluginSandboxLaunchBackend::MacosAppSandboxHelper => args
            .macos_helper_binary
            .as_ref()
            .map(ExternalPluginWorkerCommand::new)
            .or_else(|| default_plugin_sandbox_launcher_command_for_backend(backend)),
        _ => current_plugin_sandbox_launcher_command(),
    }
}

fn default_preset_root() -> PathBuf {
    std::env::temp_dir().join("sotf-external-plugin-sandbox-test-presets")
}

fn wait_for_worker_sandbox_status(
    host: &mut PluginHost,
    timeout: Duration,
) -> Vec<IsolatedExternalPluginWorkerReport> {
    let deadline = std::time::Instant::now() + timeout;
    let mut reports = host.poll_isolated_external_plugin_workers();
    loop {
        if worker_reports_ready(&reports) || std::time::Instant::now() >= deadline {
            return reports;
        }
        std::thread::sleep(Duration::from_millis(10));
        reports = host.poll_isolated_external_plugin_workers();
    }
}

fn worker_reports_ready(reports: &[IsolatedExternalPluginWorkerReport]) -> bool {
    !reports.is_empty()
        && reports.iter().all(|report| {
            report.error.is_some()
                || matches!(
                    report.event,
                    Some(ExternalPluginProcessEvent::Exited { .. })
                        | Some(ExternalPluginProcessEvent::NotRunning)
                )
                || report.sandbox_status != PluginSandboxStatusCode::Unknown
        })
}

fn print_discovered_plugins(plugins: &[PluginDescriptor], json_output: bool) -> Result<(), String> {
    if json_output {
        let json = serde_json::to_string_pretty(plugins)
            .map_err(|err| format!("failed to serialize plugin list: {err}"))?;
        println!("{json}");
        return Ok(());
    }
    for (index, plugin) in plugins.iter().enumerate() {
        println!(
            "#{index}: {} [{} {:?}] {}",
            plugin.name,
            plugin.id,
            plugin.format,
            plugin.path.display()
        );
    }
    Ok(())
}

fn print_text_summary(
    descriptor: &PluginDescriptor,
    backend: PluginSandboxLaunchBackend,
    preset_root: &Path,
    lifecycle: CliLifecycleMode,
    processed_frames: usize,
    output_channels: usize,
    initial_reports: &[IsolatedExternalPluginWorkerReport],
    startup_reports: &[IsolatedExternalPluginWorkerReport],
    final_reports: &[IsolatedExternalPluginWorkerReport],
) {
    println!("plugin: {} ({})", descriptor.name, descriptor.id);
    println!("path: {}", descriptor.path.display());
    println!("format: {:?}", descriptor.format);
    println!("backend: {}", backend.backend_id());
    println!("lifecycle: {lifecycle:?}");
    println!("preset root: {}", preset_root.display());
    println!("processed frames: {processed_frames}");
    println!("output channels: {output_channels}");
    print_reports("initial worker reports", initial_reports);
    print_reports("startup worker reports", startup_reports);
    print_reports("final worker reports", final_reports);
}

fn print_reports(label: &str, reports: &[IsolatedExternalPluginWorkerReport]) {
    println!("{label}: {}", reports.len());
    for report in reports {
        println!(
            "  plugin={} node={} event={:?} error={:?} starts={} exits={} launch_failures={} sandbox={:?}/{:?} reason={:?}",
            report.plugin_index,
            report.node_id,
            report.event,
            report.error,
            report.worker_start_count,
            report.worker_exit_count,
            report.worker_launch_failure_count,
            report.sandbox_status,
            report.sandbox_backend,
            report.sandbox_reason,
        );
    }
}

fn print_json_summary(
    descriptor: &PluginDescriptor,
    backend: PluginSandboxLaunchBackend,
    preset_root: &Path,
    lifecycle: CliLifecycleMode,
    processed_frames: usize,
    output_channels: usize,
    initial_reports: &[IsolatedExternalPluginWorkerReport],
    startup_reports: &[IsolatedExternalPluginWorkerReport],
    final_reports: &[IsolatedExternalPluginWorkerReport],
) -> Result<(), String> {
    let value = json!({
        "plugin": descriptor,
        "backend": backend.backend_id(),
        "lifecycle": format!("{lifecycle:?}"),
        "preset_root": preset_root,
        "processed_frames": processed_frames,
        "output_channels": output_channels,
        "initial_reports": worker_reports_json(initial_reports),
        "startup_reports": worker_reports_json(startup_reports),
        "final_reports": worker_reports_json(final_reports),
    });
    let json = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("failed to serialize summary: {err}"))?;
    println!("{json}");
    Ok(())
}

fn worker_reports_json(reports: &[IsolatedExternalPluginWorkerReport]) -> Vec<serde_json::Value> {
    reports
        .iter()
        .map(|report| {
            json!({
                "plugin_index": report.plugin_index,
                "node_id": report.node_id,
                "event": format!("{:?}", report.event),
                "error": report.error,
                "worker_start_count": report.worker_start_count,
                "worker_exit_count": report.worker_exit_count,
                "worker_launch_failure_count": report.worker_launch_failure_count,
                "block_timeout_count": report.block_timeout_count,
                "block_worker_failure_count": report.block_worker_failure_count,
                "block_wrong_sequence_count": report.block_wrong_sequence_count,
                "sandbox_status": format!("{:?}", report.sandbox_status),
                "sandbox_backend": format!("{:?}", report.sandbox_backend),
                "sandbox_reason": report.sandbox_reason,
            })
        })
        .collect()
}

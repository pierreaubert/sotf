use super::misc::default_preset_root;
use super::misc::load_grant_store;
use super::print::print_discovered_plugins;
use super::print::print_json_summary;
use super::print::print_text_summary;
use super::sandbox::sandbox_backend;
use super::sandbox::sandbox_launcher;
use super::types::Args;
use super::types::CliAuthorizationGrant;
use super::types::CliLifecycleMode;
use super::types::CliNetworkGrant;
use super::worker::wait_for_worker_sandbox_status;
use clap::Parser;
use sotf_plugins::{
    ExternalPluginWorkerCommand, IsolatedExternalPlugin, IsolatedExternalPluginConfig,
    PluginDescriptor, PluginHost, PluginSandboxAuthorizationGrant, PluginSandboxGrantStore,
    PluginSandboxIdentity, PluginSandboxLifecycleMode, PluginSandboxNetworkGrant,
    PluginSandboxPermission, PluginSandboxPolicy, PluginSandboxUserGrant,
    default_plugin_sandbox_protected_media_paths,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(super) fn run() -> Result<(), String> {
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

pub(super) fn validate_lifecycle_args(args: &Args) -> Result<(), String> {
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

pub(super) fn lifecycle_policy(
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

pub(super) fn protected_media_paths(args: &Args) -> Vec<PathBuf> {
    let mut paths = default_plugin_sandbox_protected_media_paths();
    paths.extend(args.media_paths.iter().cloned());
    paths.extend(args.protected_media_paths.iter().cloned());
    paths
}

pub(super) fn runtime_media_paths(args: &Args) -> Vec<PathBuf> {
    if !args.media_paths.is_empty() {
        return args.media_paths.clone();
    }
    default_plugin_sandbox_protected_media_paths()
        .into_iter()
        .filter(|path| path.exists())
        .collect()
}

pub(super) fn select_plugin<'a>(
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

pub(super) fn add_cli_grants(
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

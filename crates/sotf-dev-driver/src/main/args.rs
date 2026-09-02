use super::ctx::run_script;
use super::misc::resolve_base_url;
use super::types::Args;
use super::types::Command;
use anyhow::{Result, anyhow, bail};
use sotf_dev_driver::fuzz::{
    FuzzCommandOptions, MinimizeCommandOptions, ReplayCommandOptions, run_fuzz_command,
    run_minimize_command, run_replay_command,
};
use std::collections::BTreeSet;

pub(super) fn run(args: &Args) -> Result<()> {
    match &args.command {
        Some(Command::RunSuite { suite, verbose }) => super::suite::run_suite(suite, *verbose),
        Some(Command::Fuzz {
            target,
            seed,
            steps,
            time,
            workers,
            fixture,
            artifacts,
            manifest,
            executable,
            url,
            run_id,
            durable_trace,
            allow_hardware_audio,
            allow_network,
            allow_external_plugins,
            allow_hal_install,
            allow_physical_device,
        }) => {
            use sotf_dev_driver::fuzz::model::opt_in;
            let mut opt_ins = BTreeSet::new();
            for (enabled, name) in [
                (*allow_hardware_audio, opt_in::HARDWARE_AUDIO),
                (*allow_network, opt_in::NETWORK),
                (*allow_external_plugins, opt_in::EXTERNAL_PLUGINS),
                (*allow_hal_install, opt_in::HAL_INSTALL),
                (*allow_physical_device, opt_in::PHYSICAL_DEVICE),
            ] {
                if enabled {
                    opt_ins.insert(name.to_owned());
                }
            }
            let time_budget = time.as_deref().map(super::parse_duration).transpose()?;
            let results = run_fuzz_command(FuzzCommandOptions {
                target: *target,
                seed: *seed,
                steps: *steps,
                time_budget,
                workers: *workers,
                fixture_profile: fixture.clone(),
                artifact_root: artifacts.clone(),
                manifest: manifest.clone(),
                executable: executable.clone(),
                url: url.clone(),
                run_id: run_id.clone(),
                durable_trace: *durable_trace,
                opt_ins,
            })?;
            let failed = results
                .iter()
                .filter(|result| result.summary.outcome == "failed")
                .collect::<Vec<_>>();
            for result in &results {
                println!(
                    "{}: {} ({} steps), artifacts at {}",
                    result.summary.target,
                    result.summary.outcome,
                    result.summary.steps,
                    result.run_dir.display()
                );
            }
            if failed.is_empty() {
                Ok(())
            } else {
                bail!("{} fuzz worker(s) found failures", failed.len())
            }
        }
        Some(Command::Replay {
            replay,
            executable,
            url,
            run_id,
            best_effort_capabilities,
        }) => match run_replay_command(&ReplayCommandOptions {
            replay: replay.clone(),
            executable: executable.clone(),
            url: url.clone(),
            run_id: run_id.clone(),
            best_effort_capabilities: *best_effort_capabilities,
        })? {
            Some(signature) => bail!(
                "replay reproduced {:?}: {}",
                signature.class,
                signature.normalized
            ),
            None => {
                println!("replay completed without a failure");
                Ok(())
            }
        },
        Some(Command::Minimize {
            replay,
            executable,
            url,
            run_id,
        }) => {
            let minimized = run_minimize_command(&MinimizeCommandOptions {
                replay: replay.clone(),
                executable: executable.clone(),
                url: url.clone(),
                run_id: run_id.clone(),
            })?;
            println!("minimized replay written to {}", minimized.display());
            Ok(())
        }
        None => {
            let script = args
                .script
                .as_ref()
                .ok_or_else(|| anyhow!("missing scenario path or subcommand"))?;
            let env_port = match std::env::var("SOTF_DEV_API_PORT") {
                Ok(port) => Some(port),
                Err(std::env::VarError::NotPresent) => None,
                Err(e) => bail!("reading SOTF_DEV_API_PORT: {e}"),
            };
            let url = resolve_base_url(args.url.as_deref(), env_port.as_deref())?;
            run_script(script, &url, args.verbose)
        }
    }
}

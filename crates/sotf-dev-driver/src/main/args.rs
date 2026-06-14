use super::ctx::run_script;
use super::misc::resolve_base_url;
use super::types::Args;
use super::types::Command;
use anyhow::{Result, anyhow, bail};

pub(super) fn run(args: &Args) -> Result<()> {
    match &args.command {
        Some(Command::RunSuite { suite, verbose }) => super::suite::run_suite(suite, *verbose),
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

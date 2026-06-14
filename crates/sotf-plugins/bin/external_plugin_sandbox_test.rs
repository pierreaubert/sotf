#[path = "external_plugin_sandbox_test/args.rs"]
mod args;
#[path = "external_plugin_sandbox_test/misc.rs"]
mod misc;
#[path = "external_plugin_sandbox_test/print.rs"]
mod print;
#[path = "external_plugin_sandbox_test/sandbox.rs"]
mod sandbox;
#[path = "external_plugin_sandbox_test/types.rs"]
mod types;
#[path = "external_plugin_sandbox_test/worker.rs"]
mod worker;

use args::run;

fn main() {
    if let Err(err) = run() {
        eprintln!("external-plugin-sandbox-test: {err}");
        std::process::exit(1);
    }
}

use std::env;
use std::process::{self};

#[path = "qa_upmixer/artifact_event.rs"]
mod artifact_event;
#[path = "qa_upmixer/artifact_tracker.rs"]
mod artifact_tracker;
#[path = "qa_upmixer/default.rs"]
mod default;
#[path = "qa_upmixer/diagnostic_deltas.rs"]
mod diagnostic_deltas;
#[path = "qa_upmixer/diagnostic_max_deltas.rs"]
mod diagnostic_max_deltas;
#[path = "qa_upmixer/load.rs"]
mod load;
#[path = "qa_upmixer/misc.rs"]
mod misc;
#[path = "qa_upmixer/parse.rs"]
mod parse;
#[path = "qa_upmixer/run.rs"]
mod run;
#[path = "qa_upmixer/types.rs"]
mod types;
#[path = "qa_upmixer/write.rs"]
mod write;

use misc::print_usage;
use run::run_diagnostic;
use run::run_isolation;
use run::run_self_qa;

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("diagnose") => {
            if let Err(err) = run_diagnostic(args.collect()) {
                eprintln!("diagnose failed: {err}");
                process::exit(2);
            }
        }
        Some("isolate") => {
            if let Err(err) = run_isolation(args.collect()) {
                eprintln!("isolate failed: {err}");
                process::exit(2);
            }
        }
        Some("--help") | Some("-h") => print_usage(),
        Some(other) => {
            eprintln!("unknown command: {other}");
            print_usage();
            process::exit(2);
        }
        None => run_self_qa(),
    }
}

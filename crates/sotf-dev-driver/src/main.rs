//! Scenario driver for the SotF GPUI dev API.
//!
//! Parses line-based `.scn` scripts and translates each verb into an
//! HTTP call against a running `SotF` instance with the `dev-api`
//! feature enabled.
//!
//! Verbs:
//!   action <Name> [json-payload]
//!   query  <path>
//!   assert <path> == <literal>            (string|number|bool, with optional `tolerance=<f>`)
//!   wait_until <path> == <literal>        (with optional `timeout=<duration>`)
//!   sleep <duration>
//!   focus <screen-name>                   (sugar for SwitchTo<Screen>)
//!   key <keystroke>                       (e.g. `cmd-shift-p`, `enter`, `a`)
//!   click <selector>                      (selector must have been registered via dev_track(...))
//!   assert_visible <selector>              (rendered selector has non-empty bounds)
//!   assert_absent <selector>               (selector is not in the current rendered frame)
//!   assert_in_viewport <selector>           (rendered selector is not clipped)
//!   assert_non_overlapping <a> <b>           (rendered selectors do not overlap)
//!   export_room_eq_json [path]             Export completed RoomEQ DSP JSON
//!   elements                              (print every tracked selector; debugging aid)
//!
//! `<duration>` accepts `Ns`, `Nms`, `Nm`. Bare numbers default to seconds.

use clap::Parser;

mod suite;

#[path = "main/args.rs"]
mod args;
#[path = "main/compare.rs"]
mod compare;
#[path = "main/comparison_op.rs"]
mod comparison_op;
#[path = "main/ctx.rs"]
mod ctx;
#[path = "main/misc.rs"]
mod misc;
#[path = "main/parse.rs"]
mod parse;
#[cfg(test)]
#[path = "main/tests.rs"]
mod tests;
#[path = "main/types.rs"]
mod types;
#[path = "main/verb.rs"]
mod verb;

pub(crate) use ctx::*;
pub(crate) use parse::*;

use args::run;
use types::Args;

fn main() {
    let args = Args::parse();
    if let Err(e) = run(&args) {
        eprintln!("FAIL: {e:#}");
        std::process::exit(1);
    }
    println!("PASS");
}

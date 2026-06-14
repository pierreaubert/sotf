use super::misc::split2;
use super::misc::strip_comment;
use super::parse::parse_dev_response;
use super::parse::parse_duration;
use super::types::Ctx;
use super::verb::verb_action;
use super::verb::verb_assert;
use super::verb::verb_click;
use super::verb::verb_elements;
use super::verb::verb_export_room_eq_json;
use super::verb::verb_focus;
use super::verb::verb_key;
use super::verb::verb_query;
use super::verb::verb_wait_until;
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

pub(crate) fn run_script(script: &PathBuf, url: &str, verbose: bool) -> Result<()> {
    let source = fs::read_to_string(script).with_context(|| format!("reading {:?}", script))?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let ctx = Ctx {
        client,
        base: url.trim_end_matches('/').to_string(),
        verbose,
    };

    for (lineno, raw) in source.lines().enumerate() {
        let lineno = lineno + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if ctx.verbose {
            println!("[{lineno:>3}] {line}");
        }
        execute(line, &ctx).with_context(|| format!("line {lineno}: `{line}`"))?;
    }
    Ok(())
}

fn execute(line: &str, ctx: &Ctx) -> Result<()> {
    let (verb, rest) = split2(line);
    match verb {
        "action" => verb_action(rest, ctx),
        "query" => verb_query(rest, ctx).map(|v| {
            if ctx.verbose {
                println!("    -> {v}");
            }
        }),
        "assert" => verb_assert(rest, ctx),
        "wait_until" => verb_wait_until(rest, ctx),
        "sleep" => {
            let dur = parse_duration(rest.trim())?;
            sleep(dur);
            Ok(())
        }
        "focus" => verb_focus(rest, ctx),
        "key" => verb_key(rest, ctx),
        "click" => verb_click(rest, ctx),
        "export_room_eq_json" | "export_roomeq_json" => verb_export_room_eq_json(rest, ctx),
        "elements" => verb_elements(ctx),
        other => bail!("unknown verb `{other}`"),
    }
}

pub(super) fn post_dev_json(ctx: &Ctx, endpoint: &str, body: &Value, label: &str) -> Result<Value> {
    let resp = ctx
        .client
        .post(format!("{}{}", ctx.base, endpoint))
        .json(body)
        .send()?;
    parse_dev_response(resp, label)
}

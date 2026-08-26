use super::misc::split2;
use super::misc::strip_comment;
use super::parse::parse_dev_response;
use super::parse::parse_duration;
use super::types::Ctx;
use super::verb::verb_accessibility;
use super::verb::verb_action;
use super::verb::verb_assert;
use super::verb::verb_assert_absent;
use super::verb::verb_assert_accessible;
use super::verb::verb_assert_element_state;
use super::verb::verb_assert_focused;
use super::verb::verb_assert_in_viewport;
use super::verb::verb_assert_inaccessible;
use super::verb::verb_assert_non_overlapping;
use super::verb::verb_assert_snapshot;
use super::verb::verb_assert_visible;
use super::verb::verb_click;
use super::verb::verb_drag;
use super::verb::verb_elements;
use super::verb::verb_export_room_eq_json;
use super::verb::verb_focus;
use super::verb::verb_hover;
use super::verb::verb_key;
use super::verb::verb_plugin_add;
use super::verb::verb_plugin_chain_load;
use super::verb::verb_plugin_chain_save;
use super::verb::verb_plugin_clear;
use super::verb::verb_plugin_count;
use super::verb::verb_plugin_param_count;
use super::verb::verb_plugin_param_get;
use super::verb::verb_plugin_param_set;
use super::verb::verb_plugin_remove;
use super::verb::verb_query;
use super::verb::verb_resize;
use super::verb::verb_screenshot;
use super::verb::verb_scroll;
use super::verb::verb_type;
use super::verb::verb_wait_idle;
use super::verb::verb_wait_until;
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

pub(crate) fn run_script(script: &PathBuf, url: &str, verbose: bool) -> Result<()> {
    run_script_with_run_id(script, url, verbose, None)
}

pub(crate) fn run_script_with_run_id(
    script: &PathBuf,
    url: &str,
    verbose: bool,
    run_id: Option<&str>,
) -> Result<()> {
    let source = fs::read_to_string(script).with_context(|| format!("reading {:?}", script))?;
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(run_id) = run_id {
        headers.insert(
            "x-sotf-dev-run-id",
            reqwest::header::HeaderValue::from_str(run_id)
                .context("building dev-api run ID header")?,
        );
    }
    let client = reqwest::blocking::Client::builder()
        .default_headers(headers)
        // The QA targets close every response connection. Keep no idle socket
        // around for a later scripted command to race with that close.
        .pool_max_idle_per_host(0)
        .timeout(Duration::from_secs(30))
        .build()?;
    let ctx = Ctx {
        client,
        base: url.trim_end_matches('/').to_string(),
        verbose,
    };

    for (lineno, raw) in source.lines().enumerate() {
        let lineno = lineno + 1;
        let line = strip_comment(raw);
        let expanded = crate::misc::expand_env_vars(line);
        let line = expanded.trim();
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
        "assert_accessible" => verb_assert_accessible(rest, ctx),
        "assert_inaccessible" => verb_assert_inaccessible(rest, ctx),
        "assert_focused" => verb_assert_focused(rest, ctx),
        "assert_snapshot" => verb_assert_snapshot(rest, ctx),
        "wait_until" => verb_wait_until(rest, ctx),
        "wait_idle" => verb_wait_idle(rest, ctx),
        "sleep" => {
            let dur = parse_duration(rest.trim())?;
            sleep(dur);
            Ok(())
        }
        "focus" => verb_focus(rest, ctx),
        "key" => verb_key(rest, ctx),
        "type" => verb_type(rest, ctx),
        "click" => verb_click(rest, ctx),
        "hover" => verb_hover(rest, ctx),
        "drag" => verb_drag(rest, ctx),
        "scroll" => verb_scroll(rest, ctx),
        "resize" => verb_resize(rest, ctx),
        "screenshot" => verb_screenshot(rest, ctx),
        "assert_visible" => verb_assert_visible(rest, ctx),
        "assert_absent" => verb_assert_absent(rest, ctx),
        "assert_in_viewport" => verb_assert_in_viewport(rest, ctx),
        "assert_non_overlapping" => verb_assert_non_overlapping(rest, ctx),
        "assert_enabled" => verb_assert_element_state(rest, "enabled", ctx),
        "assert_selected" => verb_assert_element_state(rest, "selected", ctx),
        "assert_expanded" => verb_assert_element_state(rest, "expanded", ctx),
        "export_room_eq_json" | "export_roomeq_json" => verb_export_room_eq_json(rest, ctx),
        "elements" => verb_elements(ctx),
        "accessibility" => verb_accessibility(ctx),
        "plugin_add" => verb_plugin_add(rest, ctx),
        "plugin_remove" => verb_plugin_remove(rest, ctx),
        "plugin_clear" => verb_plugin_clear(rest, ctx),
        "plugin_count" => verb_plugin_count(rest, ctx).map(|v| println!("    -> {v}")),
        "plugin_param_count" => verb_plugin_param_count(rest, ctx).map(|v| println!("    -> {v}")),
        "plugin_param_set" => verb_plugin_param_set(rest, ctx),
        "plugin_param_get" => verb_plugin_param_get(rest, ctx).map(|v| println!("    -> {v}")),
        "plugin_chain_save" => verb_plugin_chain_save(rest, ctx),
        "plugin_chain_load" => verb_plugin_chain_load(rest, ctx),
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

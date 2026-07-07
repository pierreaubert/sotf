use super::ctx::post_dev_json;
use super::misc::focus_action_name;
use super::misc::split2;
use super::misc::urlencode;
use super::parse::parse_compare;
use super::parse::parse_dev_response;
use super::types::Ctx;
use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;
use serde_json::json;
use std::thread::sleep;
use std::time::{Duration, Instant};

pub(super) fn verb_action(rest: &str, ctx: &Ctx) -> Result<()> {
    let (name, payload_raw) = split2(rest);
    if name.is_empty() {
        bail!("action verb needs a name");
    }
    let payload: Option<Value> = if payload_raw.trim().is_empty() {
        None
    } else {
        Some(serde_json::from_str(payload_raw.trim()).context("payload is not valid JSON")?)
    };
    let body = serde_json::json!({ "name": name, "payload": payload });
    post_dev_json(ctx, "/action", &body, &format!("action `{name}`"))?;
    Ok(())
}

pub(super) fn verb_query(rest: &str, ctx: &Ctx) -> Result<Value> {
    let path = rest.trim();
    if path.is_empty() {
        bail!("query verb needs a path");
    }
    let url = format!("{}/query?path={}", ctx.base, urlencode(path));
    let resp = ctx.client.get(url).send()?;
    let json = parse_dev_response(resp, &format!("query `{path}`"))?;
    json.get("value")
        .cloned()
        .ok_or_else(|| anyhow!("server returned no `value`"))
}

pub(super) fn verb_assert(rest: &str, ctx: &Ctx) -> Result<()> {
    let cmp = parse_compare(rest)?;
    let actual = verb_query(&cmp.path, ctx)?;
    if !cmp.matches(&actual) {
        bail!(
            "assertion failed: {} {} {} (got {})",
            cmp.path,
            cmp.op.as_str(),
            cmp.expected_text,
            actual
        );
    }
    if ctx.verbose {
        println!("    -> ok ({actual})");
    }
    Ok(())
}

pub(super) fn verb_wait_until(rest: &str, ctx: &Ctx) -> Result<()> {
    let cmp = parse_compare(rest)?;
    let timeout = cmp.timeout.unwrap_or(Duration::from_secs(1));
    let deadline = Instant::now() + timeout;
    let mut last = Value::Null;
    while Instant::now() < deadline {
        match verb_query(&cmp.path, ctx) {
            Ok(v) => {
                if cmp.matches(&v) {
                    if ctx.verbose {
                        println!("    -> matched ({v})");
                    }
                    return Ok(());
                }
                last = v;
            }
            Err(e) => last = Value::String(format!("{e}")),
        }
        sleep(Duration::from_millis(50));
    }
    bail!(
        "wait_until timed out after {:?}: {} {} {} (last seen: {})",
        timeout,
        cmp.path,
        cmp.op.as_str(),
        cmp.expected_text,
        last
    );
}

pub(super) fn verb_key(rest: &str, ctx: &Ctx) -> Result<()> {
    let keystroke = rest.trim();
    if keystroke.is_empty() {
        bail!("key verb needs a keystroke");
    }
    let body = serde_json::json!({ "keystroke": keystroke });
    post_dev_json(ctx, "/key", &body, &format!("key `{keystroke}`"))?;
    Ok(())
}

pub(super) fn verb_click(rest: &str, ctx: &Ctx) -> Result<()> {
    let selector = rest.trim();
    if selector.is_empty() {
        bail!("click verb needs a selector");
    }
    let body = serde_json::json!({ "selector": selector });
    post_dev_json(ctx, "/click", &body, &format!("click `{selector}`"))?;
    Ok(())
}

pub(super) fn verb_elements(ctx: &Ctx) -> Result<()> {
    let resp = ctx.client.get(format!("{}/elements", ctx.base)).send()?;
    let json = parse_dev_response(resp, "elements")?;
    let list = json
        .get("elements")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if list.is_empty() {
        println!("    (no tracked elements yet)");
    } else {
        for el in list {
            let sel = el.get("selector").and_then(Value::as_str).unwrap_or("?");
            let cx = el.get("cx").and_then(Value::as_f64).unwrap_or(0.0);
            let cy = el.get("cy").and_then(Value::as_f64).unwrap_or(0.0);
            println!("    {sel:<40} @ ({cx:.0}, {cy:.0})");
        }
    }
    Ok(())
}

pub(super) fn verb_export_room_eq_json(rest: &str, ctx: &Ctx) -> Result<()> {
    let path = rest.trim();
    let body = if path.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::json!({ "path": path })
    };
    let json = post_dev_json(ctx, "/qa/room-eq/export-json", &body, "RoomEQ JSON export")?;
    if ctx.verbose {
        let value = json.get("value").cloned().unwrap_or(Value::Null);
        println!("    -> {value}");
    }
    Ok(())
}

pub(super) fn verb_focus(rest: &str, ctx: &Ctx) -> Result<()> {
    let target = rest.trim();
    if target.is_empty() {
        bail!("focus verb needs a screen name");
    }
    let action_name = focus_action_name(target)?;
    verb_action(&action_name, ctx)
}

pub(super) fn verb_plugin_add(rest: &str, ctx: &Ctx) -> Result<()> {
    let plugin_type = rest.trim();
    if plugin_type.is_empty() {
        bail!("plugin_add needs a plugin type");
    }
    let body = json!({ "name": "PluginAdd", "payload": { "plugin_type": plugin_type } });
    post_dev_json(ctx, "/action", &body, "plugin_add")?;
    Ok(())
}

pub(super) fn verb_plugin_remove(rest: &str, ctx: &Ctx) -> Result<()> {
    let index: usize = rest
        .trim()
        .parse()
        .context("plugin_remove needs an index")?;
    let body = json!({ "name": "PluginRemove", "payload": { "index": index } });
    post_dev_json(ctx, "/action", &body, "plugin_remove")?;
    Ok(())
}

pub(super) fn verb_plugin_clear(_rest: &str, ctx: &Ctx) -> Result<()> {
    let body = json!({ "name": "PluginClear", "payload": {} });
    post_dev_json(ctx, "/action", &body, "plugin_clear")?;
    Ok(())
}

pub(super) fn verb_plugin_count(_rest: &str, ctx: &Ctx) -> Result<Value> {
    verb_query("plugins.count", ctx)
}

pub(super) fn verb_plugin_param_count(rest: &str, ctx: &Ctx) -> Result<Value> {
    let index: usize = rest
        .trim()
        .parse()
        .context("plugin_param_count needs an index")?;
    verb_query(&format!("plugins.plugin.{index}.param_count"), ctx)
}

pub(super) fn verb_plugin_param_set(rest: &str, ctx: &Ctx) -> Result<()> {
    let mut parts = rest.split_whitespace();
    let index: usize = parts
        .next()
        .ok_or_else(|| anyhow!("plugin_param_set needs index"))?
        .parse()?;
    let param_index: usize = parts
        .next()
        .ok_or_else(|| anyhow!("plugin_param_set needs param_index"))?
        .parse()?;
    let value: f64 = parts
        .next()
        .ok_or_else(|| anyhow!("plugin_param_set needs value"))?
        .parse()?;
    let body = json!({ "name": "PluginSetParam", "payload": { "index": index, "param_index": param_index, "value": value } });
    post_dev_json(ctx, "/action", &body, "plugin_param_set")?;
    Ok(())
}

pub(super) fn verb_plugin_param_get(rest: &str, ctx: &Ctx) -> Result<Value> {
    let mut parts = rest.split_whitespace();
    let index: usize = parts
        .next()
        .ok_or_else(|| anyhow!("plugin_param_get needs index"))?
        .parse()?;
    let param_index: usize = parts
        .next()
        .ok_or_else(|| anyhow!("plugin_param_get needs param_index"))?
        .parse()?;
    verb_query(
        &format!("plugins.plugin.{index}.param.{param_index}.value"),
        ctx,
    )
}

pub(super) fn verb_plugin_chain_save(rest: &str, ctx: &Ctx) -> Result<()> {
    let path = rest.trim();
    if path.is_empty() {
        bail!("plugin_chain_save needs a path");
    }
    let body = json!({ "name": "PluginChainSave", "payload": { "path": path } });
    post_dev_json(ctx, "/action", &body, "plugin_chain_save")?;
    Ok(())
}

pub(super) fn verb_plugin_chain_load(rest: &str, ctx: &Ctx) -> Result<()> {
    let path = rest.trim();
    if path.is_empty() {
        bail!("plugin_chain_load needs a path");
    }
    let body = json!({ "name": "PluginChainLoad", "payload": { "path": path } });
    post_dev_json(ctx, "/action", &body, "plugin_chain_load")?;
    Ok(())
}

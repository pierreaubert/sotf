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
//!   elements                              (list every tracked selector — debugging aid)
//!
//! `<duration>` accepts `Ns`, `Nms`, `Nm`. Bare numbers default to seconds.

use std::fs;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use serde_json::Value;

#[derive(Parser, Debug)]
#[command(name = "sotf-dev-driver", version)]
struct Args {
    /// Path to a .scn scenario file.
    script: PathBuf,
    /// Base URL of the running SotF dev API.
    #[arg(long, default_value = "http://127.0.0.1:7777")]
    url: String,
    /// Print every verb + result.
    #[arg(long)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();
    if let Err(e) = run(&args) {
        eprintln!("FAIL: {e:#}");
        std::process::exit(1);
    }
    println!("PASS");
}

fn run(args: &Args) -> Result<()> {
    let source =
        fs::read_to_string(&args.script).with_context(|| format!("reading {:?}", args.script))?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let ctx = Ctx {
        client,
        base: args.url.trim_end_matches('/').to_string(),
        verbose: args.verbose,
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

struct Ctx {
    client: reqwest::blocking::Client,
    base: String,
    verbose: bool,
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(i) => &line[..i],
        None => line,
    }
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
        "elements" => verb_elements(ctx),
        other => bail!("unknown verb `{other}`"),
    }
}

fn split2(s: &str) -> (&str, &str) {
    match s.find(char::is_whitespace) {
        Some(i) => (&s[..i], s[i..].trim_start()),
        None => (s, ""),
    }
}

// ---------------------------------------------------------------------------
// Verbs
// ---------------------------------------------------------------------------

fn verb_action(rest: &str, ctx: &Ctx) -> Result<()> {
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
    let resp = ctx
        .client
        .post(format!("{}/action", ctx.base))
        .json(&body)
        .send()?;
    let status = resp.status();
    let json: Value = resp.json().unwrap_or(Value::Null);
    if !status.is_success() || !json.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        let err = json
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        bail!("action `{name}` failed ({status}): {err}");
    }
    Ok(())
}

fn verb_query(rest: &str, ctx: &Ctx) -> Result<Value> {
    let path = rest.trim();
    if path.is_empty() {
        bail!("query verb needs a path");
    }
    let url = format!("{}/query?path={}", ctx.base, urlencode(path));
    let resp = ctx.client.get(url).send()?;
    let json: Value = resp.json()?;
    if !json.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        let err = json
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        bail!("query `{path}` failed: {err}");
    }
    json.get("value")
        .cloned()
        .ok_or_else(|| anyhow!("server returned no `value`"))
}

fn verb_assert(rest: &str, ctx: &Ctx) -> Result<()> {
    let cmp = parse_compare(rest)?;
    let actual = verb_query(&cmp.path, ctx)?;
    if !cmp.matches(&actual) {
        bail!(
            "assertion failed: {} == {} (got {})",
            cmp.path,
            cmp.expected_text,
            actual
        );
    }
    if ctx.verbose {
        println!("    -> ok ({actual})");
    }
    Ok(())
}

fn verb_wait_until(rest: &str, ctx: &Ctx) -> Result<()> {
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
        "wait_until timed out after {:?}: {} != {} (last seen: {})",
        timeout,
        cmp.path,
        cmp.expected_text,
        last
    );
}

fn verb_key(rest: &str, ctx: &Ctx) -> Result<()> {
    let keystroke = rest.trim();
    if keystroke.is_empty() {
        bail!("key verb needs a keystroke");
    }
    let body = serde_json::json!({ "keystroke": keystroke });
    let resp = ctx
        .client
        .post(format!("{}/key", ctx.base))
        .json(&body)
        .send()?;
    let status = resp.status();
    let json: Value = resp.json().unwrap_or(Value::Null);
    if !status.is_success() || !json.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        let err = json
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        bail!("key `{keystroke}` failed ({status}): {err}");
    }
    Ok(())
}

fn verb_click(rest: &str, ctx: &Ctx) -> Result<()> {
    let selector = rest.trim();
    if selector.is_empty() {
        bail!("click verb needs a selector");
    }
    let body = serde_json::json!({ "selector": selector });
    let resp = ctx
        .client
        .post(format!("{}/click", ctx.base))
        .json(&body)
        .send()?;
    let status = resp.status();
    let json: Value = resp.json().unwrap_or(Value::Null);
    if !status.is_success() || !json.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        let err = json
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        bail!("click `{selector}` failed ({status}): {err}");
    }
    Ok(())
}

fn verb_elements(ctx: &Ctx) -> Result<()> {
    let resp = ctx.client.get(format!("{}/elements", ctx.base)).send()?;
    let json: Value = resp.json()?;
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

fn verb_focus(rest: &str, ctx: &Ctx) -> Result<()> {
    let target = rest.trim();
    if target.is_empty() {
        bail!("focus verb needs a screen name");
    }
    // Sugar: `focus library` → action SwitchToLibrary.
    let mut cap = String::new();
    let mut next_upper = true;
    for c in target.chars() {
        if c == '_' || c == '-' {
            next_upper = true;
            continue;
        }
        if next_upper {
            cap.extend(c.to_uppercase());
            next_upper = false;
        } else {
            cap.push(c);
        }
    }
    let action_name = format!("SwitchTo{cap}");
    verb_action(&action_name, ctx)
}

// ---------------------------------------------------------------------------
// Comparison parsing
// ---------------------------------------------------------------------------

struct Compare {
    path: String,
    expected: ExpectedValue,
    expected_text: String,
    tolerance: Option<f64>,
    timeout: Option<Duration>,
}

enum ExpectedValue {
    Bool(bool),
    Number(f64),
    String(String),
    Null,
}

impl Compare {
    fn matches(&self, actual: &Value) -> bool {
        match (&self.expected, actual) {
            (ExpectedValue::Bool(b), Value::Bool(a)) => a == b,
            (ExpectedValue::Number(n), Value::Number(a)) => match a.as_f64() {
                Some(av) => match self.tolerance {
                    Some(t) => (av - n).abs() <= t,
                    None => (av - n).abs() < f64::EPSILON,
                },
                None => false,
            },
            (ExpectedValue::String(s), Value::String(a)) => a == s,
            (ExpectedValue::Null, Value::Null) => true,
            _ => false,
        }
    }
}

fn parse_compare(rest: &str) -> Result<Compare> {
    // Split off trailing `tolerance=` / `timeout=` clauses.
    let mut tolerance = None;
    let mut timeout = None;
    let mut core = rest.to_string();
    loop {
        let trimmed = core.trim_end();
        if let Some((head, tail)) = trimmed.rsplit_once(char::is_whitespace) {
            if let Some(v) = tail.strip_prefix("tolerance=") {
                tolerance = Some(v.parse::<f64>().context("invalid tolerance value")?);
                core = head.to_string();
                continue;
            }
            if let Some(v) = tail.strip_prefix("timeout=") {
                timeout = Some(parse_duration(v)?);
                core = head.to_string();
                continue;
            }
        }
        break;
    }

    // Now `core` is `<path> == <literal>` with the tolerance/timeout
    // suffixes already stripped (see loop above). Splitting on `core`
    // — not the raw `rest` — ensures a literal that happens to contain
    // the substring `tolerance=` or `timeout=` survives intact.
    let (path, lit_text) = core
        .split_once("==")
        .ok_or_else(|| anyhow!("missing `==` in comparison"))?;
    let path = path.trim().to_string();
    let lit_text = lit_text.trim();
    let expected = parse_literal(lit_text)?;

    Ok(Compare {
        path,
        expected,
        expected_text: lit_text.to_string(),
        tolerance,
        timeout,
    })
}

fn parse_literal(s: &str) -> Result<ExpectedValue> {
    let s = s.trim();
    if s == "null" {
        return Ok(ExpectedValue::Null);
    }
    if s == "true" {
        return Ok(ExpectedValue::Bool(true));
    }
    if s == "false" {
        return Ok(ExpectedValue::Bool(false));
    }
    if let Ok(n) = s.parse::<f64>() {
        return Ok(ExpectedValue::Number(n));
    }
    // Treat as string. Strip surrounding quotes if present.
    let trimmed = s.trim_matches('"');
    Ok(ExpectedValue::String(trimmed.to_string()))
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim();
    if let Some(num) = s.strip_suffix("ms") {
        return Ok(Duration::from_millis(num.parse()?));
    }
    if let Some(num) = s.strip_suffix("s") {
        return Ok(Duration::from_secs_f64(num.parse()?));
    }
    if let Some(num) = s.strip_suffix("m") {
        let mins: f64 = num.parse()?;
        return Ok(Duration::from_secs_f64(mins * 60.0));
    }
    Ok(Duration::from_secs_f64(s.parse()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn comment_stripping() {
        assert_eq!(strip_comment("foo  # bar").trim(), "foo");
        assert_eq!(strip_comment("# only").trim(), "");
        assert_eq!(strip_comment("plain").trim(), "plain");
    }

    #[test]
    fn duration_suffixes() {
        assert_eq!(parse_duration("250ms").unwrap(), Duration::from_millis(250));
        assert_eq!(parse_duration("2s").unwrap(), Duration::from_secs(2));
        assert_eq!(parse_duration("0.5s").unwrap(), Duration::from_millis(500));
        assert_eq!(parse_duration("1m").unwrap(), Duration::from_secs(60));
        assert_eq!(parse_duration("3").unwrap(), Duration::from_secs(3));
    }

    #[test]
    fn compare_bool_match() {
        let cmp = parse_compare("playback.is_playing == true").unwrap();
        assert!(cmp.matches(&json!(true)));
        assert!(!cmp.matches(&json!(false)));
        assert!(!cmp.matches(&json!("true")));
    }

    #[test]
    fn compare_number_with_tolerance() {
        let cmp = parse_compare("playback.volume == 0.85 tolerance=0.01").unwrap();
        assert!(cmp.matches(&json!(0.851)));
        assert!(cmp.matches(&json!(0.845)));
        assert!(!cmp.matches(&json!(0.9)));
    }

    #[test]
    fn compare_string_quoted_and_bare() {
        let cmp = parse_compare(r#"screen.focused == "Library""#).unwrap();
        assert!(cmp.matches(&json!("Library")));
        assert!(!cmp.matches(&json!("Queue")));

        let cmp_bare = parse_compare("screen.focused == Queue").unwrap();
        assert!(cmp_bare.matches(&json!("Queue")));
    }

    #[test]
    fn compare_timeout_clause() {
        let cmp = parse_compare("queue.length == 3 timeout=500ms").unwrap();
        assert_eq!(cmp.timeout, Some(Duration::from_millis(500)));
        assert!(cmp.matches(&json!(3)));
    }

    #[test]
    fn urlencode_safe_chars() {
        assert_eq!(urlencode("playback.volume"), "playback.volume");
        assert_eq!(urlencode("a b"), "a%20b");
        assert_eq!(urlencode("a&b"), "a%26b");
    }
}

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
//!   export_room_eq_json [path]             Export completed RoomEQ DSP JSON
//!   elements                              (print every tracked selector; debugging aid)
//!
//! `<duration>` accepts `Ns`, `Nms`, `Nm`. Bare numbers default to seconds.

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use serde_json::Value;

mod suite;

const DEFAULT_DEV_API_URL: &str = "http://127.0.0.1:7777";

#[derive(Parser, Debug)]
#[command(name = "sotf-dev-driver", version)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
    /// Path to a .scn scenario file (legacy single-scenario mode).
    script: Option<PathBuf>,
    /// Base URL of the running SotF dev API.
    #[arg(long)]
    url: Option<String>,
    /// Print every verb + result.
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Start SotF with --qa and run every scenario in a suite TOML file.
    RunSuite {
        /// Path to a suite TOML file.
        suite: PathBuf,
        /// Print process and scenario details.
        #[arg(short, long)]
        verbose: bool,
    },
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
    match &args.command {
        Some(Command::RunSuite { suite, verbose }) => suite::run_suite(suite, *verbose),
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

fn resolve_base_url(cli_url: Option<&str>, env_port: Option<&str>) -> Result<String> {
    if let Some(url) = cli_url {
        return Ok(url.to_string());
    }
    let Some(port) = env_port.map(str::trim).filter(|port| !port.is_empty()) else {
        return Ok(DEFAULT_DEV_API_URL.to_string());
    };
    let port: u16 = port
        .parse()
        .with_context(|| format!("SOTF_DEV_API_PORT must be a TCP port, got `{port}`"))?;
    Ok(format!("http://127.0.0.1:{port}"))
}

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

struct Ctx {
    client: reqwest::blocking::Client,
    base: String,
    verbose: bool,
}

fn strip_comment(line: &str) -> &str {
    let mut in_quote = false;
    let mut escaped = false;
    for (idx, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_quote => escaped = true,
            '"' => in_quote = !in_quote,
            '#' if !in_quote => return &line[..idx],
            _ => {}
        }
    }
    line
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
    post_dev_json(ctx, "/action", &body, &format!("action `{name}`"))?;
    Ok(())
}

fn verb_query(rest: &str, ctx: &Ctx) -> Result<Value> {
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

fn verb_assert(rest: &str, ctx: &Ctx) -> Result<()> {
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
        "wait_until timed out after {:?}: {} {} {} (last seen: {})",
        timeout,
        cmp.path,
        cmp.op.as_str(),
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
    post_dev_json(ctx, "/key", &body, &format!("key `{keystroke}`"))?;
    Ok(())
}

fn verb_click(rest: &str, ctx: &Ctx) -> Result<()> {
    let selector = rest.trim();
    if selector.is_empty() {
        bail!("click verb needs a selector");
    }
    let body = serde_json::json!({ "selector": selector });
    post_dev_json(ctx, "/click", &body, &format!("click `{selector}`"))?;
    Ok(())
}

fn verb_elements(ctx: &Ctx) -> Result<()> {
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

fn verb_export_room_eq_json(rest: &str, ctx: &Ctx) -> Result<()> {
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

fn verb_focus(rest: &str, ctx: &Ctx) -> Result<()> {
    let target = rest.trim();
    if target.is_empty() {
        bail!("focus verb needs a screen name");
    }
    let action_name = focus_action_name(target)?;
    verb_action(&action_name, ctx)
}

fn focus_action_name(target: &str) -> Result<String> {
    if !target
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic())
    {
        bail!("focus screen name must start with an ASCII letter");
    }
    if !target
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        bail!("focus screen name may only contain ASCII letters, digits, `_`, or `-`");
    }
    let mut cap = String::new();
    let mut next_upper = true;
    for c in target.chars() {
        if c == '_' || c == '-' {
            next_upper = true;
            continue;
        }
        if next_upper {
            cap.push(c.to_ascii_uppercase());
            next_upper = false;
        } else {
            cap.push(c);
        }
    }
    Ok(format!("SwitchTo{cap}"))
}

fn post_dev_json(ctx: &Ctx, endpoint: &str, body: &Value, label: &str) -> Result<Value> {
    let resp = ctx
        .client
        .post(format!("{}{}", ctx.base, endpoint))
        .json(body)
        .send()?;
    parse_dev_response(resp, label)
}

pub(crate) fn parse_dev_response(resp: reqwest::blocking::Response, label: &str) -> Result<Value> {
    let status = resp.status();
    let body = resp
        .text()
        .with_context(|| format!("reading {label} response body"))?;
    parse_dev_response_body(status, &body, label)
}

fn parse_dev_response_body(status: reqwest::StatusCode, body: &str, label: &str) -> Result<Value> {
    let json = serde_json::from_str::<Value>(&body);
    if !status.is_success() {
        let err = json
            .as_ref()
            .ok()
            .and_then(|json| json.get("error"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| response_excerpt(&body));
        bail!("{label} failed ({status}): {err}");
    }
    let json = json.with_context(|| {
        format!(
            "{label} returned non-JSON response ({status}): {}",
            response_excerpt(&body)
        )
    })?;
    if json.get("ok").and_then(Value::as_bool) != Some(true) {
        let err = json
            .get("error")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| response_excerpt(&body));
        bail!("{label} failed ({status}): {err}");
    }
    Ok(json)
}

fn response_excerpt(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "empty response body".to_string();
    }
    let mut out = String::new();
    for (idx, ch) in trimmed.chars().enumerate() {
        if idx == 512 {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}

// ---------------------------------------------------------------------------
// Comparison parsing
// ---------------------------------------------------------------------------

struct Compare {
    path: String,
    op: ComparisonOp,
    expected: ExpectedValue,
    expected_text: String,
    tolerance: Option<f64>,
    timeout: Option<Duration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComparisonOp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

impl ComparisonOp {
    fn as_str(self) -> &'static str {
        match self {
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::Lt => "<",
            Self::Le => "<=",
        }
    }
}

enum ExpectedValue {
    Bool(bool),
    Number(f64),
    String(String),
    Null,
}

impl Compare {
    fn matches(&self, actual: &Value) -> bool {
        match (&self.expected, actual, self.op) {
            (ExpectedValue::Bool(b), Value::Bool(a), ComparisonOp::Eq) => a == b,
            (ExpectedValue::Bool(b), Value::Bool(a), ComparisonOp::Ne) => a != b,
            (ExpectedValue::Number(n), Value::Number(a), _) => match a.as_f64() {
                Some(av) => self.matches_number(av, *n),
                None => false,
            },
            (ExpectedValue::String(s), Value::String(a), ComparisonOp::Eq) => a == s,
            (ExpectedValue::String(s), Value::String(a), ComparisonOp::Ne) => a != s,
            (ExpectedValue::Null, Value::Null, ComparisonOp::Eq) => true,
            (ExpectedValue::Null, Value::Null, ComparisonOp::Ne) => false,
            (ExpectedValue::Null, _, ComparisonOp::Ne) => true,
            _ => false,
        }
    }

    fn matches_number(&self, actual: f64, expected: f64) -> bool {
        match self.op {
            ComparisonOp::Eq => match self.tolerance {
                Some(t) => (actual - expected).abs() <= t,
                None => nearly_equal(actual, expected),
            },
            ComparisonOp::Ne => match self.tolerance {
                Some(t) => (actual - expected).abs() > t,
                None => !nearly_equal(actual, expected),
            },
            ComparisonOp::Gt => actual > expected,
            ComparisonOp::Ge => actual >= expected,
            ComparisonOp::Lt => actual < expected,
            ComparisonOp::Le => actual <= expected,
        }
    }
}

fn nearly_equal(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= f64::EPSILON * actual.abs().max(expected.abs()).max(1.0)
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
                let parsed = v.parse::<f64>().context("invalid tolerance value")?;
                if !parsed.is_finite() || parsed < 0.0 {
                    bail!("tolerance must be finite and non-negative");
                }
                tolerance = Some(parsed);
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

    // Now `core` is `<path> <op> <literal>` with the tolerance/timeout
    // suffixes already stripped (see loop above). Splitting on `core`
    // — not the raw `rest` — ensures a literal that happens to contain
    // the substring `tolerance=` or `timeout=` survives intact.
    let (path, op, lit_text) = split_comparison(&core)?;
    let path = path.trim().to_string();
    let lit_text = lit_text.trim();
    let expected = parse_literal(lit_text)?;

    Ok(Compare {
        path,
        op,
        expected,
        expected_text: lit_text.to_string(),
        tolerance,
        timeout,
    })
}

fn split_comparison(core: &str) -> Result<(&str, ComparisonOp, &str)> {
    for (needle, op) in [
        (">=", ComparisonOp::Ge),
        ("<=", ComparisonOp::Le),
        ("!=", ComparisonOp::Ne),
        ("==", ComparisonOp::Eq),
        (">", ComparisonOp::Gt),
        ("<", ComparisonOp::Lt),
    ] {
        if let Some((path, lit_text)) = core.split_once(needle) {
            return Ok((path, op, lit_text));
        }
    }
    Err(anyhow!("missing comparison operator"))
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
        if !n.is_finite() {
            bail!("numeric literal must be finite");
        }
        return Ok(ExpectedValue::Number(n));
    }
    if s.starts_with('"') || s.ends_with('"') {
        let quoted = s
            .strip_prefix('"')
            .and_then(|inner| inner.strip_suffix('"'))
            .ok_or_else(|| anyhow!("string literal quotes must be balanced"))?;
        return Ok(ExpectedValue::String(quoted.to_string()));
    }
    Ok(ExpectedValue::String(s.to_string()))
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => write!(&mut out, "%{b:02X}").expect("writing to String cannot fail"),
        }
    }
    out
}

pub(crate) fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim();
    let (num, multiplier) = match s.strip_suffix("ms") {
        Some(num) => (num, 0.001),
        None => match s.chars().last() {
            Some('s') => (&s[..s.len() - 1], 1.0),
            Some('m') => (&s[..s.len() - 1], 60.0),
            _ => (s, 1.0),
        },
    };
    let value: f64 = num
        .parse()
        .with_context(|| format!("invalid duration `{s}`"))?;
    if !value.is_finite() || value < 0.0 {
        bail!("duration must be finite and non-negative");
    }
    Ok(Duration::from_secs_f64(value * multiplier))
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
        assert_eq!(
            strip_comment(r#"assert title == "issue #42" # trailing"#).trim(),
            r#"assert title == "issue #42""#
        );
    }

    #[test]
    fn duration_suffixes() {
        assert_eq!(parse_duration("250ms").unwrap(), Duration::from_millis(250));
        assert_eq!(parse_duration("2s").unwrap(), Duration::from_secs(2));
        assert_eq!(parse_duration("0.5s").unwrap(), Duration::from_millis(500));
        assert_eq!(parse_duration("1m").unwrap(), Duration::from_secs(60));
        assert_eq!(parse_duration("3").unwrap(), Duration::from_secs(3));
        assert_eq!(
            parse_duration("1.5ms").unwrap(),
            Duration::from_micros(1500)
        );
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
    fn compare_number_relative_epsilon_without_tolerance() {
        let cmp = parse_compare("playback.volume == 0.3").unwrap();
        assert!(cmp.matches(&json!(0.1f64 + 0.2f64)));
    }

    #[test]
    fn compare_number_ordering() {
        assert!(
            parse_compare("roomeq.filter_count > 0")
                .unwrap()
                .matches(&json!(8))
        );
        assert!(
            parse_compare("roomeq.filter_count >= 8")
                .unwrap()
                .matches(&json!(8))
        );
        assert!(
            parse_compare("roomeq.average_post_score < 35")
                .unwrap()
                .matches(&json!(26.5))
        );
        assert!(
            parse_compare("roomeq.average_post_score <= 26.5")
                .unwrap()
                .matches(&json!(26.5))
        );
        assert!(
            !parse_compare("roomeq.average_post_score < 20")
                .unwrap()
                .matches(&json!(26.5))
        );
    }

    #[test]
    fn compare_not_equal() {
        assert!(
            parse_compare("roomeq.error != null")
                .unwrap()
                .matches(&json!("boom"))
        );
        assert!(
            !parse_compare("roomeq.error != null")
                .unwrap()
                .matches(&Value::Null)
        );
        assert!(
            parse_compare("screen.focused != Library")
                .unwrap()
                .matches(&json!("RoomEq"))
        );
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
    fn compare_keeps_clause_like_text_inside_string_literal() {
        let cmp = parse_compare(r#"screen.focused == "tolerance=high" timeout=500ms"#).unwrap();
        assert_eq!(cmp.expected_text, r#""tolerance=high""#);
        assert_eq!(cmp.timeout, Some(Duration::from_millis(500)));
        assert!(cmp.matches(&json!("tolerance=high")));
    }

    #[test]
    fn compare_rejects_unbalanced_string_quotes() {
        assert!(parse_compare(r#"screen.focused == "Library"#).is_err());
        assert!(parse_compare(r#"screen.focused == Library""#).is_err());
    }

    #[test]
    fn focus_action_name_validates_names() {
        assert_eq!(focus_action_name("room_eq").unwrap(), "SwitchToRoomEq");
        assert_eq!(
            focus_action_name("headphone-eq").unwrap(),
            "SwitchToHeadphoneEq"
        );
        assert!(focus_action_name("2nd_screen").is_err());
        assert!(focus_action_name("room/eq").is_err());
    }

    #[test]
    fn base_url_prefers_cli_then_env_port_then_default() {
        assert_eq!(
            resolve_base_url(Some("http://127.0.0.1:9999"), Some("8888")).unwrap(),
            "http://127.0.0.1:9999"
        );
        assert_eq!(
            resolve_base_url(None, Some("8888")).unwrap(),
            "http://127.0.0.1:8888"
        );
        assert_eq!(resolve_base_url(None, None).unwrap(), DEFAULT_DEV_API_URL);
        assert!(resolve_base_url(None, Some("nope")).is_err());
    }

    #[test]
    fn query_error_includes_plain_text_response_body() {
        let err = parse_dev_response_body(
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            "plain panic details",
            "query `playback.volume`",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("query `playback.volume` failed"));
        assert!(err.contains("500 Internal Server Error"));
        assert!(err.contains("plain panic details"));
    }

    #[test]
    fn action_error_includes_json_error_message() {
        let err = parse_dev_response_body(
            reqwest::StatusCode::BAD_REQUEST,
            r#"{"ok":false,"error":"unknown action"}"#,
            "action `Nope`",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("action `Nope` failed"));
        assert!(err.contains("400 Bad Request"));
        assert!(err.contains("unknown action"));
    }

    #[test]
    fn urlencode_safe_chars() {
        assert_eq!(urlencode("playback.volume"), "playback.volume");
        assert_eq!(urlencode("a b"), "a%20b");
        assert_eq!(urlencode("a&b"), "a%26b");
    }
}

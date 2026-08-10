use super::compare::Compare;
use super::comparison_op::split_comparison;
use super::misc::response_excerpt;
use super::types::ExpectedValue;
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::time::Duration;

pub(crate) fn parse_dev_response(resp: reqwest::blocking::Response, label: &str) -> Result<Value> {
    let status = resp.status();
    let body = resp
        .text()
        .with_context(|| format!("reading {label} response body"))?;
    parse_dev_response_body(status, &body, label)
}

pub(super) fn parse_dev_response_body(
    status: reqwest::StatusCode,
    body: &str,
    label: &str,
) -> Result<Value> {
    let json = serde_json::from_str::<Value>(body);
    if !status.is_success() {
        let err = json
            .as_ref()
            .ok()
            .and_then(|json| json.get("error"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| response_excerpt(body));
        bail!("{label} failed ({status}): {err}");
    }
    let json = json.with_context(|| {
        format!(
            "{label} returned non-JSON response ({status}): {}",
            response_excerpt(body)
        )
    })?;
    if json.get("ok").and_then(Value::as_bool) != Some(true) {
        let err = json
            .get("error")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| response_excerpt(body));
        bail!("{label} failed ({status}): {err}");
    }
    Ok(json)
}

pub(super) fn parse_compare(rest: &str) -> Result<Compare> {
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

pub(super) fn parse_literal(s: &str) -> Result<ExpectedValue> {
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
        let quoted = serde_json::from_str::<String>(s)
            .with_context(|| format!("invalid quoted string literal `{s}`"))?;
        return Ok(ExpectedValue::String(quoted));
    }
    Ok(ExpectedValue::String(s.to_string()))
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

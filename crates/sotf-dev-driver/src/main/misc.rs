use anyhow::{Context, Result, bail};
use std::fmt::Write as _;

pub(super) const DEFAULT_DEV_API_URL: &str = "http://127.0.0.1:7777";

pub(super) fn resolve_base_url(cli_url: Option<&str>, env_port: Option<&str>) -> Result<String> {
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

pub(super) fn strip_comment(line: &str) -> &str {
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

pub fn expand_env_vars(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.char_indices().peekable();
    while let Some((i, ch)) = chars.next() {
        if ch == '$' {
            let start = i + 1;
            let end = loop {
                match chars.peek() {
                    Some((_, c)) if c.is_alphanumeric() || *c == '_' => {
                        chars.next();
                    }
                    Some((j, _)) => break *j,
                    None => break line.len(),
                }
            };
            let name = &line[start..end];
            match std::env::var(name) {
                Ok(val) => out.push_str(&val),
                Err(_) => out.push_str(&line[i..end]),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

pub(super) fn split2(s: &str) -> (&str, &str) {
    match s.find(char::is_whitespace) {
        Some(i) => (&s[..i], s[i..].trim_start()),
        None => (s, ""),
    }
}

pub(super) fn focus_action_name(target: &str) -> Result<String> {
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

pub(super) fn response_excerpt(body: &str) -> String {
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

pub(super) fn nearly_equal(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= f64::EPSILON * actual.abs().max(expected.abs()).max(1.0)
}

pub(super) fn urlencode(s: &str) -> String {
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

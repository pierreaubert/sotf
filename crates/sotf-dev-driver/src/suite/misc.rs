use crate::parse_dev_response;
use anyhow::Result;
use serde_json::Value;
use std::net::TcpListener;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn post_json(
    client: &reqwest::blocking::Client,
    base_url: &str,
    path: &str,
    body: Value,
) -> Result<Value> {
    let resp = client
        .post(format!("{base_url}{path}"))
        .json(&body)
        .send()?;
    parse_dev_response(resp, path)
}

pub(super) fn free_port() -> Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    Ok(listener.local_addr()?.port())
}

pub(super) fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(super) fn safe_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

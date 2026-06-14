use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::process::Child;
use std::thread::sleep;
use std::time::{Duration, Instant};

pub(super) fn wait_for_health(
    client: &reqwest::blocking::Client,
    base_url: &str,
    timeout: Duration,
    child: &mut Child,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last = String::new();
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            bail!("app exited before dev-api readiness: {status}");
        }
        match client.get(format!("{base_url}/health")).send() {
            Ok(resp) => match resp.json::<Value>() {
                Ok(json) if json.get("ok").and_then(Value::as_bool) == Some(true) => return Ok(()),
                Ok(json) => last = json.to_string(),
                Err(e) => last = e.to_string(),
            },
            Err(e) => last = e.to_string(),
        }
        sleep(Duration::from_millis(100));
    }
    bail!("dev-api did not become healthy after {timeout:?}; last error: {last}");
}

pub(super) fn wait_or_kill(child: &mut Child, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        sleep(Duration::from_millis(100));
    }
    child
        .kill()
        .context("killing app after graceful quit timeout")?;
    let _ = child.wait();
    Ok(())
}

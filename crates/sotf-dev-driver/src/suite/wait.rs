use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::path::Path;
use std::process::Child;
use std::thread::sleep;
use std::time::{Duration, Instant};

pub(super) fn wait_for_health(
    client: &reqwest::blocking::Client,
    base_url: &str,
    timeout: Duration,
    child: &mut Child,
    expected_run_id: &str,
    expected_qa_dir: &Path,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last = String::new();
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            bail!("app exited before dev-api readiness: {status}");
        }
        match client.get(format!("{base_url}/health")).send() {
            Ok(resp) => match resp.json::<Value>() {
                Ok(json) if health_matches(&json, expected_run_id, expected_qa_dir) => {
                    return Ok(());
                }
                Ok(json) => last = json.to_string(),
                Err(e) => last = e.to_string(),
            },
            Err(e) => last = e.to_string(),
        }
        sleep(Duration::from_millis(100));
    }
    bail!("dev-api did not become healthy after {timeout:?}; last error: {last}");
}

const DEV_API_PROTOCOL_VERSION: u64 = 1;

fn health_matches(json: &Value, expected_run_id: &str, expected_qa_dir: &Path) -> bool {
    let Some(payload) = json.get("value") else {
        return false;
    };

    json.get("ok").and_then(Value::as_bool) == Some(true)
        && payload.get("dev_api_enabled").and_then(Value::as_bool) == Some(true)
        && payload.get("protocol_version").and_then(Value::as_u64) == Some(DEV_API_PROTOCOL_VERSION)
        && payload.get("run_id").and_then(Value::as_str) == Some(expected_run_id)
        && payload.get("qa_directory").and_then(Value::as_str)
            == Some(expected_qa_dir.to_string_lossy().as_ref())
        && payload
            .get("binary")
            .and_then(|binary| binary.get("package"))
            .and_then(Value::as_str)
            .is_some_and(|package| !package.is_empty())
        && payload
            .get("process_started_at_unix_ms")
            .and_then(Value::as_u64)
            .is_some()
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

#[cfg(test)]
mod tests {
    use super::health_matches;
    use serde_json::json;
    use std::path::Path;

    fn health(run_id: &str) -> serde_json::Value {
        json!({
            "ok": true,
            "value": {
                "dev_api_enabled": true,
                "protocol_version": 1,
                "run_id": run_id,
                "qa_directory": "target/qa",
                "binary": { "package": "sotf-gpui" },
                "process_started_at_unix_ms": 1,
            }
        })
    }

    #[test]
    fn health_requires_matching_run_identity_and_qa_directory() {
        assert!(health_matches(
            &health("run-1"),
            "run-1",
            Path::new("target/qa")
        ));
        assert!(!health_matches(
            &health("stale-run"),
            "run-1",
            Path::new("target/qa")
        ));
        assert!(!health_matches(
            &health("run-1"),
            "run-1",
            Path::new("other-qa")
        ));
    }
}

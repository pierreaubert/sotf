use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::path::Path;
use std::process::Child;
use std::thread::sleep;
use std::time::{Duration, Instant};

/// The first successful health response can race startup audio-engine
/// reconfiguration on macOS. Require the UI thread to remain responsive long
/// enough for that work to begin and finish before sending fixture or input
/// commands.
const HEALTH_STABILITY_WINDOW: Duration = Duration::from_secs(2);

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
    let mut matching_since = None;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            bail!("app exited before dev-api readiness: {status}");
        }
        match client.get(format!("{base_url}/health")).send() {
            Ok(resp) => match resp.json::<Value>() {
                Ok(json) if health_matches(&json, expected_run_id, expected_qa_dir) => {
                    let now = Instant::now();
                    let since = matching_since.get_or_insert(now);
                    if now.duration_since(*since) >= HEALTH_STABILITY_WINDOW {
                        return Ok(());
                    }
                    last = format!(
                        "matching dev-api health; waiting for {HEALTH_STABILITY_WINDOW:?} responsiveness window"
                    );
                }
                Ok(json) => {
                    matching_since = None;
                    last = json.to_string();
                }
                Err(e) => {
                    matching_since = None;
                    last = e.to_string();
                }
            },
            Err(e) => {
                matching_since = None;
                last = e.to_string();
            }
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

/// Require at least one painted frame before scenarios start.
///
/// Health alone proves the dev-api serves requests on some thread;
/// it does not prove the UI ever painted. On display-less Linux the
/// GPUI headless platform must drive frames itself — when it cannot
/// (old GPUI build, no GPU adapter for screenshots is fine but no
/// frames at all is not), every selector assertion would fail with
/// misleading "not present" errors deep into the suite. Fail here
/// instead, with the actual cause named.
pub(super) fn wait_for_painted_frame(
    client: &reqwest::blocking::Client,
    base_url: &str,
    timeout: Duration,
    child: &mut Child,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last = String::new();
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            bail!("app exited before first painted frame: {status}");
        }
        match client.get(format!("{base_url}/elements")).send() {
            Ok(resp) => match resp.json::<Value>() {
                Ok(json) => {
                    if elements_route_unknown(&json) {
                        // Targets without a tracked-element surface
                        // (the TUI) have no painted frame to wait
                        // for; their health check already passed.
                        return Ok(());
                    }
                    let tracked = painted_elements_count(&json);
                    if tracked > 0 {
                        return Ok(());
                    }
                    last = format!("0 tracked elements painted: {json}");
                }
                Err(error) => last = error.to_string(),
            },
            Err(error) => last = error.to_string(),
        }
        sleep(Duration::from_millis(100));
    }
    bail!(
        "no painted frame after {timeout:?}: dev-api is healthy but the UI never          painted a tracked element. On Linux without DISPLAY/WAYLAND_DISPLAY this          means the GPUI headless platform is not driving frames in this build.          Last response: {last}"
    );
}

/// Whether the target answered `/elements` with route-not-found,
/// i.e. it has no tracked-element surface at all.
fn elements_route_unknown(json: &Value) -> bool {
    json.get("ok").and_then(Value::as_bool) == Some(false)
        && json
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|error| error.contains("unknown route: GET /elements"))
}

fn painted_elements_count(json: &Value) -> usize {
    // /elements replies carry the array either at the top level or
    // nested in the protocol envelope's `value`, depending on the
    // dev-api surface version; accept both.
    let elements = json
        .get("elements")
        .or_else(|| json.get("value").and_then(|value| value.get("elements")));
    elements.and_then(Value::as_array).map_or(0, Vec::len)
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
    use super::{elements_route_unknown, health_matches, painted_elements_count};
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
    fn painted_elements_count_reads_tracked_elements() {
        // Top-level shape served by /elements.
        assert_eq!(
            painted_elements_count(&json!({"elements": [{"selector": "a"}, {"selector": "b"}]})),
            2
        );
        assert_eq!(
            painted_elements_count(&json!({"value": {"elements": []}})),
            0
        );
        assert_eq!(
            painted_elements_count(&json!({"value": {"elements": [{"selector": "a"}]}})),
            1
        );
        assert_eq!(painted_elements_count(&json!({"ok": true})), 0);
    }

    #[test]
    fn elements_route_unknown_detects_tui_surface() {
        // The TUI dev-api has no /elements route; its error envelope
        // is how the painted-frame gate knows to skip non-GPUI apps.
        assert!(elements_route_unknown(
            &json!({"ok": false, "error": "unknown route: GET /elements"})
        ));
        assert!(!elements_route_unknown(
            &json!({"ok": false, "error": "unknown route: GET /health"})
        ));
        assert!(!elements_route_unknown(
            &json!({"ok": true, "elements": []})
        ));
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

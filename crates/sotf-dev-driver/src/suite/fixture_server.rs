use super::copy::copy_audio_fixtures;
use super::misc::{free_port, post_json, unix_timestamp};
use super::runner_config::{RunnerConfig, spawn_app};
use super::types::{FixtureServerConfig, ScenarioConfig};
use super::wait::{wait_for_health, wait_or_kill};
use crate::parse_duration;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::fs::{self, File};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

/// Fixed bearer token for the hermetic fixture server. Loopback only; the
/// value is also handed to the app under test through `/qa/remote-server`.
const FIXTURE_API_TOKEN: &str = "sotf-fixture-token";
const FIXTURE_SERVER_NAME: &str = "SOTF Fixture Server";

/// A running headless `sotf-desktop --server` process owned by the scenario.
/// Dropping the guard kills the process, including on error paths.
pub(super) struct FixtureServer {
    child: Child,
    api_base_url: String,
}

impl Drop for FixtureServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl FixtureServer {
    pub(super) fn registration_json(&self) -> Value {
        json!({
            "name": FIXTURE_SERVER_NAME,
            "api_base_url": self.api_base_url,
            "token": FIXTURE_API_TOKEN,
        })
    }
}

/// Start the fixture server for a scenario: optionally pre-seed its library,
/// write its `servers.json`, launch `--server`, wait for API health, and
/// optionally restart it once against the same config directory.
pub(super) fn start_fixture_server(
    runner: &RunnerConfig,
    scenario: &ScenarioConfig,
    scenario_dir: &Path,
    config: &FixtureServerConfig,
    verbose: bool,
) -> Result<FixtureServer> {
    let fixture_qa_dir = scenario_dir.join("fixture-server-qa");
    fs::create_dir_all(&fixture_qa_dir).with_context(|| format!("creating {fixture_qa_dir:?}"))?;

    if config.seed_demo_audio {
        let fixture_library = scenario_dir.join("fixture-library");
        copy_audio_fixtures(&runner.demo_audio_dir, &fixture_library)?;
        preseed_fixture_library(
            runner,
            scenario,
            scenario_dir,
            &fixture_qa_dir,
            &fixture_library,
            verbose,
        )
        .context("pre-seeding fixture server library")?;
    }

    let api_port = free_port()?;
    write_fixture_server_config(&fixture_qa_dir, api_port)?;

    let timeout = parse_duration(&runner.readiness_timeout)?;
    let mut server = spawn_fixture_server(runner, scenario_dir, &fixture_qa_dir, api_port, timeout)
        .context("starting fixture server")?;

    if config.restart_once {
        if verbose {
            println!("restarting fixture server for `{}`", scenario.name);
        }
        drop(server);
        server = spawn_fixture_server(runner, scenario_dir, &fixture_qa_dir, api_port, timeout)
            .context("restarting fixture server")?;
    }

    Ok(server)
}

fn write_fixture_server_config(qa_dir: &Path, api_port: u16) -> Result<()> {
    let config = json!({
        "mpd": {
            "enabled": false,
            "bind_address": "127.0.0.1",
            "port": free_port()?,
            "tls_enabled": false,
            "auth_mode": "Password",
            "password": "sotf-fixture",
            "trusted_client_fingerprints": []
        },
        "dlna": {
            "enabled": false,
            "bind_address": "127.0.0.1",
            "friendly_name": FIXTURE_SERVER_NAME,
            "port": free_port()?
        },
        "api": {
            "enabled": true,
            "bind_address": "127.0.0.1",
            "port": api_port,
            "friendly_name": FIXTURE_SERVER_NAME,
            "tls_enabled": false,
            "auth_token": FIXTURE_API_TOKEN
        }
    });
    fs::write(
        qa_dir.join("servers.json"),
        serde_json::to_vec_pretty(&config)?,
    )
    .context("writing fixture servers.json")
}

fn spawn_fixture_server(
    runner: &RunnerConfig,
    scenario_dir: &Path,
    qa_dir: &Path,
    api_port: u16,
    timeout: Duration,
) -> Result<FixtureServer> {
    let stdout = File::create(scenario_dir.join("fixture-server.stdout.log"))
        .context("creating fixture server stdout log")?;
    let stderr = File::create(scenario_dir.join("fixture-server.stderr.log"))
        .context("creating fixture server stderr log")?;
    let mut child = Command::new(&runner.app_bin)
        .arg("--server")
        .arg("--qa")
        .arg(qa_dir)
        .env("SOTF_SERVER_DISABLE_DISCOVERY", "1")
        .env("SOTF_QA_DIR", qa_dir.as_os_str())
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .with_context(|| format!("launching {} --server", runner.app_bin.display()))?;

    let api_base_url = format!("http://127.0.0.1:{api_port}");
    let client = reqwest::blocking::Client::builder()
        .pool_max_idle_per_host(0)
        .timeout(Duration::from_secs(2))
        .build()?;
    let deadline = Instant::now() + timeout;
    let mut last = String::new();
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            bail!("fixture server exited before API readiness: {status}");
        }
        match client.get(format!("{api_base_url}/api/v1/health")).send() {
            Ok(resp) if resp.status().is_success() => {
                return Ok(FixtureServer {
                    child,
                    api_base_url,
                });
            }
            Ok(resp) => last = format!("health status {}", resp.status()),
            Err(err) => last = err.to_string(),
        }
        sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
    bail!("fixture server API did not become healthy after {timeout:?}; last error: {last}");
}

/// Populate the fixture server's library database by booting the GUI app
/// once against the fixture config directory and seeding it through the
/// same `/qa/seed` path used for the app under test. Server mode loads its
/// library from that database, so this is the production seeding path.
fn preseed_fixture_library(
    runner: &RunnerConfig,
    scenario: &ScenarioConfig,
    scenario_dir: &Path,
    fixture_qa_dir: &Path,
    fixture_library: &Path,
    verbose: bool,
) -> Result<()> {
    let port = free_port()?;
    let base_url = format!("http://127.0.0.1:{port}");
    let run_id = format!("{port}-{}-fixture", unix_timestamp());
    let mut child = spawn_app(
        runner,
        scenario,
        scenario_dir,
        fixture_qa_dir,
        port,
        &run_id,
        verbose,
    )?;
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "x-sotf-dev-run-id",
        reqwest::header::HeaderValue::from_str(&run_id).context("building run ID header")?,
    );
    let client = reqwest::blocking::Client::builder()
        .default_headers(headers)
        .pool_max_idle_per_host(0)
        .timeout(Duration::from_secs(10))
        .build()?;

    let result = (|| -> Result<()> {
        wait_for_health(
            &client,
            &base_url,
            parse_duration(&runner.readiness_timeout)?,
            &mut child,
            &run_id,
            fixture_qa_dir,
        )?;
        post_json(
            &client,
            &base_url,
            "/qa/seed",
            json!({ "library_dirs": [fixture_library] }),
        )
        .context("seeding fixture library")?;
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if let Some(status) = child.try_wait()? {
                bail!("fixture pre-seed app exited during scan: {status}");
            }
            let count = client
                .get(format!("{base_url}/query?path=library.album_count"))
                .send()
                .and_then(|resp| resp.json::<Value>())
                .ok()
                .and_then(|json| json.get("value").and_then(Value::as_u64))
                .unwrap_or(0);
            if count > 0 {
                return Ok(());
            }
            if Instant::now() >= deadline {
                bail!("fixture library scan produced no albums within 60s");
            }
            sleep(Duration::from_millis(250));
        }
    })();

    let _ = post_json(&client, &base_url, "/quit", json!({}));
    wait_or_kill(&mut child, Duration::from_secs(5))?;
    result
}

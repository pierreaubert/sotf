use std::fs::{self, File};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Deserialize)]
struct SuiteFile {
    #[serde(default)]
    runner: RunnerConfig,
    #[serde(default, alias = "scenario")]
    scenarios: Vec<ScenarioConfig>,
}

#[derive(Debug, Deserialize)]
struct RunnerConfig {
    #[serde(default = "default_app_bin")]
    app_bin: PathBuf,
    #[serde(default)]
    app_args: Vec<String>,
    #[serde(default = "default_artifacts_dir")]
    artifacts_dir: PathBuf,
    #[serde(default = "default_demo_audio_dir")]
    demo_audio_dir: PathBuf,
    #[serde(default = "default_readiness_timeout")]
    readiness_timeout: String,
    #[serde(default)]
    size: Option<String>,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            app_bin: default_app_bin(),
            app_args: Vec::new(),
            artifacts_dir: default_artifacts_dir(),
            demo_audio_dir: default_demo_audio_dir(),
            readiness_timeout: default_readiness_timeout(),
            size: None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ScenarioConfig {
    name: String,
    path: PathBuf,
    #[serde(default = "default_scenario_timeout")]
    timeout: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    seed_demo_audio: bool,
    #[serde(default)]
    require_virtual_audio: bool,
    #[serde(default)]
    fake_recording: Option<FakeRecordingConfig>,
    #[serde(default)]
    room_eq: Option<RoomEqConfig>,
}

#[derive(Debug, Deserialize)]
struct FakeRecordingConfig {
    #[serde(default = "default_fake_channels")]
    channels: usize,
    #[serde(default = "default_fake_points")]
    points: usize,
}

#[derive(Debug, Deserialize)]
struct RoomEqConfig {
    fixture_dir: PathBuf,
    #[serde(default)]
    dist_path: Option<PathBuf>,
    target: String,
    loss: String,
    processing: String,
    crossover: String,
    #[serde(default = "default_room_eq_num_filters")]
    num_filters: usize,
    #[serde(default = "default_room_eq_max_iter")]
    max_iter: usize,
    #[serde(default = "default_room_eq_population")]
    population: usize,
    #[serde(default = "default_true")]
    start: bool,
}

fn default_app_bin() -> PathBuf {
    PathBuf::from("target/debug/sotf-desktop")
}

fn default_artifacts_dir() -> PathBuf {
    PathBuf::from("target/qa-gpui")
}

fn default_demo_audio_dir() -> PathBuf {
    PathBuf::from("crates/app-gpui/assets/demo-audio")
}

fn default_readiness_timeout() -> String {
    "20s".to_string()
}

fn default_scenario_timeout() -> String {
    "60s".to_string()
}

fn default_fake_channels() -> usize {
    2
}

fn default_fake_points() -> usize {
    48
}

fn default_room_eq_num_filters() -> usize {
    7
}

fn default_room_eq_max_iter() -> usize {
    20
}

fn default_room_eq_population() -> usize {
    24
}

fn default_true() -> bool {
    true
}

pub(crate) fn run_suite(path: &Path, verbose: bool) -> Result<()> {
    let source = fs::read_to_string(path).with_context(|| format!("reading {path:?}"))?;
    let suite: SuiteFile = toml::from_str(&source).context("parsing suite TOML")?;
    if suite.scenarios.is_empty() {
        bail!("suite has no [[scenario]] entries");
    }

    let run_dir = suite
        .runner
        .artifacts_dir
        .join(format!("run-{}", unix_timestamp()));
    fs::create_dir_all(&run_dir).with_context(|| format!("creating {run_dir:?}"))?;

    let mut passed = 0usize;
    let mut skipped = 0usize;
    for scenario in &suite.scenarios {
        let result = run_one(&suite.runner, scenario, &run_dir, verbose);
        match result {
            Ok(ScenarioOutcome::Passed) => {
                passed += 1;
                println!("PASS {}", scenario.name);
            }
            Ok(ScenarioOutcome::Skipped(reason)) => {
                skipped += 1;
                println!("SKIP {}: {reason}", scenario.name);
            }
            Err(e) => {
                bail!("scenario `{}` failed: {e:#}", scenario.name);
            }
        }
    }

    println!(
        "suite complete: {passed} passed, {skipped} skipped, artifacts at {}",
        run_dir.display()
    );
    Ok(())
}

enum ScenarioOutcome {
    Passed,
    Skipped(String),
}

fn run_one(
    runner: &RunnerConfig,
    scenario: &ScenarioConfig,
    run_dir: &Path,
    verbose: bool,
) -> Result<ScenarioOutcome> {
    if scenario.require_virtual_audio
        && std::env::var("AEQ_E2E_DEVICE")
            .unwrap_or_default()
            .is_empty()
    {
        return Ok(ScenarioOutcome::Skipped(
            "requires AEQ_E2E_DEVICE for virtual-audio routing".to_string(),
        ));
    }

    let scenario_dir = run_dir.join(safe_name(&scenario.name));
    let qa_dir = scenario_dir.join("qa");
    let seeded_library = scenario_dir.join("library");
    fs::create_dir_all(&qa_dir).with_context(|| format!("creating {qa_dir:?}"))?;
    fs::create_dir_all(&scenario_dir).with_context(|| format!("creating {scenario_dir:?}"))?;

    if scenario.seed_demo_audio {
        copy_audio_fixtures(&runner.demo_audio_dir, &seeded_library)?;
    }
    let room_eq_fixture_dir = scenario
        .room_eq
        .as_ref()
        .map(|fixture| copy_room_eq_fixture(fixture, &scenario_dir))
        .transpose()?;

    let port = free_port()?;
    let base_url = format!("http://127.0.0.1:{port}");
    let mut child = spawn_app(runner, scenario, &scenario_dir, &qa_dir, port, verbose)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let scenario_result = (|| {
        wait_for_health(
            &client,
            &base_url,
            parse_duration(&runner.readiness_timeout)?,
            &mut child,
        )?;

        if scenario.seed_demo_audio {
            post_json(
                &client,
                &base_url,
                "/qa/seed",
                json!({ "library_dirs": [seeded_library] }),
            )
            .context("seeding QA library directories")?;
        }

        if let Some(fake) = &scenario.fake_recording {
            post_json(
                &client,
                &base_url,
                "/qa/recording/fake-capture",
                json!({ "channels": fake.channels, "points": fake.points }),
            )
            .context("installing fake recording capture")?;
        }

        if let (Some(config), Some(fixture_dir)) = (&scenario.room_eq, &room_eq_fixture_dir) {
            post_json(
                &client,
                &base_url,
                "/qa/room-eq",
                json!({
                    "fixture_dir": fixture_dir,
                    "target": &config.target,
                    "loss": &config.loss,
                    "processing": &config.processing,
                    "crossover": &config.crossover,
                    "num_filters": config.num_filters,
                    "max_iter": config.max_iter,
                    "population": config.population,
                    "start": config.start,
                }),
            )
            .context("loading RoomEQ fixture")?;
        }

        let timeout = parse_duration(&scenario.timeout)?;
        let deadline = Instant::now() + timeout;
        crate::run_script(&scenario.path, &base_url, verbose)
            .with_context(|| format!("running {:?}", scenario.path))?;
        if Instant::now() > deadline {
            bail!("scenario exceeded timeout {timeout:?}");
        }
        Ok(())
    })();

    let _ = post_json(&client, &base_url, "/quit", json!({}));
    wait_or_kill(&mut child, Duration::from_secs(5))?;

    scenario_result?;
    Ok(ScenarioOutcome::Passed)
}

fn spawn_app(
    runner: &RunnerConfig,
    scenario: &ScenarioConfig,
    scenario_dir: &Path,
    qa_dir: &Path,
    port: u16,
    verbose: bool,
) -> Result<Child> {
    let stdout_path = scenario_dir.join("sotf.stdout.log");
    let stderr_path = scenario_dir.join("sotf.stderr.log");
    let stdout = File::create(&stdout_path).with_context(|| format!("creating {stdout_path:?}"))?;
    let stderr = File::create(&stderr_path).with_context(|| format!("creating {stderr_path:?}"))?;

    let mut cmd = Command::new(&runner.app_bin);
    cmd.arg("--qa").arg(qa_dir);
    if let Some(size) = &runner.size {
        cmd.arg("--size").arg(size);
    }
    cmd.args(&runner.app_args)
        .env("SOTF_DEV_API_PORT", port.to_string())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));

    if verbose {
        println!(
            "launching `{}` for `{}` on {port} tags={:?}",
            runner.app_bin.display(),
            scenario.name,
            scenario.tags
        );
    }

    cmd.spawn()
        .with_context(|| format!("launching {}", runner.app_bin.display()))
}

fn wait_for_health(
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

fn post_json(
    client: &reqwest::blocking::Client,
    base_url: &str,
    path: &str,
    body: Value,
) -> Result<Value> {
    let resp = client
        .post(format!("{base_url}{path}"))
        .json(&body)
        .send()?;
    let status = resp.status();
    let json: Value = resp.json().unwrap_or(Value::Null);
    if !status.is_success() || json.get("ok").and_then(Value::as_bool) != Some(true) {
        let err = json
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        bail!("{path} failed ({status}): {err}");
    }
    Ok(json)
}

fn wait_or_kill(child: &mut Child, timeout: Duration) -> Result<()> {
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

fn copy_audio_fixtures(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("creating {dst:?}"))?;
    for entry in fs::read_dir(src).with_context(|| format!("reading {src:?}"))? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !matches!(
            ext.to_ascii_lowercase().as_str(),
            "wav" | "flac" | "mp3" | "m4a" | "mp4" | "aac"
        ) {
            continue;
        }
        fs::copy(&path, dst.join(entry.file_name()))
            .with_context(|| format!("copying fixture {:?}", path))?;
    }
    Ok(())
}

fn copy_room_eq_fixture(config: &RoomEqConfig, scenario_dir: &Path) -> Result<PathBuf> {
    let source = &config.fixture_dir;
    if !source.is_dir() {
        bail!("RoomEQ fixture does not exist: {}", source.display());
    }

    let dist_path = config
        .dist_path
        .as_deref()
        .unwrap_or(config.fixture_dir.as_path());
    if dist_path.is_absolute() {
        bail!(
            "RoomEQ dist_path must be relative, got {}",
            dist_path.display()
        );
    }

    let dst = scenario_dir.join("dist").join(dist_path);
    copy_dir_recursive(source, &dst)?;
    Ok(dst)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("creating {dst:?}"))?;
    for entry in fs::read_dir(src).with_context(|| format!("reading {src:?}"))? {
        let entry = entry?;
        let path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_recursive(&path, &dst_path)?;
        } else if file_type.is_file() {
            fs::copy(&path, &dst_path)
                .with_context(|| format!("copying {} to {}", path.display(), dst_path.display()))?;
        }
    }
    Ok(())
}

fn free_port() -> Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    Ok(listener.local_addr()?.port())
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn safe_name(name: &str) -> String {
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

    #[test]
    fn parses_suite_with_scenario_alias() {
        let src = r#"
            [runner]
            app_bin = "target/debug/sotf-desktop"

            [[scenario]]
            name = "smoke"
            path = "crates/sotf-dev-driver/scenarios/smoke.scn"
            seed_demo_audio = true
            tags = ["smoke"]

            [scenario.fake_recording]
            channels = 2
            points = 48

            [scenario.room_eq]
            fixture_dir = "crates/autoeq/data_tests/roomeq/measured/2.0_d3v"
            dist_path = "crates/autoeq/data_tests/roomeq/measured/2.0_d3v"
            target = "NearField"
            loss = "Flat"
            processing = "Iir"
            crossover = "Lr24"
            num_filters = 7
            max_iter = 20
            population = 24
        "#;
        let suite: SuiteFile = toml::from_str(src).unwrap();
        assert_eq!(suite.scenarios.len(), 1);
        assert_eq!(suite.scenarios[0].name, "smoke");
        assert!(suite.scenarios[0].seed_demo_audio);
        let fake = suite.scenarios[0].fake_recording.as_ref().unwrap();
        assert_eq!(fake.channels, 2);
        assert_eq!(fake.points, 48);
        let room_eq = suite.scenarios[0].room_eq.as_ref().unwrap();
        assert_eq!(room_eq.target, "NearField");
        assert_eq!(room_eq.loss, "Flat");
        assert_eq!(room_eq.processing, "Iir");
        assert_eq!(room_eq.crossover, "Lr24");
        assert_eq!(room_eq.num_filters, 7);
        assert_eq!(room_eq.max_iter, 20);
        assert_eq!(room_eq.population, 24);
        assert!(room_eq.start);
    }

    #[test]
    fn safe_name_removes_path_punctuation() {
        assert_eq!(safe_name("Player / Smoke"), "Player---Smoke");
    }

    #[test]
    fn checked_in_suites_parse() {
        let smoke: SuiteFile = toml::from_str(include_str!("../suites/smoke.toml")).unwrap();
        assert!(!smoke.scenarios.is_empty());

        let roomeq: SuiteFile =
            toml::from_str(include_str!("../suites/roomeq_matrix.toml")).unwrap();
        assert_eq!(roomeq.scenarios.len(), 24);

        let tui: SuiteFile = toml::from_str(include_str!("../suites/tui.toml")).unwrap();
        assert_eq!(tui.scenarios.len(), 18);

        let full: SuiteFile = toml::from_str(include_str!("../suites/full_matrix.toml")).unwrap();
        assert_eq!(full.scenarios.len(), 15);
    }
}

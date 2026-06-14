use super::copy::copy_audio_fixtures;
use super::copy::copy_room_eq_fixture;
use super::misc::free_port;
use super::misc::post_json;
use super::misc::safe_name;
use super::misc::unix_timestamp;
use super::runner_config::RunnerConfig;
use super::runner_config::spawn_app;
use super::types::ScenarioConfig;
use super::types::ScenarioOutcome;
use super::types::SuiteFile;
use super::wait::wait_for_health;
use super::wait::wait_or_kill;
use crate::parse_duration;
use anyhow::{Context, Result, bail};
use serde_json::json;
use std::fs::{self};
use std::path::Path;
use std::time::{Duration, Instant};

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

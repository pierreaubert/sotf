use super::clean_log::assert_clean_logs;
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
use serde_json::Map;
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
    let mut failures = Vec::new();
    let mut outcomes = Vec::with_capacity(suite.scenarios.len());
    for scenario in &suite.scenarios {
        let result = run_one(&suite.runner, scenario, &run_dir, verbose);
        match result {
            Ok(ScenarioOutcome::Passed) => {
                passed += 1;
                println!("PASS {}", scenario.name);
                outcomes.push(json!({ "name": scenario.name, "status": "passed" }));
            }
            Ok(ScenarioOutcome::Skipped(reason)) => {
                skipped += 1;
                println!("SKIP {}: {reason}", scenario.name);
                outcomes.push(json!({
                    "name": scenario.name,
                    "status": "skipped",
                    "reason": reason,
                }));
            }
            Err(e) => {
                let message = format!("{e:#}");
                eprintln!("FAIL {}: {message}", scenario.name);
                failures.push(scenario.name.clone());
                outcomes.push(json!({
                    "name": scenario.name,
                    "status": "failed",
                    "error": message,
                }));
            }
        }
    }

    let summary_path = run_dir.join("summary.json");
    let summary = json!({
        "passed": passed,
        "skipped": skipped,
        "failed": failures.len(),
        "scenarios": outcomes,
    });
    fs::write(
        &summary_path,
        serde_json::to_vec_pretty(&summary).expect("JSON suite summary is serializable"),
    )
    .with_context(|| format!("writing {summary_path:?}"))?;
    write_junit_report(&run_dir, &outcomes, passed, skipped, failures.len())?;
    write_html_report(&run_dir, &outcomes)?;

    if !failures.is_empty() {
        bail!(
            "{} scenario(s) failed ({}); artifacts at {}",
            failures.len(),
            failures.join(", "),
            run_dir.display()
        );
    }

    println!(
        "suite complete: {passed} passed, {skipped} skipped, artifacts at {}",
        run_dir.display()
    );
    Ok(())
}

fn write_junit_report(
    run_dir: &Path,
    outcomes: &[serde_json::Value],
    passed: usize,
    skipped: usize,
    failed: usize,
) -> Result<()> {
    let cases = outcomes
        .iter()
        .map(|outcome| {
            let name = xml_escape(outcome["name"].as_str().unwrap_or("unknown"));
            match outcome["status"].as_str() {
                Some("skipped") => format!("<testcase name=\"{name}\"><skipped/></testcase>"),
                Some("failed") => format!(
                    "<testcase name=\"{name}\"><failure message=\"{}\"/></testcase>",
                    xml_escape(outcome["error"].as_str().unwrap_or("scenario failed"))
                ),
                _ => format!("<testcase name=\"{name}\"/>"),
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let report = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuite name=\"sotf-gpui\" tests=\"{}\" failures=\"{failed}\" skipped=\"{skipped}\">\n{cases}\n</testsuite>\n",
        passed + skipped + failed
    );
    let path = run_dir.join("junit.xml");
    fs::write(&path, report).with_context(|| format!("writing {path:?}"))
}

fn write_html_report(run_dir: &Path, outcomes: &[serde_json::Value]) -> Result<()> {
    let rows = outcomes
        .iter()
        .map(|outcome| {
            let name = html_escape(outcome["name"].as_str().unwrap_or("unknown"));
            let status = html_escape(outcome["status"].as_str().unwrap_or("unknown"));
            let detail = html_escape(
                outcome["error"]
                    .as_str()
                    .or_else(|| outcome["reason"].as_str())
                    .unwrap_or(""),
            );
            format!("<tr><td>{name}</td><td>{status}</td><td><pre>{detail}</pre></td></tr>")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let report = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>SOTF GPUI suite</title><style>body{{font-family:system-ui;margin:2rem}}table{{border-collapse:collapse;width:100%}}td,th{{border:1px solid #bbb;padding:.5rem;text-align:left}}pre{{margin:0;white-space:pre-wrap}}</style><h1>SOTF GPUI suite</h1><table><thead><tr><th>Scenario</th><th>Status</th><th>Detail</th></tr></thead><tbody>{rows}</tbody></table>"
    );
    let path = run_dir.join("summary.html");
    fs::write(&path, report).with_context(|| format!("writing {path:?}"))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
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
    // The child echoes this nonce from `/health`; that prevents a suite from
    // accepting a stale dev-api process listening on a recycled port.
    let run_id = format!("{port}-{}", unix_timestamp());
    let mut child = spawn_app(
        runner,
        scenario,
        &scenario_dir,
        &qa_dir,
        port,
        &run_id,
        verbose,
    )?;
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "x-sotf-dev-run-id",
        reqwest::header::HeaderValue::from_str(&run_id)
            .context("building dev-api run ID header")?,
    );
    let client = reqwest::blocking::Client::builder()
        .default_headers(headers)
        // QA targets serve one request per connection and close it explicitly.
        .pool_max_idle_per_host(0)
        .timeout(Duration::from_secs(10))
        .build()?;

    let scenario_result = (|| {
        wait_for_health(
            &client,
            &base_url,
            parse_duration(&runner.readiness_timeout)?,
            &mut child,
            &run_id,
            &qa_dir,
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
                json!({
                    "channels": fake.channels,
                    "points": fake.points,
                    "fault": fake.fault,
                }),
            )
            .context("installing fake recording capture")?;
        }

        if let Some(config) = &scenario.headphone_discovery {
            let downloads = config
                .downloads
                .iter()
                .map(|download| {
                    (
                        download.headphone.clone(),
                        json!({
                        "path": &download.path,
                        "points": &download.points,
                        "delay_ms": download.delay_ms,
                        "failures": download.failures,
                        "failure_message": download.failure_message,
                        }),
                    )
                })
                .collect::<Map<_, _>>();
            post_json(
                &client,
                &base_url,
                "/qa/headphone/discovery-fixture",
                json!({ "catalog": &config.catalog, "downloads": downloads }),
            )
            .context("installing Headphone EQ discovery fixture")?;
        }

        if let Some(config) = &scenario.spinorama_discovery {
            let mut responses = Map::new();
            let speakers = config
                .speakers
                .iter()
                .map(|speaker| {
                    let versions = speaker
                        .versions
                        .iter()
                        .map(|version| {
                            if !version.response.is_empty() {
                                responses.insert(
                                    format!(
                                        "{}|{}|{}",
                                        speaker.speaker,
                                        version.version,
                                        version.measurements.first().cloned().unwrap_or_default()
                                    ),
                                    json!(&version.response),
                                );
                            }
                            (version.version.clone(), json!(&version.measurements))
                        })
                        .collect::<Map<_, _>>();
                    (speaker.speaker.clone(), json!({ "versions": versions }))
                })
                .collect::<Map<_, _>>();
            post_json(
                &client,
                &base_url,
                "/qa/spinorama/discovery-fixture",
                json!({
                    "catalog": &config.catalog,
                    "catalog_delay_ms": config.catalog_delay_ms,
                    "catalog_failures": config.catalog_failures,
                    "catalog_failure_message": config.catalog_failure_message,
                    "speakers": speakers,
                    "responses": responses,
                }),
            )
            .context("installing Spinorama discovery fixture")?;
        }

        if let (Some(config), Some(fixture_dir)) = (&scenario.room_eq, &room_eq_fixture_dir) {
            let endpoint = if config.ui_driven {
                "/qa/room-eq/ui-fixture"
            } else {
                "/qa/room-eq"
            };
            let payload = if config.ui_driven {
                json!({ "fixture_dir": fixture_dir, "invalid": config.invalid })
            } else {
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
                })
            };
            post_json(&client, &base_url, endpoint, payload).context("loading RoomEQ fixture")?;
        }

        let timeout = parse_duration(&scenario.timeout)?;
        let deadline = Instant::now() + timeout;
        crate::run_script_with_run_id(&scenario.path, &base_url, verbose, Some(&run_id))
            .with_context(|| format!("running {:?}", scenario.path))?;
        if Instant::now() > deadline {
            bail!("scenario exceeded timeout {timeout:?}");
        }
        Ok(())
    })();

    let _ = post_json(&client, &base_url, "/quit", json!({}));
    wait_or_kill(&mut child, Duration::from_secs(5))?;

    scenario_result?;
    assert_clean_logs(&scenario_dir, &scenario.allowed_log_patterns)?;
    Ok(ScenarioOutcome::Passed)
}

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
use super::types::ScenarioTimings;
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
    let suite_started = Instant::now();
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
        let scenario_started = Instant::now();
        let mut timings = ScenarioTimings::default();
        let result = run_one(&suite.runner, scenario, &run_dir, verbose, &mut timings);
        timings.total_ms = elapsed_ms(scenario_started);
        match result {
            Ok(ScenarioOutcome::Passed) => {
                passed += 1;
                println!("PASS {}", scenario.name);
                outcomes.push(json!({
                    "name": scenario.name,
                    "status": "passed",
                    "timings": timings,
                }));
            }
            Ok(ScenarioOutcome::Skipped(reason)) => {
                skipped += 1;
                println!("SKIP {}: {reason}", scenario.name);
                outcomes.push(json!({
                    "name": scenario.name,
                    "status": "skipped",
                    "reason": reason,
                    "timings": timings,
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
                    "timings": timings,
                }));
            }
        }
    }

    let suite_duration_ms = elapsed_ms(suite_started);
    let summary_path = run_dir.join("summary.json");
    let summary = json!({
        "passed": passed,
        "skipped": skipped,
        "failed": failures.len(),
        "duration_ms": suite_duration_ms,
        "scenarios": outcomes,
    });
    fs::write(
        &summary_path,
        serde_json::to_vec_pretty(&summary).expect("JSON suite summary is serializable"),
    )
    .with_context(|| format!("writing {summary_path:?}"))?;
    write_junit_report(
        &run_dir,
        &outcomes,
        passed,
        skipped,
        failures.len(),
        suite_duration_ms,
    )?;
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
    suite_duration_ms: u64,
) -> Result<()> {
    let report = junit_report(outcomes, passed, skipped, failed, suite_duration_ms);
    let path = run_dir.join("junit.xml");
    fs::write(&path, report).with_context(|| format!("writing {path:?}"))
}

fn junit_report(
    outcomes: &[serde_json::Value],
    passed: usize,
    skipped: usize,
    failed: usize,
    suite_duration_ms: u64,
) -> String {
    let cases = outcomes
        .iter()
        .map(|outcome| {
            let name = xml_escape(outcome["name"].as_str().unwrap_or("unknown"));
            let time = duration_seconds(outcome);
            match outcome["status"].as_str() {
                Some("skipped") => {
                    format!("<testcase name=\"{name}\" time=\"{time:.3}\"><skipped/></testcase>")
                }
                Some("failed") => format!(
                    "<testcase name=\"{name}\" time=\"{time:.3}\"><failure message=\"{}\"/></testcase>",
                    xml_escape(outcome["error"].as_str().unwrap_or("scenario failed"))
                ),
                _ => format!("<testcase name=\"{name}\" time=\"{time:.3}\"/>"),
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let suite_time = suite_duration_ms as f64 / 1_000.0;
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuite name=\"sotf-gpui\" tests=\"{}\" failures=\"{failed}\" skipped=\"{skipped}\" time=\"{suite_time:.3}\">\n{cases}\n</testsuite>\n",
        passed + skipped + failed
    )
}

fn write_html_report(run_dir: &Path, outcomes: &[serde_json::Value]) -> Result<()> {
    let report = html_report(outcomes);
    let path = run_dir.join("summary.html");
    fs::write(&path, report).with_context(|| format!("writing {path:?}"))
}

fn html_report(outcomes: &[serde_json::Value]) -> String {
    let rows = outcomes
        .iter()
        .map(|outcome| {
            let name = html_escape(outcome["name"].as_str().unwrap_or("unknown"));
            let status = html_escape(outcome["status"].as_str().unwrap_or("unknown"));
            let duration = outcome["timings"]["total_ms"].as_u64().unwrap_or(0);
            let phases = html_escape(&timing_summary(outcome));
            let detail = html_escape(
                outcome["error"]
                    .as_str()
                    .or_else(|| outcome["reason"].as_str())
                    .unwrap_or(""),
            );
            format!(
                "<tr><td>{name}</td><td>{status}</td><td>{duration} ms</td><td>{phases}</td><td><pre>{detail}</pre></td></tr>"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<!doctype html><meta charset=\"utf-8\"><title>SOTF GPUI suite</title><style>body{{font-family:system-ui;margin:2rem}}table{{border-collapse:collapse;width:100%}}td,th{{border:1px solid #bbb;padding:.5rem;text-align:left}}pre{{margin:0;white-space:pre-wrap}}</style><h1>SOTF GPUI suite</h1><table><thead><tr><th>Scenario</th><th>Status</th><th>Duration</th><th>Lifecycle</th><th>Detail</th></tr></thead><tbody>{rows}</tbody></table>"
    )
}

fn duration_seconds(outcome: &serde_json::Value) -> f64 {
    outcome["timings"]["total_ms"].as_u64().unwrap_or(0) as f64 / 1_000.0
}

fn timing_summary(outcome: &serde_json::Value) -> String {
    const PHASES: [(&str, &str); 7] = [
        ("prepare_ms", "prepare"),
        ("spawn_ms", "spawn"),
        ("readiness_ms", "readiness"),
        ("fixtures_ms", "fixtures"),
        ("script_ms", "script"),
        ("shutdown_ms", "shutdown"),
        ("log_audit_ms", "log audit"),
    ];
    let mut parts = PHASES
        .iter()
        .filter_map(|(field, label)| {
            outcome["timings"][field]
                .as_u64()
                .map(|duration| format!("{label}: {duration} ms"))
        })
        .collect::<Vec<_>>();
    if let Some(operations) = outcome["timings"]["operations"].as_array() {
        parts.extend(operations.iter().filter_map(|operation| {
            let name = operation["name"].as_str()?;
            let duration_ms = operation["duration_ms"].as_u64()?;
            Some(match operation["budget_ms"].as_u64() {
                Some(budget_ms) => {
                    format!("metric {name}: {duration_ms} ms (budget ≤ {budget_ms} ms)")
                }
                None => format!("metric {name}: {duration_ms} ms"),
            })
        }));
    }
    parts.join(" · ")
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
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
    timings: &mut ScenarioTimings,
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

    let prepare_started = Instant::now();
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
    timings.prepare_ms = Some(elapsed_ms(prepare_started));
    let spawn_started = Instant::now();
    let mut child = spawn_app(
        runner,
        scenario,
        &scenario_dir,
        &qa_dir,
        port,
        &run_id,
        verbose,
    )?;
    timings.spawn_ms = Some(elapsed_ms(spawn_started));
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
        let readiness_started = Instant::now();
        wait_for_health(
            &client,
            &base_url,
            parse_duration(&runner.readiness_timeout)?,
            &mut child,
            &run_id,
            &qa_dir,
        )?;
        timings.readiness_ms = Some(elapsed_ms(readiness_started));

        let fixtures_started = Instant::now();
        if scenario.theme.is_some()
            || scenario.language.is_some()
            || scenario.font_scale.is_some()
            || scenario.reduced_motion.is_some()
            || scenario.release_channel.is_some()
        {
            post_json(
                &client,
                &base_url,
                "/qa/ui-environment",
                json!({
                    "theme": scenario.theme,
                    "language": scenario.language,
                    "font_scale": scenario.font_scale,
                    "reduced_motion": scenario.reduced_motion,
                    "release_channel": scenario.release_channel,
                }),
            )
            .context("applying scenario UI environment")?;
        }

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
                "catalog_requests": config.catalog_requests.iter().map(|request| json!({
                    "delay_ms": request.delay_ms,
                    "fail": request.fail,
                })).collect::<Vec<_>>(),
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
        timings.fixtures_ms = Some(elapsed_ms(fixtures_started));

        let timeout = parse_duration(&scenario.timeout)?;
        let deadline = Instant::now() + timeout;
        let script_started = Instant::now();
        let script_report =
            crate::run_script_with_run_id(&scenario.path, &base_url, verbose, Some(&run_id))
                .with_context(|| format!("running {:?}", scenario.path))?;
        let budget_result = script_report.ensure_budgets();
        timings.operations = script_report.named;
        timings.script_ms = Some(elapsed_ms(script_started));
        budget_result?;
        if Instant::now() > deadline {
            bail!("scenario exceeded timeout {timeout:?}");
        }
        Ok(())
    })();

    let shutdown_started = Instant::now();
    let _ = post_json(&client, &base_url, "/quit", json!({}));
    wait_or_kill(&mut child, Duration::from_secs(5))?;
    timings.shutdown_ms = Some(elapsed_ms(shutdown_started));

    scenario_result?;
    let log_audit_started = Instant::now();
    assert_clean_logs(&scenario_dir, &scenario.allowed_log_patterns)?;
    timings.log_audit_ms = Some(elapsed_ms(log_audit_started));
    Ok(ScenarioOutcome::Passed)
}

#[cfg(test)]
mod report_tests {
    use super::*;

    fn outcomes() -> Vec<serde_json::Value> {
        vec![
            json!({
                "name": "passes & measures",
                "status": "passed",
                "timings": {
                    "total_ms": 1_250,
                    "prepare_ms": 10,
                    "spawn_ms": 20,
                    "readiness_ms": 30,
                    "fixtures_ms": 40,
                    "script_ms": 1_100,
                    "shutdown_ms": 45,
                    "log_audit_ms": 5,
                    "operations": [
                        {
                            "name": "input_response",
                            "duration_ms": 12,
                            "budget_ms": 20,
                            "within_budget": true
                        },
                    ],
                },
            }),
            json!({
                "name": "fails",
                "status": "failed",
                "error": "bad <state>",
                "timings": { "total_ms": 500 },
            }),
        ]
    }

    #[test]
    fn junit_report_includes_suite_and_scenario_durations() {
        let report = junit_report(&outcomes(), 1, 0, 1, 1_750);
        assert!(report.contains("time=\"1.750\""));
        assert!(report.contains("name=\"passes &amp; measures\" time=\"1.250\""));
        assert!(report.contains("message=\"bad &lt;state&gt;\""));
    }

    #[test]
    fn html_report_includes_lifecycle_breakdown() {
        let report = html_report(&outcomes());
        assert!(report.contains("<td>1250 ms</td>"));
        assert!(report.contains("readiness: 30 ms"));
        assert!(report.contains("script: 1100 ms"));
        assert!(report.contains("metric input_response: 12 ms (budget ≤ 20 ms)"));
        assert!(report.contains("bad &lt;state&gt;"));
    }
}

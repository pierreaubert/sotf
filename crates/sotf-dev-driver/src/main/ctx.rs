use super::misc::split2;
use super::misc::strip_comment;
use super::parse::parse_dev_response;
use super::parse::parse_duration;
use super::types::Ctx;
use super::verb::verb_accessibility;
use super::verb::verb_action;
use super::verb::verb_assert;
use super::verb::verb_assert_absent;
use super::verb::verb_assert_accessible;
use super::verb::verb_assert_accessible_masked;
use super::verb::verb_assert_element_state;
use super::verb::verb_assert_focused;
use super::verb::verb_assert_in_viewport;
use super::verb::verb_assert_inaccessible;
use super::verb::verb_assert_non_overlapping;
use super::verb::verb_assert_h_centered;
use super::verb::verb_assert_perceptual_snapshot;
use super::verb::verb_assert_snapshot;
use super::verb::verb_assert_visible;
use super::verb::verb_click;
use super::verb::verb_drag;
use super::verb::verb_elements;
use super::verb::verb_export_room_eq_json;
use super::verb::verb_focus;
use super::verb::verb_hover;
use super::verb::verb_key;
use super::verb::verb_plugin_add;
use super::verb::verb_plugin_chain_load;
use super::verb::verb_plugin_chain_save;
use super::verb::verb_plugin_clear;
use super::verb::verb_plugin_count;
use super::verb::verb_plugin_param_count;
use super::verb::verb_plugin_param_get;
use super::verb::verb_plugin_param_set;
use super::verb::verb_plugin_remove;
use super::verb::verb_query;
use super::verb::verb_resize;
use super::verb::verb_screenshot;
use super::verb::verb_scroll;
use super::verb::verb_scroll_at;
use super::verb::verb_scroll_into_view;
use super::verb::verb_type;
use super::verb::verb_wait_idle;
use super::verb::verb_wait_visible;
use super::verb::verb_wait_until;
use super::verb::{verb_click_at, verb_double_click_at};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct NamedTiming {
    pub(crate) name: String,
    pub(crate) duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) budget_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) within_budget: Option<bool>,
}

#[derive(Debug, Default)]
pub(crate) struct ScriptTimingReport {
    pub(crate) named: Vec<NamedTiming>,
}

#[derive(Debug, Default)]
struct TimingCollector {
    active: BTreeMap<String, Instant>,
    completed: Vec<NamedTiming>,
}

pub(crate) fn run_script(script: &PathBuf, url: &str, verbose: bool) -> Result<()> {
    let report = run_script_with_run_id(script, url, verbose, None, None)?;
    report.ensure_budgets()
}

pub(crate) fn run_script_with_run_id(
    script: &PathBuf,
    url: &str,
    verbose: bool,
    run_id: Option<&str>,
    qa_directory: Option<&str>,
) -> Result<ScriptTimingReport> {
    let source = fs::read_to_string(script).with_context(|| format!("reading {:?}", script))?;
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(run_id) = run_id {
        headers.insert(
            "x-sotf-dev-run-id",
            reqwest::header::HeaderValue::from_str(run_id)
                .context("building dev-api run ID header")?,
        );
    }
    let client = reqwest::blocking::Client::builder()
        .default_headers(headers)
        // The QA targets close every response connection. Keep no idle socket
        // around for a later scripted command to race with that close.
        .pool_max_idle_per_host(0)
        .timeout(Duration::from_secs(30))
        .build()?;
    let ctx = Ctx {
        client,
        base: url.trim_end_matches('/').to_string(),
        verbose,
    };
    let mut timing = TimingCollector::default();

    for (lineno, raw) in source.lines().enumerate() {
        let lineno = lineno + 1;
        let line = strip_comment(raw);
        let expanded = crate::misc::expand_env_vars_in_qa(line, qa_directory);
        let line = expanded.trim();
        if line.is_empty() {
            continue;
        }
        let diagnostic_line = scenario_line_for_diagnostics(line);
        if ctx.verbose {
            println!("[{lineno:>3}] {diagnostic_line}");
        }
        let (verb, rest) = split2(line);
        match verb {
            "qa_empty_dir" => {
                fixture_empty_directory(qa_directory, rest)
                    .with_context(|| format!("line {lineno}: `{diagnostic_line}`"))?;
                continue;
            }
            "timing_start" => {
                timing
                    .start(rest)
                    .with_context(|| format!("line {lineno}: `{diagnostic_line}`"))?;
                continue;
            }
            "timing_end" => {
                timing
                    .finish(rest)
                    .with_context(|| format!("line {lineno}: `{diagnostic_line}`"))?;
                continue;
            }
            _ => {}
        }
        execute(line, &ctx).with_context(|| format!("line {lineno}: `{diagnostic_line}`"))?;
    }
    timing.finish_report()
}

#[cfg(test)]
mod fixture_directory_tests {
    use super::fixture_empty_directory;

    #[test]
    fn fixture_directory_is_confined_and_never_removes_contents() {
        let root = tempfile::tempdir().unwrap();
        let qa = root.path().to_str();
        assert!(fixture_empty_directory(None, "create collision").is_err());
        assert!(fixture_empty_directory(qa, "create ../outside").is_err());
        assert!(fixture_empty_directory(qa, "create /tmp/outside").is_err());
        assert!(fixture_empty_directory(qa, "remove .").is_err());
        fixture_empty_directory(qa, "create collision").unwrap();
        let file = root.path().join("collision/keep.txt");
        std::fs::write(&file, "preserved").unwrap();
        assert!(fixture_empty_directory(qa, "remove collision").is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "preserved");
        std::fs::remove_file(file).unwrap();
        fixture_empty_directory(qa, "remove collision").unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn fixture_directory_rejects_symlink_escape() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();
        assert!(fixture_empty_directory(root.path().to_str(), "create link/escape").is_err());
        assert!(fixture_empty_directory(root.path().to_str(), "remove link").is_err());
        assert!(!outside.path().join("escape").exists());
    }
}

fn fixture_empty_directory(qa_directory: Option<&str>, arguments: &str) -> Result<()> {
    use std::path::{Component, Path};
    let (operation, relative) = split2(arguments);
    if !matches!(operation, "create" | "remove") {
        bail!("qa_empty_dir requires create or remove");
    }
    let relative = Path::new(relative.trim());
    if relative.as_os_str().is_empty()
        || !relative
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        bail!("qa_empty_dir requires a relative path without traversal");
    }
    let root = Path::new(qa_directory.context("qa_empty_dir requires an isolated QA directory")?)
        .canonicalize()?;
    let target = root.join(relative);
    let parent = target
        .parent()
        .context("fixture path has no parent")?
        .canonicalize()?;
    if !parent.starts_with(&root) {
        bail!("fixture directory must stay inside the QA directory");
    }
    if fs::symlink_metadata(&target).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        bail!("fixture directory must not be a symlink");
    }
    match operation {
        "create" => fs::create_dir(&target),
        "remove" => fs::remove_dir(&target), // Never remove contents or recurse.
        _ => unreachable!(),
    }
    .with_context(|| format!("{operation} empty fixture directory {}", target.display()))
}

impl TimingCollector {
    fn start(&mut self, raw_name: &str) -> Result<()> {
        let name = timing_name(raw_name)?;
        if self.active.contains_key(name) || self.completed.iter().any(|timing| timing.name == name)
        {
            bail!("timing `{name}` is already defined");
        }
        self.active.insert(name.to_string(), Instant::now());
        Ok(())
    }

    fn finish(&mut self, raw_spec: &str) -> Result<()> {
        let (name, budget_ms) = timing_end_spec(raw_spec)?;
        let started = self
            .active
            .remove(&name)
            .with_context(|| format!("timing `{name}` was not started"))?;
        let duration_ms = elapsed_ms(started);
        self.completed.push(NamedTiming {
            name,
            duration_ms,
            budget_ms,
            within_budget: budget_ms.map(|budget_ms| duration_ms <= budget_ms),
        });
        Ok(())
    }

    fn finish_report(self) -> Result<ScriptTimingReport> {
        if !self.active.is_empty() {
            bail!(
                "unclosed timing span(s): {}",
                self.active.keys().cloned().collect::<Vec<_>>().join(", ")
            );
        }
        Ok(ScriptTimingReport {
            named: self.completed,
        })
    }
}

impl ScriptTimingReport {
    pub(crate) fn ensure_budgets(&self) -> Result<()> {
        let violations = self
            .named
            .iter()
            .filter_map(|timing| {
                let budget_ms = timing.budget_ms?;
                (timing.duration_ms > budget_ms).then(|| {
                    format!(
                        "timing `{}` exceeded its budget: {} ms > {} ms",
                        timing.name, timing.duration_ms, budget_ms
                    )
                })
            })
            .collect::<Vec<_>>();
        if !violations.is_empty() {
            bail!("performance budget violation(s): {}", violations.join("; "));
        }
        Ok(())
    }
}

fn timing_name(raw_name: &str) -> Result<&str> {
    let name = raw_name.trim();
    if name.is_empty()
        || name.chars().any(char::is_whitespace)
        || !name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    {
        bail!("timing name must contain only ASCII letters, numbers, '.', '-', or '_': `{name}`");
    }
    Ok(name)
}

fn timing_end_spec(raw_spec: &str) -> Result<(String, Option<u64>)> {
    let mut fields = raw_spec.split_whitespace();
    let name = timing_name(fields.next().unwrap_or_default())?.to_string();
    let budget_ms = fields
        .next()
        .map(|field| {
            let raw_budget = field
                .strip_prefix("max=")
                .with_context(|| format!("unknown timing_end option `{field}`"))?;
            let budget = parse_duration(raw_budget).context("invalid timing_end max duration")?;
            u64::try_from(budget.as_millis()).context("timing_end max duration is too large")
        })
        .transpose()?;
    if let Some(extra) = fields.next() {
        bail!("unexpected timing_end option `{extra}`");
    }
    Ok((name, budget_ms))
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod timing_tests {
    use super::*;

    #[test]
    fn timing_collector_records_named_span() {
        let mut collector = TimingCollector::default();
        collector.start("first_meaningful_paint").unwrap();
        collector.finish("first_meaningful_paint max=1s").unwrap();
        let report = collector.finish_report().unwrap();
        assert_eq!(report.named.len(), 1);
        assert_eq!(report.named[0].name, "first_meaningful_paint");
        assert_eq!(report.named[0].budget_ms, Some(1_000));
        assert_eq!(report.named[0].within_budget, Some(true));
        report.ensure_budgets().unwrap();
    }

    #[test]
    fn timing_collector_rejects_ambiguous_or_unclosed_spans() {
        let mut collector = TimingCollector::default();
        assert!(collector.start("input response").is_err());
        collector.start("input_response").unwrap();
        assert!(collector.start("input_response").is_err());
        assert!(collector.finish("navigation").is_err());
        assert!(collector.finish_report().is_err());
    }

    #[test]
    fn timing_budget_violation_is_actionable() {
        let report = ScriptTimingReport {
            named: vec![NamedTiming {
                name: "search".to_string(),
                duration_ms: 501,
                budget_ms: Some(500),
                within_budget: Some(false),
            }],
        };
        let error = report.ensure_budgets().unwrap_err().to_string();
        assert!(error.contains("timing `search` exceeded its budget: 501 ms > 500 ms"));
    }

    #[test]
    fn timing_end_rejects_unknown_or_duplicate_options() {
        assert!(timing_end_spec("input budget=1s").is_err());
        assert!(timing_end_spec("input max=1s max=2s").is_err());
        assert!(timing_end_spec("input max=eventually").is_err());
    }
}

fn execute(line: &str, ctx: &Ctx) -> Result<()> {
    let (verb, rest) = split2(line);
    match verb {
        "snapshot" => {
            let path = rest.trim();
            if path.is_empty() {
                anyhow::bail!("snapshot needs an output path");
            }
            let response = ctx.client.get(format!("{}/snapshot", ctx.base)).send()?;
            let snapshot = parse_dev_response(response, "diagnostic snapshot")?;
            std::fs::write(path, serde_json::to_vec_pretty(&snapshot)?)?;
            Ok(())
        }
        "action" => verb_action(rest, ctx),
        "query" => verb_query(rest, ctx).map(|v| {
            if ctx.verbose {
                println!("    -> {v}");
            }
        }),
        "assert" => verb_assert(rest, ctx),
        "assert_accessible" => verb_assert_accessible(rest, ctx),
        "assert_accessible_masked" => verb_assert_accessible_masked(rest, ctx),
        "assert_inaccessible" => verb_assert_inaccessible(rest, ctx),
        "assert_focused" => verb_assert_focused(rest, ctx),
        "assert_snapshot" => verb_assert_snapshot(rest, ctx),
        "assert_perceptual_snapshot" => verb_assert_perceptual_snapshot(rest, ctx),
        "wait_until" => verb_wait_until(rest, ctx),
        "wait_visible" => verb_wait_visible(rest, ctx),
        "wait_idle" => verb_wait_idle(rest, ctx),
        "sleep" => {
            let dur = parse_duration(rest.trim())?;
            sleep(dur);
            Ok(())
        }
        "focus" => verb_focus(rest, ctx),
        "key" => verb_key(rest, ctx),
        "type" | "type_secret" => verb_type(rest, ctx),
        "click" => verb_click(rest, ctx),
        "click_at" => verb_click_at(rest, ctx),
        "double_click_at" => verb_double_click_at(rest, ctx),
        "hover" => verb_hover(rest, ctx),
        "drag" => verb_drag(rest, ctx),
        "scroll" => verb_scroll(rest, ctx),
        "scroll_at" => verb_scroll_at(rest, ctx),
        "scroll_into_view" => verb_scroll_into_view(rest, ctx),
        "resize" => verb_resize(rest, ctx),
        "screenshot" => verb_screenshot(rest, ctx),
        "assert_visible" => verb_assert_visible(rest, ctx),
        "assert_absent" => verb_assert_absent(rest, ctx),
        "assert_in_viewport" => verb_assert_in_viewport(rest, ctx),
        "assert_non_overlapping" => verb_assert_non_overlapping(rest, ctx),
        "assert_h_centered" => verb_assert_h_centered(rest, ctx),
        "assert_enabled" => verb_assert_element_state(rest, "enabled", ctx),
        "assert_selected" => verb_assert_element_state(rest, "selected", ctx),
        "assert_expanded" => verb_assert_element_state(rest, "expanded", ctx),
        "assert_text" => verb_assert_element_state(rest, "text", ctx),
        "export_room_eq_json" | "export_roomeq_json" => verb_export_room_eq_json(rest, ctx),
        "elements" => verb_elements(ctx),
        "accessibility" => verb_accessibility(ctx),
        "plugin_add" => verb_plugin_add(rest, ctx),
        "plugin_remove" => verb_plugin_remove(rest, ctx),
        "plugin_clear" => verb_plugin_clear(rest, ctx),
        "plugin_count" => verb_plugin_count(rest, ctx).map(|v| println!("    -> {v}")),
        "plugin_param_count" => verb_plugin_param_count(rest, ctx).map(|v| println!("    -> {v}")),
        "plugin_param_set" => verb_plugin_param_set(rest, ctx),
        "plugin_param_get" => verb_plugin_param_get(rest, ctx).map(|v| println!("    -> {v}")),
        "plugin_chain_save" => verb_plugin_chain_save(rest, ctx),
        "plugin_chain_load" => verb_plugin_chain_load(rest, ctx),
        other => bail!("unknown verb `{other}`"),
    }
}

pub(super) fn scenario_line_for_diagnostics(line: &str) -> &str {
    if split2(line).0 == "type_secret" {
        "type_secret [REDACTED]"
    } else {
        line
    }
}

pub(super) fn post_dev_json(ctx: &Ctx, endpoint: &str, body: &Value, label: &str) -> Result<Value> {
    let resp = ctx
        .client
        .post(format!("{}{}", ctx.base, endpoint))
        .json(body)
        .send()?;
    parse_dev_response(resp, label)
}

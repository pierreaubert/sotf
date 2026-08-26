use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::model::{Failure, StructuredSkip, TargetId};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunSummary {
    pub schema_version: u16,
    pub target: TargetId,
    pub seed: u64,
    pub outcome: String,
    pub steps: u64,
    pub duration_ms: u64,
    pub failures: Vec<Failure>,
    pub skip: Option<StructuredSkip>,
    pub coverage_keys: usize,
    pub opt_ins: Vec<String>,
}

pub fn write_reports(run_dir: &Path, summary: &RunSummary) -> Result<(), ReportError> {
    fs::write(
        run_dir.join("summary.json"),
        serde_json::to_vec_pretty(summary)?,
    )?;
    fs::write(run_dir.join("junit.xml"), junit(summary))?;
    fs::write(run_dir.join("summary.html"), html(summary))?;
    Ok(())
}

fn junit(summary: &RunSummary) -> String {
    let failure = summary.failures.first().map(|failure| {
        format!(
            "<failure message=\"{}\"/>",
            xml_escape(&failure.signature.normalized)
        )
    });
    let skipped = summary
        .skip
        .as_ref()
        .map(|skip| format!("<skipped message=\"{}\"/>", xml_escape(&skip.reason)));
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><testsuite name=\"sotf-fuzz\" tests=\"1\" failures=\"{}\" skipped=\"{}\"><testcase name=\"{}\">{}{}</testcase></testsuite>\n",
        usize::from(failure.is_some()),
        usize::from(skipped.is_some()),
        summary.target,
        failure.unwrap_or_default(),
        skipped.unwrap_or_default(),
    )
}

fn html(summary: &RunSummary) -> String {
    let json = serde_json::to_string_pretty(summary).unwrap_or_else(|_| "{}".into());
    format!(
        "<!doctype html><meta charset=\"utf-8\"><title>SOTF fuzz {}</title><h1>SOTF fuzz: {}</h1><pre>{}</pre>\n",
        summary.target,
        summary.target,
        html_escape(&json)
    )
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
    xml_escape(value)
}

#[derive(Debug, Error)]
pub enum ReportError {
    #[error("report I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("report JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn emits_json_junit_and_html() {
        let dir = tempdir().unwrap();
        let summary = RunSummary {
            schema_version: 1,
            target: TargetId::Tui,
            seed: 42,
            outcome: "passed".into(),
            steps: 10,
            duration_ms: 20,
            failures: vec![],
            skip: None,
            coverage_keys: 3,
            opt_ins: vec![],
        };
        write_reports(dir.path(), &summary).unwrap();
        assert!(dir.path().join("summary.json").is_file());
        assert!(dir.path().join("junit.xml").is_file());
        assert!(dir.path().join("summary.html").is_file());
    }
}

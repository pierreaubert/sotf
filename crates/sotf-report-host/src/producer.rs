//! Generates shared AutoEQ reports entirely in-process.
//!
//! Payload composition is delegated to AutoEQ's native dataset composer
//! (`autoeq_report_wasm::report_dataset_payload`); file-backed report
//! inputs are prepared by [`crate::prepare`]. No Python runtime, staged
//! scripts, or subprocesses are involved, so generation works identically
//! in desktop, mobile, and sandboxed hosts.

// Rust guideline compliant 2026-02-21

use anyhow::Context;
use std::path::{Path, PathBuf};

use crate::prepare::prepare_report;

/// Generated document and its private working directory.
#[derive(Debug)]
pub struct GeneratedReport {
    directory: tempfile::TempDir,
    html_path: PathBuf,
}

impl GeneratedReport {
    /// Read the self-contained HTML/WASM document.
    ///
    /// # Errors
    /// Returns an error if the generated file cannot be read.
    pub fn read_html(&self) -> anyhow::Result<String> {
        std::fs::read_to_string(&self.html_path).context("Reading shared AutoEQ report")
    }

    /// Generated report path, valid while this object lives.
    pub fn path(&self) -> &Path {
        &self.html_path
    }

    /// Save the same document for standalone viewing.
    ///
    /// # Errors
    /// Returns an error if the destination cannot be written.
    pub fn save(&self, destination: &Path) -> anyhow::Result<()> {
        std::fs::copy(&self.html_path, destination).context("Saving shared AutoEQ report")?;
        Ok(())
    }

    /// Directory retained for diagnostics during this report's lifetime.
    pub fn working_directory(&self) -> &Path {
        self.directory.path()
    }
}

/// Generate every dashboard using AutoEQ's native report composer.
///
/// Run this function on a background worker. `artifact_root` resolves
/// relative response/IR sidecars without rewriting their identities.
///
/// # Errors
/// Returns an error for invalid results, missing artifacts, or composer
/// failures.
pub fn generate_report(
    result: &serde_json::Value,
    artifact_root: Option<&Path>,
) -> anyhow::Result<GeneratedReport> {
    generate_report_with_quality(result, artifact_root, None)
}

/// Generate the shared dashboards with source-bound capture evidence.
///
/// The additional section is assembled by the upstream native/WASM report
/// library. The host supplies evidence and does not render a dashboard.
///
/// # Errors
/// Returns generation errors or invalid capture-quality evidence.
pub fn generate_report_with_quality(
    result: &serde_json::Value,
    artifact_root: Option<&Path>,
    quality: Option<&autoeq_report_wasm::capture_quality::CaptureQualityReport>,
) -> anyhow::Result<GeneratedReport> {
    let (prepared, options) = prepare_report(result, artifact_root)?;
    let dataset =
        autoeq_report_wasm::report_dataset_payload::build_dataset_report(&prepared, &options)
            .map_err(anyhow::Error::msg)?;
    let mut payload = dataset.payload;
    // Validation and dashboard grouping belong to the same upstream library
    // used by standalone Rust producers and WASM consumers.
    let empty_quality = autoeq_report_wasm::capture_quality::CaptureQualityReport::default();
    autoeq_report_wasm::capture_quality::append_capture_quality_section_to_value(
        &mut payload,
        quality.unwrap_or(&empty_quality),
    )?;
    let title = payload["title"]
        .as_str()
        .context("Report title is missing")?;
    let assets = autoeq_report_wasm::assemble::checked_in_assets().map_err(anyhow::Error::msg)?;
    let html = autoeq_report_wasm::assemble::assemble_html(
        title,
        &serde_json::to_string(&payload)?,
        &assets,
    );
    let directory = tempfile::Builder::new().prefix("sotf-report-").tempdir()?;
    let html_path = directory.path().join("report.html");
    std::fs::write(&html_path, html)?;
    Ok(GeneratedReport {
        directory,
        html_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_delegates_all_dashboard_groups_to_autoeq() {
        let result = serde_json::json!({"version":"1.0", "channels":{}, "metadata":{}});
        let report = generate_report(&result, None).unwrap();
        let html = report.read_html().unwrap();
        for group in [
            "DSP analysis",
            "Acoustics analysis",
            "Psychoacoustic report",
            "Why this correction?",
            "Time domain analysis",
            "DSP signal flow",
        ] {
            assert!(html.contains(group), "Missing shared dashboard {group}");
        }
        assert!(html.contains("autoeq-report-data-v1"));
        assert!(!html.contains("{{WASM_2D_B64}}"));
        assert!(!html.contains("{{PAYLOAD_JSON}}"));
        let exported = report.working_directory().join("standalone.html");
        report.save(&exported).unwrap();
        assert_eq!(std::fs::read_to_string(exported).unwrap(), html);
    }

    #[test]
    fn generation_needs_no_python_runtime() {
        // Generation is fully in-process: even a minimal result produces a
        // document without any interpreter on the host.
        let result = serde_json::json!({"version":"1.0", "channels":{}, "metadata":{}});
        let html = generate_report(&result, None).unwrap().read_html().unwrap();
        assert!(html.contains("autoeq-report-data-v1"));
    }

    #[test]
    fn invalid_artifact_root_is_reported() {
        let result = serde_json::json!({"channels":{}});
        let error =
            generate_report(&result, Some(Path::new("/missing/sotf-artifacts"))).unwrap_err();
        assert!(
            error.to_string().contains("AutoEQ report artifacts")
                || error.to_string().contains("artifact")
        );
    }

    #[test]
    fn shared_quality_dashboard_preserves_the_existing_payload() {
        use autoeq_report_wasm::capture_quality::CaptureQualityReport;
        let quality: CaptureQualityReport = serde_json::from_value(serde_json::json!({"sources":[{
            "source_id":"take-L-mic-2-position-3", "channel":"L", "mic":"2", "position":"3",
            "stimulus":{"signal_type":"sweep","sample_rate_hz":48000,"duration_secs":4.25,"level_dbfs":-30.0},
            "findings":[
                {"metric":{"kind":"thd"},"status":"warning","value":25.76,"unit":"%",
                 "policy":"SingleTake","method":"Farina harmonic IR","threshold":{"comparison":"less_than_or_equal","value":10.0,"unit":"%"},
                 "recommendation":"Lower playback level and retake"},
                {"metric":{"kind":"broadband_snr"},"status":"unknown","unit":"dB",
                 "unknown_reason":"Digital silence does not estimate ambient noise"}
            ]
        }]})).unwrap();
        let result = serde_json::json!({"version":"1.0","channels":{},"metadata":{}});
        let report = generate_report(&result, None).unwrap();
        let read_payload = |html: &str| -> serde_json::Value {
            serde_json::from_str(
                html.split_once("<script id=\"report-payload\" type=\"application/json\">")
                    .unwrap()
                    .1
                    .split_once("</script>")
                    .unwrap()
                    .0,
            )
            .unwrap()
        };
        let original = read_payload(&report.read_html().unwrap());
        let report = generate_report_with_quality(&result, None, Some(&quality)).unwrap();
        let enriched = read_payload(&report.read_html().unwrap());
        let old_sections = original["sections"].as_array().unwrap();
        let new_sections = enriched["sections"].as_array().unwrap();
        assert_eq!(&new_sections[..old_sections.len()], old_sections.as_slice());
        assert_eq!(new_sections.len(), old_sections.len() + 1);
        let evidence = new_sections.last().unwrap()["html"].as_str().unwrap();
        for label in [
            "Capture quality",
            "Warning",
            "Unknown",
            "25.76",
            "SingleTake",
            "Farina harmonic IR",
            "Digital silence",
            "-30",
        ] {
            assert!(
                evidence.contains(label),
                "Missing retained evidence {label}"
            );
        }
        assert_eq!(new_sections.last().unwrap()["bucket"], "DSP analysis");
        if let Some(destination) = std::env::var_os("SOTF_REPORT_QA_HTML") {
            report.save(Path::new(&destination)).unwrap();
        }
    }

    #[test]
    fn upstream_sti_dashboard_retains_measured_basis_and_missing_evidence() {
        let result = serde_json::json!({
            "version":"1.0", "metadata":{}, "channels":{
                "L":{"plugins":[], "speech_transmission":{
                    "method":"iec_60268_16_2020_indirect_ir_only_v1",
                    "basis":"measured_room_ir", "sti":0.625,
                    "sample_rate_hz":48000, "duration_s":2.0,
                    "octave_centers_hz":[125,250,500,1000,2000,4000,8000],
                    "modulation_frequencies_hz":[0.63,0.8,1.0,1.25,1.6,2.0,2.5,3.15,4.0,5.0,6.3,8.0,10.0,12.5],
                    "mti":vec![0.625; 7],
                    "modulation_transfer":vec![vec![0.625; 7]; 14]
                }},
                "R":{"plugins":[]}
            }
        });
        let report = generate_report(&result, None).unwrap();
        let html = report.read_html().unwrap();
        assert!(html.contains("Full STI (IR-only): 0.625"));
        assert!(html.contains("STI unavailable"));
        assert!(html.contains("not a post-EQ prediction"));
    }

    #[test]
    fn slim_result_curves_are_loaded_by_the_upstream_bundle_loader() {
        let artifacts = tempfile::tempdir().unwrap();
        std::fs::write(
            artifacts.path().join("initial.csv"),
            "freq,spl\n20,0\n100,2\n1833,1.5\n10000,-1\n20000,-2\n",
        )
        .unwrap();
        std::fs::write(
            artifacts.path().join("eq.csv"),
            "freq,spl\n20,0\n100,-2\n1833,-1.5\n10000,1\n20000,2\n",
        )
        .unwrap();
        std::fs::write(
            artifacts.path().join("measurements_index.json"),
            r#"{"channels":{"L":{"initial_curve":"initial.csv","eq_response":"eq.csv"}}}"#,
        )
        .unwrap();
        let result = serde_json::json!({"version":"1.0","channels":{"L":{"plugins":[],"initial_curve":null,"eq_response":null}},"metadata":{}});
        let report = generate_report(&result, Some(artifacts.path())).unwrap();
        let html = report.read_html().unwrap();
        let payload = html
            .split("<script id=\"report-payload\" type=\"application/json\">")
            .nth(1)
            .unwrap()
            .split("</script>")
            .next()
            .unwrap();
        let payload: serde_json::Value = serde_json::from_str(payload).unwrap();
        assert!(
            payload.to_string().contains("1833"),
            "Sidecar frequency must reach the shared dashboard payload"
        );
        assert_eq!(
            result["channels"]["L"]["initial_curve"],
            serde_json::Value::Null
        );
    }
}

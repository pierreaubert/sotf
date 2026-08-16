//! Machine-checkable external validation artifacts for AAE.
//!
//! This module intentionally does not create listening data.  It validates a
//! manifest and ingests results produced by the documented, level-matched
//! double-blind protocol.  Missing external artifacts always produce an
//! explicit non-acceptance report.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

pub const MANIFEST_SCHEMA_VERSION: u32 = 1;
pub const REQUIRED_FIXTURE_LABELS: &[&str] = &[
    "clean_dialogue",
    "noisy_dialogue",
    "off_centre_dialogue",
    "hard_panned_dialogue",
    "mono_music",
    "stereo_music",
    "percussion",
    "anti_phase",
    "diffuse_ambience",
    "sustained_bass",
    "transient_full_scale",
];
pub const REQUIRED_LAYOUTS: &[&str] = &["5.1", "9.1.6"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationManifest {
    pub schema_version: u32,
    pub fixtures: Vec<FixtureMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureMetadata {
    pub fixture_id: String,
    pub label: String,
    pub license_or_source: String,
    /// Lowercase SHA-256 digest of the exact source file.
    pub sha256: String,
    pub layout: String,
    pub sample_rate: u32,
    pub expected_dialogue_regions: Vec<DialogueRegion>,
    /// Optional path. Restricted fixtures may omit this and be acquired from
    /// the source named above; the digest remains mandatory.
    #[serde(default)]
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueRegion {
    pub start_frame: u64,
    pub end_frame: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRun {
    pub binary_commit: String,
    pub parameter_json: serde_json::Value,
    pub device_and_room: String,
    pub listener_count: u32,
    pub qa_output_sha256: String,
    pub manifest_sha256: String,
    pub compared_against_bypass: bool,
    pub compared_against_previous_release: bool,
    pub ratings: Vec<ListeningRating>,
    pub detector_results: Vec<DetectorResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListeningRating {
    pub fixture_id: String,
    pub room_preset: String,
    pub layout: String,
    pub envelopment: f32,
    pub timbral_neutrality: f32,
    pub dialogue_clarity: f32,
    pub pumping: f32,
    pub bass_localization: f32,
    pub preference: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectorResult {
    pub fixture_id: String,
    pub true_positive: u64,
    pub false_positive: u64,
    pub false_negative: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceThresholds {
    pub min_listener_count: u32,
    pub min_ratings_per_condition: usize,
    pub min_detector_precision: f32,
    pub min_detector_recall: f32,
    pub min_mean_preference: f32,
    pub max_mean_pumping: f32,
}

impl Default for AcceptanceThresholds {
    fn default() -> Self {
        Self {
            min_listener_count: 8,
            min_ratings_per_condition: 8,
            min_detector_precision: 0.8,
            min_detector_recall: 0.8,
            min_mean_preference: 4.0,
            max_mean_pumping: 3.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceReport {
    pub schema_version: u32,
    pub accepted: bool,
    pub deterministic_evidence: bool,
    pub external_evidence: bool,
    pub manifest_sha256: Option<String>,
    pub run_binary_commit: Option<String>,
    pub failures: Vec<String>,
    pub thresholds: AcceptanceThresholds,
}

pub fn load_manifest(path: impl AsRef<Path>) -> Result<ValidationManifest, String> {
    let file = File::open(path.as_ref())
        .map_err(|error| format!("cannot open manifest {}: {error}", path.as_ref().display()))?;
    serde_json::from_reader(BufReader::new(file))
        .map_err(|error| format!("cannot parse manifest {}: {error}", path.as_ref().display()))
}

pub fn load_run(path: impl AsRef<Path>) -> Result<ValidationRun, String> {
    let file = File::open(path.as_ref())
        .map_err(|error| format!("cannot open run {}: {error}", path.as_ref().display()))?;
    serde_json::from_reader(BufReader::new(file))
        .map_err(|error| format!("cannot parse run {}: {error}", path.as_ref().display()))
}

pub fn validate_manifest(
    manifest: &ValidationManifest,
    fixture_root: Option<&Path>,
) -> Result<String, Vec<String>> {
    let mut failures = Vec::new();
    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        failures.push(format!(
            "unsupported manifest schema version {}; expected {}",
            manifest.schema_version, MANIFEST_SCHEMA_VERSION
        ));
    }
    let mut ids = HashSet::new();
    let mut labels = HashSet::new();
    for fixture in &manifest.fixtures {
        if fixture.fixture_id.trim().is_empty() || !ids.insert(&fixture.fixture_id) {
            failures.push(format!(
                "fixture_id is empty or duplicated: {:?}",
                fixture.fixture_id
            ));
        }
        if fixture.label.trim().is_empty() {
            failures.push(format!("{} has an empty label", fixture.fixture_id));
        } else {
            labels.insert(fixture.label.as_str());
        }
        if fixture.license_or_source.trim().is_empty() {
            failures.push(format!("{} has no license_or_source", fixture.fixture_id));
        }
        if !is_sha256(&fixture.sha256) {
            failures.push(format!("{} has an invalid sha256", fixture.fixture_id));
        }
        if !REQUIRED_LAYOUTS.contains(&fixture.layout.as_str()) {
            failures.push(format!(
                "{} uses unsupported validation layout {:?}; expected 5.1 or 9.1.6",
                fixture.fixture_id, fixture.layout
            ));
        }
        if fixture.sample_rate == 0 {
            failures.push(format!("{} has a zero sample rate", fixture.fixture_id));
        }
        let mut previous_end = 0;
        for region in &fixture.expected_dialogue_regions {
            if region.start_frame >= region.end_frame || region.start_frame < previous_end {
                failures.push(format!(
                    "{} has overlapping or empty dialogue region {}..{}",
                    fixture.fixture_id, region.start_frame, region.end_frame
                ));
            }
            previous_end = region.end_frame;
        }
        if let Some(path) = &fixture.path {
            let resolved = fixture_root.map_or_else(|| path.clone(), |root| root.join(path));
            match sha256_file(&resolved) {
                Ok(actual) if actual == fixture.sha256.to_ascii_lowercase() => {}
                Ok(actual) => failures.push(format!(
                    "{} hash mismatch: manifest {}, actual {}",
                    fixture.fixture_id, fixture.sha256, actual
                )),
                Err(error) => failures.push(format!(
                    "{} fixture is unavailable at {}: {error}",
                    fixture.fixture_id,
                    resolved.display()
                )),
            }
        }
    }
    for required in REQUIRED_FIXTURE_LABELS {
        if !labels.contains(required) {
            failures.push(format!("required fixture class is missing: {required}"));
        }
    }
    for layout in REQUIRED_LAYOUTS {
        if !manifest
            .fixtures
            .iter()
            .any(|fixture| fixture.layout == *layout)
        {
            failures.push(format!("no fixture covers required layout {layout}"));
        }
    }
    if manifest.fixtures.is_empty() {
        failures.push("manifest contains no fixtures".into());
    }
    if failures.is_empty() {
        Ok(manifest_digest(manifest))
    } else {
        Err(failures)
    }
}

pub fn evaluate(
    manifest: &ValidationManifest,
    run: Option<&ValidationRun>,
    thresholds: AcceptanceThresholds,
    deterministic_evidence: bool,
    fixture_root: Option<&Path>,
) -> AcceptanceReport {
    let manifest_hash = match validate_manifest(manifest, fixture_root) {
        Ok(hash) => hash,
        Err(failures) => {
            return AcceptanceReport {
                schema_version: MANIFEST_SCHEMA_VERSION,
                accepted: false,
                deterministic_evidence,
                external_evidence: false,
                manifest_sha256: None,
                run_binary_commit: None,
                failures,
                thresholds,
            };
        }
    };
    let Some(run) = run else {
        return AcceptanceReport {
            schema_version: MANIFEST_SCHEMA_VERSION,
            accepted: false,
            deterministic_evidence,
            external_evidence: false,
            manifest_sha256: Some(manifest_hash),
            run_binary_commit: None,
            failures: vec![
                "external run artifact is missing; synthetic QA cannot establish listening acceptance"
                    .into(),
            ],
            thresholds,
        };
    };
    let mut failures = Vec::new();
    if run.manifest_sha256 != manifest_hash {
        failures.push(format!(
            "run references manifest {}, expected {}",
            run.manifest_sha256, manifest_hash
        ));
    }
    if run.binary_commit.trim().is_empty() {
        failures.push("binary_commit is missing".into());
    }
    if !is_sha256(&run.qa_output_sha256) {
        failures.push("qa_output_sha256 is invalid".into());
    }
    if !run.compared_against_bypass || !run.compared_against_previous_release {
        failures.push("run must compare against both bypass and previous release".into());
    }
    if run.listener_count < thresholds.min_listener_count {
        failures.push(format!(
            "listener count {} is below required {}",
            run.listener_count, thresholds.min_listener_count
        ));
    }
    let required_conditions = manifest.fixtures.len() * REQUIRED_LAYOUTS.len();
    if run.ratings.len() < required_conditions * thresholds.min_ratings_per_condition {
        failures.push(format!(
            "ratings {} are below required {} fixture/layout ratings",
            run.ratings.len(),
            required_conditions * thresholds.min_ratings_per_condition
        ));
    }
    let detector_totals = run.detector_results.iter().fold((0, 0, 0), |sum, result| {
        (
            sum.0 + result.true_positive,
            sum.1 + result.false_positive,
            sum.2 + result.false_negative,
        )
    });
    let precision =
        detector_totals.0 as f32 / (detector_totals.0 + detector_totals.1).max(1) as f32;
    let recall = detector_totals.0 as f32 / (detector_totals.0 + detector_totals.2).max(1) as f32;
    if precision < thresholds.min_detector_precision {
        failures.push(format!(
            "detector precision {precision:.3} is below threshold"
        ));
    }
    if recall < thresholds.min_detector_recall {
        failures.push(format!("detector recall {recall:.3} is below threshold"));
    }
    if run.ratings.is_empty() {
        failures.push("no listening ratings were supplied".into());
    } else {
        let mean_preference = run
            .ratings
            .iter()
            .map(|rating| rating.preference)
            .sum::<f32>()
            / run.ratings.len() as f32;
        let mean_pumping =
            run.ratings.iter().map(|rating| rating.pumping).sum::<f32>() / run.ratings.len() as f32;
        if mean_preference < thresholds.min_mean_preference {
            failures.push(format!(
                "mean preference {mean_preference:.3} is below threshold"
            ));
        }
        if mean_pumping > thresholds.max_mean_pumping {
            failures.push(format!("mean pumping {mean_pumping:.3} exceeds threshold"));
        }
    }
    AcceptanceReport {
        schema_version: MANIFEST_SCHEMA_VERSION,
        accepted: failures.is_empty(),
        deterministic_evidence,
        external_evidence: true,
        manifest_sha256: Some(manifest_hash),
        run_binary_commit: Some(run.binary_commit.clone()),
        failures,
        thresholds,
    }
}

pub fn manifest_digest(manifest: &ValidationManifest) -> String {
    let bytes = serde_json::to_vec(manifest).expect("validation manifest is serializable");
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(label: &str, layout: &str) -> FixtureMetadata {
        FixtureMetadata {
            fixture_id: format!("{label}-{layout}"),
            label: label.into(),
            license_or_source: "test fixture source".into(),
            sha256: "00".repeat(32),
            layout: layout.into(),
            sample_rate: 48_000,
            expected_dialogue_regions: Vec::new(),
            path: None,
        }
    }

    fn manifest() -> ValidationManifest {
        ValidationManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            fixtures: REQUIRED_FIXTURE_LABELS
                .iter()
                .enumerate()
                .map(|(index, label)| fixture(label, REQUIRED_LAYOUTS[index % 2]))
                .collect(),
        }
    }

    #[test]
    fn manifest_requires_every_external_fixture_class() {
        let mut value = manifest();
        value.fixtures.pop();
        let errors = validate_manifest(&value, None).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.contains("transient_full_scale"))
        );
    }

    #[test]
    fn missing_external_run_can_never_be_accepted() {
        let report = evaluate(
            &manifest(),
            None,
            AcceptanceThresholds::default(),
            true,
            None,
        );
        assert!(!report.accepted);
        assert!(!report.external_evidence);
        assert!(
            report
                .failures
                .iter()
                .any(|failure| failure.contains("missing"))
        );
    }

    #[test]
    fn manifest_digest_is_stable() {
        assert_eq!(manifest_digest(&manifest()), manifest_digest(&manifest()));
        assert!(is_sha256(&"ab".repeat(32)));
        assert!(!is_sha256("not-a-hash"));
    }
}

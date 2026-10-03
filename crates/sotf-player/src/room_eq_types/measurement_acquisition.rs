//! Binds imported acquisition evidence to ordered, unchanged response data.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::recording_types::RecordingResult;

/// Source acquisition evidence retained across frontend persistence and export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementAcquisition {
    provenance: autoeq::MeasurementProvenance,
    response_sha256: Vec<String>,
}

impl MeasurementAcquisition {
    /// Binds declared evidence to the responses loaded in acquisition order.
    pub(crate) fn new<'a>(
        provenance: autoeq::MeasurementProvenance,
        responses: impl IntoIterator<Item = &'a RecordingResult>,
    ) -> Self {
        Self {
            provenance,
            response_sha256: responses.into_iter().map(response_digest).collect(),
        }
    }

    /// Returns evidence only while the bound response data and order remain unchanged.
    ///
    /// Changed data retains raw capture facts with quality acceptance revoked.
    /// This detects frontend edits; it does not authenticate externally supplied JSON.
    pub(crate) fn for_responses<'a>(
        &self,
        responses: impl IntoIterator<Item = &'a RecordingResult>,
    ) -> autoeq::MeasurementProvenance {
        let mut valid = true;
        let digests: Vec<_> = responses
            .into_iter()
            .map(|response| {
                let count = response.frequencies.len();
                valid &= count > 0
                    && response.magnitude_db.len() == count
                    && (response.phase_deg.is_empty() || response.phase_deg.len() == count)
                    && response
                        .frequencies
                        .iter()
                        .all(|value| value.is_finite() && *value > 0.0)
                    && response
                        .frequencies
                        .windows(2)
                        .all(|pair| pair[0] < pair[1])
                    && response
                        .magnitude_db
                        .iter()
                        .chain(&response.phase_deg)
                        .all(|value| value.is_finite());
                response_digest(response)
            })
            .collect();
        if valid && !digests.is_empty() && digests == self.response_sha256 {
            return self.provenance.clone();
        }
        let mut capture = self.provenance.capture.clone();
        if let Some(capture) = capture.as_mut() {
            for take in &mut capture.takes {
                take.quality_passed = false;
            }
        }
        autoeq::MeasurementProvenance {
            capture,
            ..Default::default()
        }
    }
}

fn response_digest(response: &RecordingResult) -> String {
    let mut digest = Sha256::new();
    digest.update(b"sotf-room-eq-response-v1");
    digest.update([u8::from(response.sample_rate_hz.is_some())]);
    digest.update(response.sample_rate_hz.unwrap_or_default().to_le_bytes());
    for values in [
        &response.frequencies,
        &response.magnitude_db,
        &response.phase_deg,
    ] {
        digest.update((values.len() as u64).to_le_bytes());
        for value in values {
            digest.update(value.to_bits().to_le_bytes());
        }
    }
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

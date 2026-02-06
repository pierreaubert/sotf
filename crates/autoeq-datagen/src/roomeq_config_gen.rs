//! Generate autoeq RoomConfig JSON files from scenarios
//!
//! Creates the configuration JSON that the `roomeq` optimizer reads.

use anyhow::Result;
use autoeq::roomeq::{MultiSubGroup, OptimizerConfig, RoomConfig, SpeakerConfig};
use autoeq::{MeasurementRef, MeasurementSource};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::scenarios::Scenario;

/// Generate a roomeq JSON config for a scenario.
///
/// The config references CSV files by filename only (config.json sits alongside CSVs).
/// `_csv_dir` is accepted for API compatibility but paths are always relative filenames.
pub fn generate_config(scenario: &Scenario, _csv_dir: &Path) -> Result<RoomConfig> {
    let source_names = &scenario.source_names;

    let mut speakers: HashMap<String, SpeakerConfig> = HashMap::new();

    // Classify sources
    let mut main_sources: Vec<&str> = Vec::new();
    let mut sub_sources: Vec<&str> = Vec::new();

    for name in source_names {
        if name.starts_with("sub") {
            sub_sources.push(name);
        } else {
            main_sources.push(name);
        }
    }

    // Add main speakers (left, right, etc.) as Single configs
    for &name in &main_sources {
        let csv_path = PathBuf::from(format!("{name}_lp0.csv"));
        speakers.insert(
            name.to_string(),
            SpeakerConfig::Single(MeasurementSource::Single(MeasurementRef::Path(csv_path))),
        );
    }

    // Add subwoofers
    match sub_sources.len() {
        0 => {} // No subs
        1 => {
            // Single sub: SpeakerConfig::Single
            let name = sub_sources[0];
            let csv_path = PathBuf::from(format!("{name}_lp0.csv"));
            speakers.insert(
                "lfe".to_string(),
                SpeakerConfig::Single(MeasurementSource::Single(MeasurementRef::Path(csv_path))),
            );
        }
        _ => {
            // Multiple subs: SpeakerConfig::MultiSub
            let subwoofers: Vec<MeasurementSource> = sub_sources
                .iter()
                .map(|name| {
                    let csv_path = PathBuf::from(format!("{name}_lp0.csv"));
                    MeasurementSource::Single(MeasurementRef::Path(csv_path))
                })
                .collect();
            speakers.insert(
                "lfe".to_string(),
                SpeakerConfig::MultiSub(MultiSubGroup {
                    name: "subs".to_string(),
                    subwoofers,
                }),
            );
        }
    }

    let optimizer = OptimizerConfig {
        algorithm: "autoeq:de".to_string(),
        num_filters: 7,
        max_freq: 500.0,
        loss_type: "flat".to_string(),
        seed: Some(42),
        max_iter: 5000,
        refine: true,
        asymmetric_loss: true,
        ..OptimizerConfig::default()
    };

    let config = RoomConfig {
        version: "1.1.0".to_string(),
        speakers,
        crossovers: None,
        target_curve: None,
        group_delay: None,
        optimizer,
        recording_config: None,
    };

    Ok(config)
}

/// Write a roomeq config JSON file
pub fn write_config(config: &RoomConfig, output_path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(output_path, json)?;
    Ok(())
}

//! Generate autoeq RoomConfig JSON files from scenarios
//!
//! Creates the configuration JSON that the `roomeq` optimizer reads.

use anyhow::Result;
use autoeq::roomeq::{
    BassManagementConfig, MultiSubGroup, OptimizerConfig, ProcessingMode, RoomConfig,
    SpeakerConfig, SubwooferStrategy,
};
use autoeq::{MeasurementRef, MeasurementSingle, MeasurementSource};
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

    let mut main_sources = Vec::new();
    let mut sub_sources = Vec::new();

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
            SpeakerConfig::Single(MeasurementSource::Single(MeasurementSingle {
                measurement: MeasurementRef::Path(csv_path),
                speaker_name: None,
            })),
        );
    }

    // Configure Bass Management
    let bass_management = match sub_sources.len() {
        0 => None,
        1 => {
            // Single sub: SpeakerConfig::Single
            let name = sub_sources[0];
            let csv_path = PathBuf::from(format!("{name}_lp0.csv"));
            speakers.insert(
                "lfe".to_string(),
                SpeakerConfig::Single(MeasurementSource::Single(MeasurementSingle {
                    measurement: MeasurementRef::Path(csv_path),
                    speaker_name: None,
                })),
            );
            Some(BassManagementConfig {
                strategy: SubwooferStrategy::Single,
                ..BassManagementConfig::default()
            })
        }
        _ => {
            // Multiple subs: SpeakerConfig::MultiSub
            let subwoofers: Vec<MeasurementSource> = sub_sources
                .iter()
                .map(|name| {
                    let csv_path = PathBuf::from(format!("{name}_lp0.csv"));
                    MeasurementSource::Single(MeasurementSingle {
                        measurement: MeasurementRef::Path(csv_path),
                        speaker_name: None,
                    })
                })
                .collect();
            speakers.insert(
                "lfe".to_string(),
                SpeakerConfig::MultiSub(MultiSubGroup {
                    name: "subs".to_string(),
                    speaker_name: None,
                    subwoofers,
                }),
            );
            Some(BassManagementConfig {
                strategy: SubwooferStrategy::Mso, // Default to MSO for multi-sub
                ..BassManagementConfig::default()
            })
        }
    };

    let optimizer = OptimizerConfig {
        algorithm: "autoeq:de".to_string(),
        num_filters: 7,
        max_freq: 500.0,
        loss_type: "flat".to_string(),
        seed: Some(42),
        max_iter: 5000,
        refine: true,
        asymmetric_loss: true,
        processing_mode: ProcessingMode::LowLatency, // Default to Mode A
        ..OptimizerConfig::default()
    };

    let config = RoomConfig {
        version: "1.2.0".to_string(),
        system: None,
        speakers,
        crossovers: None,
        target_curve: None,
        group_delay: None,
        bass_management,
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
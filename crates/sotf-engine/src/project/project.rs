// ============================================================================
// Project — Unified project file format for DAW sessions
// ============================================================================

use crate::engine::{PluginConfig, PluginGraphConfig};
use serde::{Deserialize, Serialize};
use sotf_plugins::automation::AutomationCurve;
use std::path::Path;

/// Current project file format version.
pub const PROJECT_VERSION: u32 = 1;

/// A complete DAW project — the root serialization type.
///
/// Saved as a JSON file with `.sotf` extension. All audio file paths
/// are stored relative to the project file for portability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// File format version (for forward migration)
    pub version: u32,
    /// Project name
    pub name: String,
    /// Global sample rate
    pub sample_rate: u32,
    /// Tempo in BPM
    pub tempo_bpm: f64,
    /// Audio tracks
    pub tracks: Vec<TrackConfig>,
    /// MIDI tracks
    pub midi_tracks: Vec<MidiTrackConfig>,
    /// Master bus plugin chain
    pub master_plugins: Vec<PluginConfig>,
    /// Master bus graph configuration (optional, for advanced routing)
    pub master_graph: Option<PluginGraphConfig>,
    /// Processing frame size
    pub frame_size: usize,
    /// Output channel count
    pub output_channels: usize,
    /// Loop region (start, end) in samples. None = no loop.
    pub loop_range: Option<(u64, u64)>,
}

impl Project {
    /// Create a new empty project.
    pub fn new(name: impl Into<String>, sample_rate: u32) -> Self {
        Self {
            version: PROJECT_VERSION,
            name: name.into(),
            sample_rate,
            tempo_bpm: 120.0,
            tracks: Vec::new(),
            midi_tracks: Vec::new(),
            master_plugins: Vec::new(),
            master_graph: None,
            frame_size: 1024,
            output_channels: 2,
            loop_range: None,
        }
    }

    /// Save the project to a JSON file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize project: {e}"))?;
        std::fs::write(path.as_ref(), json)
            .map_err(|e| format!("Failed to write project file: {e}"))?;
        Ok(())
    }

    /// Load a project from a JSON file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let json = std::fs::read_to_string(path.as_ref())
            .map_err(|e| format!("Failed to read project file: {e}"))?;
        let mut project: Self =
            serde_json::from_str(&json).map_err(|e| format!("Failed to parse project: {e}"))?;
        project.migrate()?;
        Ok(project)
    }

    /// Apply version migrations.
    fn migrate(&mut self) -> Result<(), String> {
        // Future: add migration logic as version increments
        // if self.version < 2 { ... self.version = 2; }
        Ok(())
    }

    /// Resolve relative audio paths against a base directory.
    pub fn resolve_paths(&mut self, base_dir: &Path) {
        for track in &mut self.tracks {
            for region in &mut track.regions {
                if !Path::new(&region.source_path).is_absolute() {
                    region.source_path =
                        base_dir.join(&region.source_path).to_string_lossy().into();
                }
            }
        }
    }

    /// Make all audio paths relative to a base directory.
    pub fn relativize_paths(&mut self, base_dir: &Path) {
        for track in &mut self.tracks {
            for region in &mut track.regions {
                let abs = Path::new(&region.source_path);
                if abs.is_absolute()
                    && let Ok(rel) = abs.strip_prefix(base_dir)
                {
                    region.source_path = rel.to_string_lossy().into();
                }
            }
        }
    }
}

/// Track type discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackType {
    Audio,
    Bus,
}

/// Serializable audio track configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackConfig {
    pub name: String,
    pub track_type: TrackType,
    pub regions: Vec<RegionConfig>,
    pub plugins: Vec<PluginConfig>,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
    pub channels: usize,
    /// Per-parameter automation
    pub automation: Vec<AutomationConfig>,
}

impl TrackConfig {
    pub fn new(name: impl Into<String>, channels: usize) -> Self {
        Self {
            name: name.into(),
            track_type: TrackType::Audio,
            regions: Vec::new(),
            plugins: Vec::new(),
            volume: 1.0,
            pan: 0.0,
            muted: false,
            solo: false,
            channels,
            automation: Vec::new(),
        }
    }
}

/// Serializable region (clip placement) configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionConfig {
    /// Path to audio file (relative to project file)
    pub source_path: String,
    /// Position on the timeline in samples
    pub position_samples: u64,
    /// Duration in samples
    pub duration_samples: u64,
    /// Offset within the source file in samples
    pub source_offset: u64,
    /// Per-clip gain in dB
    pub gain_db: f32,
    /// Fade-in duration in samples
    pub fade_in_samples: u64,
    /// Fade-out duration in samples
    pub fade_out_samples: u64,
    /// Fade curve type
    pub fade_curve: String,
    /// Time stretch ratio (1.0 = normal)
    pub time_stretch_ratio: f64,
    /// Play in reverse
    pub reverse: bool,
}

/// Serializable MIDI track configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiTrackConfig {
    pub name: String,
    pub regions: Vec<MidiRegionConfig>,
    /// Instrument plugin configuration
    pub instrument: PluginConfig,
    /// Effect chain plugins
    pub plugins: Vec<PluginConfig>,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
}

/// Serializable MIDI region configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiRegionConfig {
    /// Position on the timeline in samples
    pub position_samples: u64,
    /// Duration in samples
    pub duration_samples: u64,
    /// MIDI events as JSON (serialized MidiClip)
    pub events_json: String,
}

/// Serializable automation configuration for a parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationConfig {
    /// Plugin index in the track's plugin chain
    pub plugin_index: usize,
    /// Parameter ID
    pub param_id: String,
    /// Automation curve
    pub curve: AutomationCurve,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_new() {
        let project = Project::new("Test Project", 48000);
        assert_eq!(project.version, PROJECT_VERSION);
        assert_eq!(project.name, "Test Project");
        assert_eq!(project.sample_rate, 48000);
        assert_eq!(project.tempo_bpm, 120.0);
        assert!(project.tracks.is_empty());
    }

    #[test]
    fn test_project_save_load() {
        let dir = std::env::temp_dir().join("sotf_test_project");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.sotf");

        let mut project = Project::new("My Song", 44100);
        project.tempo_bpm = 140.0;
        project.output_channels = 4;

        let mut track = TrackConfig::new("Guitar", 2);
        track.regions.push(RegionConfig {
            source_path: "audio/guitar.wav".to_string(),
            position_samples: 0,
            duration_samples: 96000,
            source_offset: 0,
            gain_db: -3.0,
            fade_in_samples: 480,
            fade_out_samples: 960,
            fade_curve: "linear".to_string(),
            time_stretch_ratio: 1.0,
            reverse: false,
        });
        project.tracks.push(track);

        project.save(&path).unwrap();
        assert!(path.exists());

        let loaded = Project::load(&path).unwrap();
        assert_eq!(loaded.name, "My Song");
        assert_eq!(loaded.sample_rate, 44100);
        assert_eq!(loaded.tempo_bpm, 140.0);
        assert_eq!(loaded.tracks.len(), 1);
        assert_eq!(loaded.tracks[0].name, "Guitar");
        assert_eq!(loaded.tracks[0].regions.len(), 1);
        assert_eq!(loaded.tracks[0].regions[0].gain_db, -3.0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_project_relativize_paths() {
        let mut project = Project::new("Test", 48000);
        let mut track = TrackConfig::new("T1", 2);
        track.regions.push(RegionConfig {
            source_path: "/home/user/music/project/audio/guitar.wav".to_string(),
            position_samples: 0,
            duration_samples: 48000,
            source_offset: 0,
            gain_db: 0.0,
            fade_in_samples: 0,
            fade_out_samples: 0,
            fade_curve: "linear".to_string(),
            time_stretch_ratio: 1.0,
            reverse: false,
        });
        project.tracks.push(track);

        project.relativize_paths(Path::new("/home/user/music/project"));
        assert_eq!(project.tracks[0].regions[0].source_path, "audio/guitar.wav");
    }

    #[test]
    fn test_project_resolve_paths() {
        let mut project = Project::new("Test", 48000);
        let mut track = TrackConfig::new("T1", 2);
        track.regions.push(RegionConfig {
            source_path: "audio/guitar.wav".to_string(),
            position_samples: 0,
            duration_samples: 48000,
            source_offset: 0,
            gain_db: 0.0,
            fade_in_samples: 0,
            fade_out_samples: 0,
            fade_curve: "linear".to_string(),
            time_stretch_ratio: 1.0,
            reverse: false,
        });
        project.tracks.push(track);

        project.resolve_paths(Path::new("/home/user/music/project"));
        assert_eq!(
            project.tracks[0].regions[0].source_path,
            "/home/user/music/project/audio/guitar.wav"
        );
    }

    #[test]
    fn test_automation_persistence_roundtrip() {
        let dir = std::env::temp_dir().join("sotf_test_automation_persist");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.sotf");

        let mut project = Project::new("AutoTest", 48000);
        let mut track = TrackConfig::new("Synth", 2);
        track.automation.push(AutomationConfig {
            plugin_index: 0,
            param_id: "gain".to_string(),
            curve: AutomationCurve::Linear {
                values: vec![0.0, 0.5, 1.0],
            },
        });
        track.automation.push(AutomationConfig {
            plugin_index: 1,
            param_id: "frequency".to_string(),
            curve: AutomationCurve::Step {
                values: vec![440.0, 880.0],
                samples_per_step: 48000,
            },
        });
        project.tracks.push(track);

        // Save
        project.save(&path).unwrap();

        // Load
        let loaded = Project::load(&path).unwrap();
        assert_eq!(loaded.tracks[0].automation.len(), 2);

        // Verify linear curve roundtrip
        let auto0 = &loaded.tracks[0].automation[0];
        assert_eq!(auto0.param_id, "gain");
        match &auto0.curve {
            AutomationCurve::Linear { values } => {
                assert_eq!(values, &[0.0, 0.5, 1.0]);
            }
            other => panic!("Expected Linear curve, got {other:?}"),
        }

        // Verify step curve roundtrip
        let auto1 = &loaded.tracks[0].automation[1];
        assert_eq!(auto1.param_id, "frequency");
        match &auto1.curve {
            AutomationCurve::Step {
                values,
                samples_per_step,
            } => {
                assert_eq!(values, &[440.0, 880.0]);
                assert_eq!(*samples_per_step, 48000);
            }
            other => panic!("Expected Step curve, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}

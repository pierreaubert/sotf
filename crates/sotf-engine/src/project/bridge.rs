// ============================================================================
// Project ↔ Timeline Bridge — Convert between serializable and runtime forms
// ============================================================================

use crate::decoder::source::AudioSource;
use crate::engine::{PluginConfig, build_plugin_host};
use crate::project::project::{Project, RegionConfig, TrackConfig};
use crate::timeline::clip::{Clip, FadeCurve, Region};
use crate::timeline::timeline::Timeline;
use crate::timeline::track::Track;
use std::path::Path;

impl Project {
    /// Build a Timeline from this project configuration.
    ///
    /// Audio file paths in regions are resolved relative to `base_dir`
    /// (typically the directory containing the .sotf project file).
    pub fn to_timeline(&self, base_dir: &Path) -> Result<Timeline, String> {
        let mut timeline =
            Timeline::new(self.output_channels, self.sample_rate, self.frame_size);
        timeline.transport.tempo_bpm = self.tempo_bpm;
        timeline.transport.set_loop(self.loop_range);

        for track_config in &self.tracks {
            let mut track =
                Track::new(&track_config.name, track_config.channels, self.sample_rate);
            track.volume = track_config.volume;
            track.pan = track_config.pan;
            track.muted = track_config.muted;
            track.solo = track_config.solo;

            for region_config in &track_config.regions {
                let region = region_config_to_region(region_config, base_dir);
                track.add_region(region);
            }

            // Reconstruct track plugin chain from saved configs
            if !track_config.plugins.is_empty() {
                match build_plugin_host(
                    &track_config.plugins,
                    self.sample_rate,
                    track_config.channels,
                ) {
                    Ok((host, _warnings)) => track.chain = host,
                    Err(e) => {
                        log::warn!(
                            "Failed to rebuild plugin chain for track '{}': {e}",
                            track_config.name
                        );
                    }
                }
            }

            timeline.add_track(track);
        }

        // Reconstruct master plugin chain
        if !self.master_plugins.is_empty() {
            match build_plugin_host(
                &self.master_plugins,
                self.sample_rate,
                self.output_channels,
            ) {
                Ok((host, _warnings)) => timeline.master_chain = host,
                Err(e) => {
                    log::warn!("Failed to rebuild master plugin chain: {e}");
                }
            }
        }

        timeline.build()?;
        Ok(timeline)
    }

    /// Create a Project from a Timeline's current state.
    ///
    /// Audio file paths are made relative to `base_dir`.
    pub fn from_timeline(
        timeline: &Timeline,
        name: impl Into<String>,
        base_dir: &Path,
    ) -> Self {
        let mut project = Project::new(name, timeline.transport.sample_rate);
        project.tempo_bpm = timeline.transport.tempo_bpm;
        project.output_channels = timeline.output_channels;
        project.frame_size = timeline.frame_size;
        project.loop_range = timeline.transport.loop_range;

        for track in &timeline.tracks {
            let mut track_config = TrackConfig::new(&track.name, track.channels);
            track_config.volume = track.volume;
            track_config.pan = track.pan;
            track_config.muted = track.muted;
            track_config.solo = track.solo;

            for region in &track.regions {
                track_config
                    .regions
                    .push(region_to_region_config(region, base_dir));
            }

            project.tracks.push(track_config);
        }

        // TODO: Serialize MIDI tracks
        // TODO: Serialize plugin chains

        project
    }
}

fn region_config_to_region(config: &RegionConfig, base_dir: &Path) -> Region {
    let source_path = if Path::new(&config.source_path).is_absolute() {
        config.source_path.clone()
    } else {
        base_dir
            .join(&config.source_path)
            .to_string_lossy()
            .into()
    };

    let fade_curve = match config.fade_curve.as_str() {
        "equal_power" => FadeCurve::EqualPower,
        "s_curve" | "scurve" => FadeCurve::SCurve,
        _ => FadeCurve::Linear,
    };

    let mut clip = Clip::from_file(source_path, config.duration_samples);
    clip.source_offset_samples = config.source_offset;
    clip.gain_db = config.gain_db;
    clip.fade_in_samples = config.fade_in_samples;
    clip.fade_out_samples = config.fade_out_samples;
    clip.fade_curve = fade_curve;
    clip.time_stretch_ratio = config.time_stretch_ratio;
    clip.reverse = config.reverse;

    Region::new(clip, config.position_samples)
}

fn region_to_region_config(region: &Region, base_dir: &Path) -> RegionConfig {
    let source_path = match &region.clip.source {
        AudioSource::File(p) => {
            if let Ok(rel) = p.strip_prefix(base_dir) {
                rel.to_string_lossy().into()
            } else {
                p.to_string_lossy().into()
            }
        }
        other => format!("{other:?}"),
    };

    let fade_curve = match region.clip.fade_curve {
        FadeCurve::Linear => "linear",
        FadeCurve::EqualPower => "equal_power",
        FadeCurve::SCurve => "s_curve",
    };

    RegionConfig {
        source_path,
        position_samples: region.position_samples,
        duration_samples: region.clip.duration_samples,
        source_offset: region.clip.source_offset_samples,
        gain_db: region.clip.gain_db,
        fade_in_samples: region.clip.fade_in_samples,
        fade_out_samples: region.clip.fade_out_samples,
        fade_curve: fade_curve.to_string(),
        time_stretch_ratio: region.clip.time_stretch_ratio,
        reverse: region.clip.reverse,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::project::Project;

    fn create_test_wav(path: &Path) {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for _ in 0..4800 {
            writer.write_sample(0.5f32).unwrap();
            writer.write_sample(0.5f32).unwrap();
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn test_project_to_timeline_roundtrip() {
        let dir = std::env::temp_dir().join("sotf_test_bridge");
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("audio.wav");
        create_test_wav(&wav);

        // Create a project with one track
        let mut project = Project::new("Test Song", 48000);
        project.output_channels = 2;
        let mut track = TrackConfig::new("Guitar", 2);
        track.volume = 0.8;
        track.regions.push(RegionConfig {
            source_path: "audio.wav".to_string(),
            position_samples: 0,
            duration_samples: 4800,
            source_offset: 0,
            gain_db: -3.0,
            fade_in_samples: 480,
            fade_out_samples: 960,
            fade_curve: "linear".to_string(),
            time_stretch_ratio: 1.0,
            reverse: false,
        });
        project.tracks.push(track);

        // Convert to Timeline
        let timeline = project.to_timeline(&dir).unwrap();
        assert_eq!(timeline.tracks.len(), 1);
        assert_eq!(timeline.tracks[0].name, "Guitar");
        assert!((timeline.tracks[0].volume - 0.8).abs() < 1e-6);
        assert_eq!(timeline.tracks[0].regions.len(), 1);
        assert_eq!(timeline.tracks[0].regions[0].clip.fade_in_samples, 480);

        // Convert back to Project
        let project2 = Project::from_timeline(&timeline, "Test Song", &dir);
        assert_eq!(project2.tracks.len(), 1);
        assert_eq!(project2.tracks[0].name, "Guitar");
        assert_eq!(project2.tracks[0].regions[0].source_path, "audio.wav");
        assert_eq!(project2.tracks[0].regions[0].gain_db, -3.0);
        assert_eq!(project2.tracks[0].regions[0].fade_in_samples, 480);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_project_save_load_timeline_roundtrip() {
        let dir = std::env::temp_dir().join("sotf_test_bridge_save");
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("track.wav");
        create_test_wav(&wav);
        let project_path = dir.join("song.sotf");

        // Build timeline
        let mut timeline = Timeline::new(2, 48000, 1024);
        let mut track = Track::new("Bass", 2, 48000);
        track.volume = 0.6;
        let clip = Clip::from_file(&wav, 4800);
        track.add_region(Region::new(clip, 9600));
        timeline.add_track(track);
        timeline.transport.tempo_bpm = 140.0;

        // Save via Project
        let project = Project::from_timeline(&timeline, "My Song", &dir);
        project.save(&project_path).unwrap();

        // Load and reconstruct
        let loaded = Project::load(&project_path).unwrap();
        let timeline2 = loaded.to_timeline(&dir).unwrap();

        assert_eq!(timeline2.tracks.len(), 1);
        assert_eq!(timeline2.tracks[0].name, "Bass");
        assert!((timeline2.tracks[0].volume - 0.6).abs() < 1e-6);
        assert_eq!(timeline2.tracks[0].regions[0].position_samples, 9600);
        assert!((timeline2.transport.tempo_bpm - 140.0).abs() < 1e-6);

        let _ = std::fs::remove_dir_all(&dir);
    }
}

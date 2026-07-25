use sotf_audio::decoder::{AudioSource, AudioSpec};
use sotf_audio::engine::{AudioEngineState, OutputAccessStatus, PluginConfig, StreamMetadata};
use sotf_audio::manager::AudioFileInfo;
use std::sync::Arc;

/// Batched playback state to reduce mutex locking
#[derive(Debug, Clone)]
pub struct PlaybackState {
    pub position_secs: f64,
    pub is_playing: bool,
    pub sample_rate: Option<u32>,
    pub last_error: Option<String>,
    /// Set once after the engine auto-restarted from a crash. Cleared after read.
    pub engine_restarted: bool,
    /// Set when the engine crashed twice — no further auto-restart will be attempted.
    pub engine_fatal: bool,
    /// Set when the current track finished playing (end-of-stream).
    /// The UI layer should check this and auto-advance the queue.
    /// Cleared on the next `get_playback_state()` call.
    pub track_ended: bool,
    /// Set when the engine seamlessly transitioned to a new source (gapless playback).
    /// Cleared after being read.
    pub gapless_transition: Option<AudioSource>,
    /// Latest live stream metadata from ICY/content-type/bitrate updates.
    pub stream_metadata: Option<StreamMetadata>,
}

/// Saved configuration for restarting after a crash.
#[derive(Clone)]
pub(crate) struct SavedPlaybackConfig {
    pub(super) source: Arc<AudioSource>,
    pub(super) plugins: Arc<[PluginConfig]>,
    pub(super) output_channels: usize,
    pub(super) output_device: Option<Arc<str>>,
    pub(super) last_position_secs: f64,
}

/// Read-only model of the current audio signal path.
///
/// This is intended for UI surfaces that want to explain why audio is being
/// resampled or processed, and to surface engine-known health indicators such
/// as underruns or (when available) clipping.
#[derive(Debug, Clone)]
pub struct SignalPath {
    /// Source-side information (file/stream format and decoded spec).
    pub source: Option<SignalPathSource>,
    /// Plugin chain applied after decoding.
    pub plugin_chain: Vec<SignalPathPlugin>,
    /// Processing stages that transform the decoded signal before output.
    pub processing: SignalPathProcessing,
    /// Output path configuration and observed state.
    pub output: SignalPathOutput,
    /// Engine-known health indicators.
    pub health: SignalPathHealth,
}

/// Source format and decoded audio spec.
#[derive(Debug, Clone)]
pub struct SignalPathSource {
    /// User-facing format name, e.g. "FLAC" or "MP3".
    pub format: String,
    /// Sample rate reported by the decoder.
    pub sample_rate_hz: u32,
    /// Channel count reported by the decoder.
    pub channels: u16,
    /// Bit depth reported by the decoder.
    pub bits_per_sample: u16,
    /// Whether the source format is lossless.
    pub lossless: bool,
}

/// One plugin entry in the signal path.
#[derive(Debug, Clone)]
pub struct SignalPathPlugin {
    /// Plugin type identifier, e.g. "EQ" or "Compressor".
    pub plugin_type: String,
    /// Raw plugin configuration (omitted if it contains filesystem paths).
    pub parameters: Option<serde_json::Value>,
}

/// Processing stages between source and output.
#[derive(Debug, Clone)]
pub struct SignalPathProcessing {
    /// Resampling from the decoder sample rate to the output sample rate.
    /// `None` when the engine and source rates match.
    pub resampling: Option<SignalPathResampling>,
    /// Total plugin-chain latency in samples.
    pub latency_samples: usize,
    /// Whether the processing chain is currently bypassed.
    pub bypassed: bool,
}

/// Resampling information.
#[derive(Debug, Clone)]
pub struct SignalPathResampling {
    pub from_hz: u32,
    pub to_hz: u64,
}

/// Output path information.
#[derive(Debug, Clone)]
pub struct SignalPathOutput {
    /// Output device name resolved by the playback stream, if known.
    pub device: Option<String>,
    /// Sample rate the output device is running at.
    pub sample_rate_hz: u64,
    /// Number of channels delivered to the output device.
    pub channels: usize,
    /// Requested output access mode (Shared / Exclusive).
    pub access_mode: String,
    /// Whether exclusive access is actually active.
    pub exclusive_active: bool,
}

/// Engine-known health indicators.
#[derive(Debug, Clone)]
pub struct SignalPathHealth {
    /// Number of buffer underruns observed by the playback thread.
    pub underruns: u64,
    /// Number of output stream errors observed by the playback thread.
    pub stream_errors: u64,
    /// Number of processed frames dropped before reaching hardware.
    pub frames_dropped: u64,
    /// `Some(true)` when the engine has detected clipping in the current
    /// processing path. `None` means the engine does not yet expose a clipping
    /// detector for this configuration.
    pub clipping_detected: Option<bool>,
    /// Remaining headroom in dB relative to 0 dBFS, if measured.
    /// `None` when the engine does not expose a peak meter.
    pub headroom_db: Option<f32>,
}

impl SignalPath {
    /// Build a signal-path snapshot from the player's saved config, the
    /// decoder's audio info, and the live engine state.
    pub(crate) fn from_player_state(
        saved_config: Option<&super::SavedPlaybackConfig>,
        audio_info: Option<&AudioFileInfo>,
        engine_state: &AudioEngineState,
    ) -> Self {
        let source = audio_info.map(|info| SignalPathSource {
            format: info.format.to_string(),
            sample_rate_hz: info.spec.sample_rate,
            channels: info.spec.channels,
            bits_per_sample: info.spec.bits_per_sample,
            lossless: info.format.is_lossless(),
        });

        let plugin_chain: Vec<SignalPathPlugin> = saved_config
            .map(|c| c.plugins.as_ref())
            .unwrap_or_default()
            .iter()
            .map(|p| SignalPathPlugin {
                plugin_type: p.plugin_type.clone(),
                parameters: Some(p.parameters.clone()),
            })
            .collect();

        let source_rate = source.as_ref().map(|s| s.sample_rate_hz).unwrap_or(0);
        // The configured engine sample rate is the authoritative output rate.
        // The observed hardware callback rate is reported separately in health.
        let output_rate = u64::from(engine_state.sample_rate.max(1));
        let resampling = if source_rate > 0 && output_rate != u64::from(source_rate) {
            Some(SignalPathResampling {
                from_hz: source_rate,
                to_hz: output_rate,
            })
        } else {
            None
        };

        Self {
            source,
            plugin_chain,
            processing: SignalPathProcessing {
                resampling,
                latency_samples: engine_state.plugin_latency_samples,
                bypassed: engine_state.processing_bypassed,
            },
            output: SignalPathOutput {
                device: engine_state.playback_output_device.clone(),
                sample_rate_hz: output_rate,
                channels: engine_state.num_channels,
                access_mode: format!("{:?}", engine_state.output_access_mode),
                exclusive_active: matches!(
                    engine_state.output_access_status,
                    OutputAccessStatus::ExclusiveActive
                ),
            },
            health: SignalPathHealth {
                underruns: engine_state.underruns,
                stream_errors: engine_state.playback_stream_error_count,
                frames_dropped: engine_state.playback_frames_dropped,
                clipping_detected: Some(engine_state.output_clipping_detected),
                headroom_db: Some(output_headroom_db(engine_state.output_peak_linear)),
            },
        }
    }

    /// True if any stage resamples the signal before output.
    pub fn is_resampled(&self) -> bool {
        self.processing.resampling.is_some()
    }

    /// True if the engine has reported any health issues it knows about.
    pub fn has_known_issues(&self) -> bool {
        self.health.underruns > 0
            || self.health.stream_errors > 0
            || self.health.frames_dropped > 0
            || self.health.clipping_detected == Some(true)
    }
}

fn output_headroom_db(peak_linear: f32) -> f32 {
    if peak_linear.is_finite() && peak_linear > 0.0 {
        (-20.0 * peak_linear.log10()).min(120.0)
    } else {
        120.0
    }
}

/// Build a `SignalPathSource` from an `AudioSpec` and a format name.
impl SignalPathSource {
    pub fn from_spec_and_format(spec: &AudioSpec, format_name: &str, lossless: bool) -> Self {
        Self {
            format: format_name.to_string(),
            sample_rate_hz: spec.sample_rate,
            channels: spec.channels,
            bits_per_sample: spec.bits_per_sample,
            lossless,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_audio::decoder::AudioSource;
    use sotf_audio::engine::PluginConfig;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn sample_saved_config() -> super::SavedPlaybackConfig {
        super::SavedPlaybackConfig {
            source: Arc::new(AudioSource::File(PathBuf::from("/tmp/test.flac"))),
            plugins: Arc::from(vec![
                PluginConfig::new(
                    "EQ",
                    serde_json::json!({"filters": [{"filter_type": "peak", "freq": 1000.0}]}),
                ),
                PluginConfig::new("Gain", serde_json::json!({"gain_db": -6.0})),
            ]),
            output_channels: 2,
            output_device: None,
            last_position_secs: 0.0,
        }
    }

    fn sample_audio_info(sample_rate: u32) -> AudioFileInfo {
        AudioFileInfo {
            path: PathBuf::from("/tmp/test.flac"),
            source: AudioSource::File(PathBuf::from("/tmp/test.flac")),
            format: sotf_audio::decoder::AudioFormat::Flac,
            spec: AudioSpec {
                sample_rate,
                channels: 2,
                bits_per_sample: 16,
                total_frames: Some(sample_rate as u64 * 60),
            },
            duration_seconds: Some(60.0),
        }
    }

    #[test]
    fn signal_path_idle_has_no_source_and_zero_channels() {
        let engine_state = AudioEngineState::default();
        let path = SignalPath::from_player_state(None, None, &engine_state);

        assert!(path.source.is_none());
        assert!(path.plugin_chain.is_empty());
        assert!(!path.is_resampled());
        assert!(!path.has_known_issues());
    }

    #[test]
    fn signal_path_reports_plugins_and_source_format() {
        let config = sample_saved_config();
        let info = sample_audio_info(48_000);
        let engine_state = AudioEngineState::default();

        let path = SignalPath::from_player_state(Some(&config), Some(&info), &engine_state);

        assert!(path.source.is_some());
        let source = path.source.unwrap();
        assert_eq!(source.format, "FLAC");
        assert_eq!(source.sample_rate_hz, 48_000);
        assert_eq!(source.bits_per_sample, 16);
        assert!(source.lossless);

        assert_eq!(path.plugin_chain.len(), 2);
        assert_eq!(path.plugin_chain[0].plugin_type, "EQ");
        assert_eq!(path.plugin_chain[1].plugin_type, "Gain");
    }

    #[test]
    fn signal_path_detects_resampling_when_source_and_output_rates_differ() {
        let config = sample_saved_config();
        let info = sample_audio_info(44_100);
        let engine_state = AudioEngineState::default();

        let path = SignalPath::from_player_state(Some(&config), Some(&info), &engine_state);

        assert!(path.is_resampled());
        let resampling = path.processing.resampling.unwrap();
        assert_eq!(resampling.from_hz, 44_100);
        assert_eq!(resampling.to_hz, 48_000);
    }

    #[test]
    fn signal_path_no_resampling_when_rates_match() {
        let config = sample_saved_config();
        let info = sample_audio_info(48_000);
        let engine_state = AudioEngineState::default();

        let path = SignalPath::from_player_state(Some(&config), Some(&info), &engine_state);

        assert!(!path.is_resampled());
        assert!(path.processing.resampling.is_none());
    }

    #[test]
    fn signal_path_reports_engine_health_issues() {
        let engine_state = AudioEngineState {
            underruns: 3,
            playback_stream_error_count: 1,
            playback_frames_dropped: 10,
            output_peak_linear: 10.0f32.powf(-6.0 / 20.0),
            output_clipping_detected: true,
            ..Default::default()
        };

        let path = SignalPath::from_player_state(None, None, &engine_state);

        assert!(path.has_known_issues());
        assert_eq!(path.health.underruns, 3);
        assert_eq!(path.health.stream_errors, 1);
        assert_eq!(path.health.frames_dropped, 10);
        assert_eq!(path.health.clipping_detected, Some(true));
        assert!((path.health.headroom_db.unwrap() - 6.0).abs() < 1e-4);
    }
}

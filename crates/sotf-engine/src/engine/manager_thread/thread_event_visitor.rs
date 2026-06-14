//! Visitor dispatch for [`ThreadEvent`]s arriving on the manager thread.
//!
//! This replaces the repetitive `let mut new_state = (**state.load()).clone(); ... state.store(...)`
//! blocks in `handle_thread_event` with focused per-event methods.

use crate::decoder::AudioSource;
use crate::engine::{AudioEngineState, PlaybackState, ThreadEvent};
use arc_swap::ArcSwap;
use std::sync::Arc;

/// Snapshot of playback-thread statistics delivered by [`ThreadEvent::PlaybackStats`].
#[derive(Clone, Copy, Debug, Default)]
pub struct PlaybackStatsSnapshot {
    pub callback_count: u64,
    pub buffer_fill_percent: u64,
    pub stream_error_count: u64,
    pub frames_received: u64,
    pub frames_written: u64,
    pub frames_dropped: u64,
    pub effective_sample_rate: u64,
}

/// Visitor interface for [`ThreadEvent`] variants.
///
/// Default implementations are no-ops so implementors only override the events
/// they care about.
pub trait ThreadEventVisitor {
    fn decoder_end_of_stream(&mut self, _state: &mut AudioEngineState) {}
    fn decoder_gapless_transition(&mut self, _state: &mut AudioEngineState, _source: AudioSource) {}
    fn decoder_error(&mut self, _state: &mut AudioEngineState, _err: String) {}
    fn stream_metadata_changed(
        &mut self,
        _state: &mut AudioEngineState,
        _metadata: Option<crate::engine::StreamMetadata>,
    ) {
    }
    fn playback_channels_changed(&mut self, _state: &mut AudioEngineState, _channels: usize) {}
    fn playback_output_device_changed(&mut self, _state: &mut AudioEngineState, _device: String) {}
    fn playback_output_access_changed(
        &mut self,
        _state: &mut AudioEngineState,
        _status: sotf_types::OutputAccessStatus,
    ) {
    }
    fn playback_stats(&mut self, _state: &mut AudioEngineState, _stats: &PlaybackStatsSnapshot) {}
    fn playback_drained(&mut self, _state: &mut AudioEngineState) {}
    fn playback_underrun(&mut self, _state: &mut AudioEngineState, _underruns: u64) {}
    fn processing_error(&mut self, _state: &mut AudioEngineState, _err: String) {}
    fn processing_warning(&mut self, _state: &mut AudioEngineState, _warning: String) {}
    fn thread_panic(&mut self, _state: &mut AudioEngineState, _thread_name: String) {}
    fn position_update(&mut self, _state: &mut AudioEngineState, _position: f64) {}
    fn seek_complete(&mut self, _state: &mut AudioEngineState) {}
    fn plugin_latency_update(&mut self, _state: &mut AudioEngineState, _latency_samples: usize) {}
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    fn isolated_external_plugin_worker_statuses(
        &mut self,
        _state: &mut AudioEngineState,
        _statuses: Vec<crate::engine::IsolatedExternalPluginWorkerStatus>,
    ) {
    }
}

/// Dispatch an event to a visitor, mutating the provided state in place.
pub fn visit<V: ThreadEventVisitor>(event: ThreadEvent, state: &mut AudioEngineState, visitor: &mut V) {
    match event {
        ThreadEvent::DecoderEndOfStream => visitor.decoder_end_of_stream(state),
        ThreadEvent::DecoderGaplessTransition(source) => {
            visitor.decoder_gapless_transition(state, source)
        }
        ThreadEvent::DecoderError(err) => visitor.decoder_error(state, err),
        ThreadEvent::StreamMetadataChanged(metadata) => {
            visitor.stream_metadata_changed(state, metadata)
        }
        ThreadEvent::PlaybackChannelsChanged(channels) => {
            visitor.playback_channels_changed(state, channels)
        }
        ThreadEvent::PlaybackOutputDeviceChanged(device) => {
            visitor.playback_output_device_changed(state, device)
        }
        ThreadEvent::PlaybackOutputAccessChanged(status) => {
            visitor.playback_output_access_changed(state, status)
        }
        ThreadEvent::PlaybackStats {
            callback_count,
            buffer_fill_percent,
            stream_error_count,
            frames_received,
            frames_written,
            frames_dropped,
            effective_sample_rate,
        } => visitor.playback_stats(
            state,
            &PlaybackStatsSnapshot {
                callback_count,
                buffer_fill_percent,
                stream_error_count,
                frames_received,
                frames_written,
                frames_dropped,
                effective_sample_rate,
            },
        ),
        ThreadEvent::PlaybackDrained => visitor.playback_drained(state),
        ThreadEvent::PlaybackUnderrun(underruns) => visitor.playback_underrun(state, underruns),
        ThreadEvent::ProcessingError(err) => visitor.processing_error(state, err),
        ThreadEvent::ProcessingWarning(warning) => visitor.processing_warning(state, warning),
        ThreadEvent::ThreadPanic(thread_name) => visitor.thread_panic(state, thread_name),
        ThreadEvent::PositionUpdate(position) => visitor.position_update(state, position),
        ThreadEvent::SeekComplete => visitor.seek_complete(state),
        ThreadEvent::PluginLatencyUpdate(latency_samples) => {
            visitor.plugin_latency_update(state, latency_samples)
        }
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        ThreadEvent::IsolatedExternalPluginWorkerStatuses(statuses) => {
            visitor.isolated_external_plugin_worker_statuses(state, statuses)
        }
    }
}

/// Visitor that applies event mutations to the shared [`AudioEngineState`].
pub struct AudioEngineStateUpdater;

impl ThreadEventVisitor for AudioEngineStateUpdater {
    fn decoder_end_of_stream(&mut self, _state: &mut AudioEngineState) {
        log::debug!("[Manager Thread] Decoder end of stream (waiting for playback drain)");
    }

    fn decoder_gapless_transition(&mut self, state: &mut AudioEngineState, source: AudioSource) {
        log::info!(
            "[Manager Thread] Gapless transition to: {}",
            source.display_name()
        );
        state.current_file = source.as_path().map(|p| p.to_path_buf());
        state.current_source = Some(source);
        state.position = 0.0;
    }

    fn decoder_error(&mut self, state: &mut AudioEngineState, err: String) {
        log::debug!("[Manager Thread] Decoder error: {}", err);
        state.playback_state = PlaybackState::Stopped;
        state.last_error = Some(err);
    }

    fn stream_metadata_changed(
        &mut self,
        state: &mut AudioEngineState,
        metadata: Option<crate::engine::StreamMetadata>,
    ) {
        state.stream_metadata = metadata;
    }

    fn playback_channels_changed(&mut self, state: &mut AudioEngineState, channels: usize) {
        state.num_channels = channels;
    }

    fn playback_output_device_changed(&mut self, state: &mut AudioEngineState, device: String) {
        state.playback_output_device = Some(device);
    }

    fn playback_output_access_changed(
        &mut self,
        state: &mut AudioEngineState,
        status: sotf_types::OutputAccessStatus,
    ) {
        state.output_access_status = status;
    }

    fn playback_stats(&mut self, state: &mut AudioEngineState, stats: &PlaybackStatsSnapshot) {
        state.playback_callback_count = stats.callback_count;
        state.playback_buffer_fill_percent = stats.buffer_fill_percent;
        state.playback_stream_error_count = stats.stream_error_count;
        state.playback_frames_received = stats.frames_received;
        state.playback_frames_written = stats.frames_written;
        state.playback_frames_dropped = stats.frames_dropped;
        state.playback_effective_sample_rate = stats.effective_sample_rate;
    }

    fn playback_drained(&mut self, state: &mut AudioEngineState) {
        log::debug!("[Manager Thread] Playback drained - all audio played");
        state.playback_state = PlaybackState::Stopped;
        state.last_error = None;
    }

    fn playback_underrun(&mut self, state: &mut AudioEngineState, underruns: u64) {
        state.underruns = underruns;
        if underruns == 1 || (underruns <= 1000 && underruns.is_multiple_of(100)) {
            log::warn!("[Manager Thread] Playback underrun count: {}", underruns);
        } else if underruns.is_multiple_of(10000) {
            log::debug!("[Manager Thread] Playback underrun count: {}", underruns);
        }
    }

    fn processing_error(&mut self, state: &mut AudioEngineState, err: String) {
        log::debug!("[Manager Thread] Processing error: {}", err);
        state.playback_state = PlaybackState::Stopped;
        state.last_error = Some(err);
    }

    fn processing_warning(&mut self, state: &mut AudioEngineState, warning: String) {
        log::warn!("[Manager Thread] Processing warning: {}", warning);
        state.last_error = Some(warning);
    }

    fn thread_panic(&mut self, state: &mut AudioEngineState, thread_name: String) {
        log::debug!("[Manager Thread] Thread panicked: {}", thread_name);
        state.playback_state = PlaybackState::Stopped;
        state.last_error = Some(format!("Thread panicked: {}", thread_name));
    }

    fn position_update(&mut self, state: &mut AudioEngineState, position: f64) {
        if state.playback_state != PlaybackState::Stopped && !state.seeking {
            let latency_sec = if state.sample_rate > 0
                && state.latency_compensation_enabled
                && !state.processing_bypassed
            {
                state.plugin_latency_samples as f64 / state.sample_rate as f64
            } else {
                0.0
            };
            state.position = (position - latency_sec).max(0.0);
        }
    }

    fn seek_complete(&mut self, state: &mut AudioEngineState) {
        log::debug!("[Manager Thread] Seek complete");
        state.seeking = false;
    }

    fn plugin_latency_update(&mut self, state: &mut AudioEngineState, latency_samples: usize) {
        let old_latency = state.plugin_latency_samples;
        state.plugin_latency_samples = latency_samples;
        if state.sample_rate > 0
            && state.latency_compensation_enabled
            && old_latency != latency_samples
        {
            let delta_sec =
                (latency_samples as f64 - old_latency as f64) / state.sample_rate as f64;
            state.position = (state.position - delta_sec).max(0.0);
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    fn isolated_external_plugin_worker_statuses(
        &mut self,
        state: &mut AudioEngineState,
        statuses: Vec<crate::engine::IsolatedExternalPluginWorkerStatus>,
    ) {
        state.isolated_external_plugin_worker_statuses = statuses;
    }
}

/// Convenience entry point used by `handle_thread_event`.
pub fn update_state_with_event(
    event: ThreadEvent,
    state: &Arc<ArcSwap<AudioEngineState>>,
) {
    let mut new_state = (**state.load()).clone();
    let mut updater = AudioEngineStateUpdater;
    visit(event, &mut new_state, &mut updater);
    state.store(Arc::new(new_state));
}

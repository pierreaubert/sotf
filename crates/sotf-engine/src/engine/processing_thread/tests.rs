use super::super::PluginDataCache;
use super::super::{PluginConfig, ProcessingCommand};
use super::build::build_plugin_host;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::isolated::isolated_external_plugin_event;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::isolated::isolated_external_plugin_sandbox_backend;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::isolated::isolated_external_plugin_status;
use super::misc::create_plugin;
use super::misc::send_or_interrupt;
use super::processing_state::{
    ProcessingState, handle_processing_command, update_plugin_data_cache,
};
use crate::plugins::{PluginSettings, PluginType};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use crate::{
    IsolatedExternalPluginSandboxBackend, IsolatedExternalPluginSandboxStatus,
    IsolatedExternalPluginWorkerEvent,
};
use arc_swap::ArcSwap;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use sotf_plugins::ExternalPluginProcessEvent;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use sotf_plugins::IsolatedExternalPluginWorkerReport;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use sotf_plugins::{PluginSandboxBackendCode, PluginSandboxStatusCode};
use std::sync::Arc;

mod misc;
mod test;

#[test]
fn test_downmix_adapts_to_current_chain_channel_count() {
    let sample_rate = 48000;
    let settings = PluginSettings::default_for(&PluginType::Downmix);
    let config = settings.to_plugin_config(sample_rate as f64);

    let plugin = create_plugin(&config.plugin_type, &config.parameters, 10, sample_rate)
        .expect("downmix should adapt default parameters to the chain width");
    assert_eq!(plugin.input_channels(), 10);
    assert_eq!(plugin.output_channels(), 2);

    let (host, warnings) = build_plugin_host(std::slice::from_ref(&config), sample_rate, 10)
        .expect("host should load adaptive downmix");
    assert!(
        warnings.is_empty(),
        "adaptive downmix should not be skipped: {:?}",
        warnings
    );
    assert_eq!(host.output_channels(), 2);
}

#[test]
fn invalid_spectrum_analyzer_config_is_reported() {
    let config = PluginConfig::new("spectrum_analyzer", serde_json::json!("not an object"));

    let (_host, warnings) = build_plugin_host(&[config], 48_000, 2).unwrap();

    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0].contains("Failed to parse spectrum analyzer params"),
        "unexpected warning: {}",
        warnings[0]
    );
}

/// Test that a non-square matrix (1 input → N outputs) is NOT auto-resized.
/// This is the exact routing used by recording: mono sweep → specific output channel.
/// Regression test for the bug where the matrix was incorrectly resized to 1×1,
/// causing all sweeps to play on channel 0 regardless of output_channel.
#[test]
fn test_matrix_mono_to_multichannel_not_resized() {
    let sample_rate = 48000;

    // Simulate recording routing: mono signal → channel 1 (Right) of a stereo output
    for target_ch in 0..4 {
        let hw_channels = 4;
        let mut matrix = vec![0.0f32; hw_channels];
        matrix[target_ch] = 1.0;

        let matrix_params = serde_json::json!({
            "input_channels": 1,
            "output_channels": hw_channels,
            "matrix": matrix,
        });

        let config = PluginConfig::new("matrix", matrix_params);

        // Chain starts with 1 channel (mono WAV file)
        let (host, _warnings) = build_plugin_host(std::slice::from_ref(&config), sample_rate, 1)
            .unwrap_or_else(|e| {
                panic!(
                    "build_plugin_host failed for 1→{} matrix targeting ch{}: {}",
                    hw_channels, target_ch, e
                )
            });

        // Verify the chain expanded to the correct output channel count
        assert_eq!(
            host.output_channels(),
            hw_channels,
            "Matrix 1→{} should produce {} output channels, got {}",
            hw_channels,
            hw_channels,
            host.output_channels()
        );
    }
}

/// Test that a square matrix IS auto-resized when chain channels differ.
/// E.g., a 2×2 matrix applied to a 4-channel chain should resize to 4×4.
#[test]
fn test_matrix_square_auto_resize() {
    let sample_rate = 48000;

    // 2×2 identity matrix applied to a 4-channel chain
    let matrix_params = serde_json::json!({
        "input_channels": 2,
        "output_channels": 2,
        "matrix": [1.0, 0.0, 0.0, 1.0],
    });

    let config = PluginConfig::new("matrix", matrix_params);
    let (host, _warnings) = build_plugin_host(std::slice::from_ref(&config), sample_rate, 4)
        .expect("build_plugin_host failed for 2×2 matrix on 4ch chain");

    // Should have been resized to 4×4
    assert_eq!(
        host.output_channels(),
        4,
        "Square 2×2 matrix on 4ch chain should auto-resize to 4×4"
    );
}

/// Test that a mono→stereo matrix correctly routes signal to the target channel.
#[test]
fn test_matrix_mono_routing_signal_integrity() {
    let sample_rate = 48000;
    let num_frames = 256;

    // Route mono to channel 1 (Right) of stereo output
    let matrix_params = serde_json::json!({
        "input_channels": 1,
        "output_channels": 2,
        "matrix": [0.0, 1.0],  // silence on L, signal on R
    });

    let config = PluginConfig::new("matrix", matrix_params);
    let (mut host, _warnings) = build_plugin_host(std::slice::from_ref(&config), sample_rate, 1)
        .expect("build_plugin_host failed");

    assert_eq!(host.output_channels(), 2);

    // Mono 440Hz sine input
    let input: Vec<f32> = (0..num_frames)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sample_rate as f32).sin() * 0.5)
        .collect();

    let mut output = vec![0.0f32; num_frames * 2];
    host.process(&input, &mut output).unwrap();

    // Channel 0 (Left) should be silent
    let left_max: f32 = output
        .iter()
        .step_by(2)
        .map(|s| s.abs())
        .fold(0.0, f32::max);
    // Channel 1 (Right) should have signal
    let right_max: f32 = output
        .iter()
        .skip(1)
        .step_by(2)
        .map(|s| s.abs())
        .fold(0.0, f32::max);

    assert!(
        left_max < 1e-6,
        "Left channel should be silent but has max={}",
        left_max
    );
    assert!(
        right_max > 0.1,
        "Right channel should have signal but max={}",
        right_max
    );
}

#[test]
fn crossfade_zero_frame_block_completes_instead_of_leaking_prev_host() {
    assert_eq!(ProcessingState::compute_crossfade_step(0, 48_000), 1.0);
}

#[test]
fn latency_changing_host_update_fades_through_silence_without_a_jump() {
    let sample_rate = 48_000;
    for reverse_rate_change in [false, true] {
        let gain = PluginConfig::new("gain", serde_json::json!({ "gain_db": 0.0 }));
        let resampler = PluginConfig::new(
            "resampler",
            serde_json::json!({
                "input_sample_rate": sample_rate,
                "output_sample_rate": 96_000,
                "chunk_size": 64
            }),
        );
        let (mut gain_host, gain_warnings) = build_plugin_host(&[gain], sample_rate, 1).unwrap();
        let (mut resampler_host, resampler_warnings) =
            build_plugin_host(&[resampler], sample_rate, 1).unwrap();
        assert!(gain_warnings.is_empty());
        assert!(resampler_warnings.is_empty());
        gain_host.build().unwrap();
        resampler_host.build().unwrap();
        let (mut old_host, new_host) = if reverse_rate_change {
            (resampler_host, gain_host)
        } else {
            (gain_host, resampler_host)
        };
        assert_ne!(
            old_host.total_latency_samples(),
            new_host.total_latency_samples()
        );
        for _ in 0..10 {
            let input = vec![1.0f32; 64];
            let old_output_frames = old_host.output_frames_for_input(64);
            let mut old_output = vec![0.0f32; old_output_frames];
            old_host.process(&input, &mut old_output).unwrap();
        }

        let mut state = ProcessingState::new(
            1,
            sample_rate,
            #[cfg(feature = "streaming")]
            None,
        );
        state.host = old_host;
        let (response_tx, _response_rx) = std::sync::mpsc::channel();
        let (event_tx, _event_rx) = std::sync::mpsc::channel();

        assert!(!handle_processing_command(
            ProcessingCommand::UpdateHost(Box::new(new_host)),
            &mut state,
            &response_tx,
            &event_tx,
        ));
        assert!(
            state.prev_host.is_some(),
            "latency-changing update should retain the old host for a safe transition"
        );

        let mut rendered = Vec::new();
        for _ in 0..50 {
            let input = vec![1.0f32; 64];
            let output_frames = state.output_frames_for_input(64);
            let mut output = vec![0.0f32; output_frames];
            let actual = state.process_frame(&input, &mut output, 64).unwrap();
            rendered.extend_from_slice(&output[..actual]);
        }
        let max_jump = rendered
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .fold(0.0f32, f32::max);
        assert!(
            max_jump < 0.05,
            "rate-change direction reverse={reverse_rate_change} introduced an adjacent-sample jump of {max_jump}"
        );
    }
}

#[test]
fn processing_hot_path_uses_prepared_buffers_for_output_and_crossfade() {
    let source = include_str!("../processing_thread.rs");

    assert!(
        !source.contains(concat!("process_buffer.", "resize(output_samples, 0.0)")),
        "processing thread must not allocate/resize the process buffer in the frame hot path"
    );
    assert!(
        !source.contains(concat!("prev_process_buffer.", "resize(buf_len, 0.0)")),
        "crossfade processing must not allocate/resize the previous-host buffer in process_frame"
    );
}

#[test]
fn send_or_interrupt_delivers_message_when_buffer_has_space() {
    let (tx, rx) = std::sync::mpsc::sync_channel::<u32>(4);
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<ProcessingCommand>();

    let handle = std::thread::spawn(move || send_or_interrupt(&tx, &cmd_rx, 42));

    let result = handle.join().expect("thread panicked");
    assert!(result.is_ok());
    assert!(result.unwrap().is_none()); // No interruption
    assert_eq!(rx.recv().unwrap(), 42);
    drop(cmd_tx); // keep cmd_tx alive until assertion
}

#[test]
fn send_or_interrupt_returns_command_when_interrupted_during_backpressure() {
    // Buffer capacity 1, pre-fill it so the next send blocks
    let (tx, rx) = std::sync::mpsc::sync_channel::<u32>(1);
    tx.send(99).unwrap(); // Fill the buffer

    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<ProcessingCommand>();

    // Send a command that will be found during the backpressure retry
    cmd_tx.send(ProcessingCommand::Stop).unwrap();

    let handle = std::thread::spawn(move || send_or_interrupt(&tx, &cmd_rx, 42));

    let result = handle.join().expect("thread panicked");
    let (cmd, unsent_msg) = result.unwrap().expect("should have been interrupted");
    assert!(matches!(cmd, ProcessingCommand::Stop));
    assert_eq!(unsent_msg.unwrap(), 42); // Message returned, not lost
    assert_eq!(rx.recv().unwrap(), 99); // Original message still in buffer
}

#[test]
fn send_or_interrupt_errors_when_channel_disconnected() {
    let (tx, rx) = std::sync::mpsc::sync_channel::<u32>(4);
    let (_cmd_tx, cmd_rx) = std::sync::mpsc::channel::<ProcessingCommand>();
    drop(rx); // Disconnect the receiver

    let result = send_or_interrupt(&tx, &cmd_rx, 42);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("disconnected"));
}

#[test]
fn audio_frame_new_enforces_data_length_invariant() {
    use crate::AudioFrame;

    // Valid: data.len() == num_frames * num_channels
    let frame = AudioFrame::new(vec![0.0; 2048], 1024, 2, 48000);
    assert_eq!(frame.num_samples(), 2048);
    assert_eq!(frame.num_frames, 1024);
    assert_eq!(frame.num_channels, 2);
}

#[test]
fn audio_frame_silent_produces_all_zeros() {
    use crate::AudioFrame;

    let frame = AudioFrame::silent(512, 6, 48000);
    assert_eq!(frame.data.len(), 512 * 6);
    assert!(frame.data.iter().all(|&s| s == 0.0));
}

#[test]
fn audio_frame_clear_resets_to_silence() {
    use crate::AudioFrame;

    let mut frame = AudioFrame::new(vec![1.0; 1024], 512, 2, 48000);
    assert!(frame.data.iter().all(|&s| s == 1.0));

    frame.clear();
    assert!(frame.data.iter().all(|&s| s == 0.0));
    // Metadata unchanged
    assert_eq!(frame.num_frames, 512);
    assert_eq!(frame.num_channels, 2);
}

#[test]
fn audio_frame_invariants_across_channel_counts() {
    use crate::AudioFrame;

    for channels in [1, 2, 4, 6, 8] {
        let frames = 256;
        let total = frames * channels;
        let data: Vec<f32> = (0..total).map(|i| i as f32 / total as f32).collect();
        let frame = AudioFrame::new(data, frames, channels, 48000);

        assert_eq!(frame.num_samples(), total);
        assert_eq!(frame.data.len(), total);
        // All samples in [-1, 1) range for this test data
        assert!(frame.data.iter().all(|&s| (0.0..1.0).contains(&s)));
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn test_isolated_external_plugin_event_and_status_mappings() {
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt;

    let event = isolated_external_plugin_event(ExternalPluginProcessEvent::AlreadyRunning);
    assert!(matches!(
        event,
        IsolatedExternalPluginWorkerEvent::AlreadyRunning
    ));

    let event = isolated_external_plugin_event(ExternalPluginProcessEvent::NotRunning);
    assert!(matches!(
        event,
        IsolatedExternalPluginWorkerEvent::NotRunning
    ));

    let event = isolated_external_plugin_event(ExternalPluginProcessEvent::Started { pid: 555 });
    assert!(matches!(
        event,
        IsolatedExternalPluginWorkerEvent::Started { pid } if pid == 555
    ));

    #[cfg(unix)]
    {
        let status = ExitStatusExt::from_raw(11 << 8);
        let event = isolated_external_plugin_event(ExternalPluginProcessEvent::Exited { status });
        assert!(matches!(
            event,
            IsolatedExternalPluginWorkerEvent::Exited {
                exit_code: Some(11)
            }
        ));
    }
    #[cfg(windows)]
    {
        let status = ExitStatusExt::from_raw((11 << 8) as u32);
        let event = isolated_external_plugin_event(ExternalPluginProcessEvent::Exited { status });
        assert!(matches!(
            event,
            IsolatedExternalPluginWorkerEvent::Exited {
                exit_code: Some(11)
            }
        ));
    }

    let report = IsolatedExternalPluginWorkerReport {
        plugin_index: 3,
        node_id: 9,
        event: Some(ExternalPluginProcessEvent::Started { pid: 777 }),
        error: Some("blocked".into()),
        worker_start_count: 4,
        worker_exit_count: 2,
        worker_launch_failure_count: 1,
        block_timeout_count: 3,
        block_worker_failure_count: 4,
        block_wrong_sequence_count: 5,
        sandbox_status: PluginSandboxStatusCode::Enforced,
        sandbox_backend: PluginSandboxBackendCode::LinuxLandlock,
        sandbox_reason: None,
    };
    let status = isolated_external_plugin_status(report);
    assert_eq!(status.plugin_index, 3);
    assert_eq!(status.node_id, 9);
    assert_eq!(status.error, Some("blocked".into()));
    assert_eq!(status.worker_start_count, 4);
    assert_eq!(status.worker_exit_count, 2);
    assert_eq!(status.worker_launch_failure_count, 1);
    assert_eq!(status.block_timeout_count, 3);
    assert_eq!(status.block_worker_failure_count, 4);
    assert_eq!(status.block_wrong_sequence_count, 5);
    assert_eq!(
        status.sandbox_status,
        IsolatedExternalPluginSandboxStatus::Enforced
    );
    assert_eq!(
        status.sandbox_backend,
        IsolatedExternalPluginSandboxBackend::LinuxLandlock
    );
    assert_eq!(status.sandbox_reason, None);
    assert!(matches!(
        status.event,
        Some(IsolatedExternalPluginWorkerEvent::Started { pid }) if pid == 777
    ));
    assert_eq!(
        isolated_external_plugin_sandbox_backend(PluginSandboxBackendCode::MacosAppSandboxHelper),
        IsolatedExternalPluginSandboxBackend::MacosAppSandboxHelper
    );
}

/// Regression test for the analyzer-cache fallback allocation path.
///
/// When a UI reader holds the current cache Arc, the processing thread must
/// skip the update rather than count the contention as a fallback allocation.
#[test]
fn plugin_cache_update_skips_under_ui_contention_without_fallback() {
    let sample_rate = 48_000;
    let config = PluginConfig::new("spectrum_analyzer", serde_json::json!(null));
    let (mut host, warnings) = build_plugin_host(std::slice::from_ref(&config), sample_rate, 2)
        .expect("spectrum analyzer host should build");
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    host.build().expect("host should build");
    assert!(
        !host.analyzer_indices().is_empty(),
        "spectrum analyzer must register as an analyzer"
    );

    let plugin_count = host.plugin_count();

    let mut state = ProcessingState::new(
        host.output_channels(),
        sample_rate,
        #[cfg(feature = "streaming")]
        None,
    );
    state.host = host;

    // Pre-size both the published cache and the spare so no bootstrap resize
    // is needed.  This mirrors what UpdateHost does in the real thread.
    let plugin_data_cache: PluginDataCache =
        Arc::new(ArcSwap::from_pointee(vec![None; plugin_count]));
    state.spare_cache_arc = Some(Arc::new(vec![None; plugin_count]));

    // Simulate a UI reader that keeps a clone of the current cache Arc.
    let _ui_holder = Arc::clone(&*plugin_data_cache.load());

    // Run enough updates that some will hit contention (the UI holds the Arc
    // that becomes the spare after the first successful swap).
    let mut updated_frames = 0;
    for _ in 0..20 {
        if update_plugin_data_cache(&mut state, &plugin_data_cache) {
            updated_frames += 1;
        }
    }

    assert!(
        updated_frames > 0,
        "at least one cache update should succeed before the UI clone causes contention"
    );
    assert!(
        updated_frames < 20,
        "some updates should be skipped due to simulated UI contention"
    );
    assert_eq!(
        state.cache_fallback_count, 0,
        "UI contention must not be reported as a cache fallback allocation"
    );
}

/// If the spare cache Arc is unexpectedly missing, the processing hot path
/// must skip this frame rather than allocate a replacement cache.
#[test]
fn plugin_cache_update_skips_missing_spare_without_allocating() {
    let sample_rate = 48_000;
    let config = PluginConfig::new("spectrum_analyzer", serde_json::json!(null));
    let (mut host, warnings) = build_plugin_host(std::slice::from_ref(&config), sample_rate, 2)
        .expect("spectrum analyzer host should build");
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    host.build().expect("host should build");

    let plugin_count = host.plugin_count();

    let mut state = ProcessingState::new(
        host.output_channels(),
        sample_rate,
        #[cfg(feature = "streaming")]
        None,
    );
    state.host = host;
    state.spare_cache_arc = None;

    let plugin_data_cache: PluginDataCache =
        Arc::new(ArcSwap::from_pointee(vec![None; plugin_count]));

    assert!(!update_plugin_data_cache(&mut state, &plugin_data_cache));
    assert_eq!(state.cache_fallback_count, 1);
    assert!(state.spare_cache_arc.is_none());
}

#[test]
fn processing_thread_idle_wait_blocks_instead_of_micro_spinning() {
    let source = include_str!("processing_state.rs");
    assert!(
        source.contains("let mut decoder_stream_active = true"),
        "processing thread should start in low-latency mode before the first decoded frame"
    );
    assert!(
        source.contains("IDLE_EMPTY_SLEEP_PROCESSING_MS"),
        "processing thread should use a coarser wait after the decoder has gone idle"
    );
    assert!(
        source.contains("recv_timeout"),
        "processing thread should wake immediately when decoder frames arrive"
    );
    assert!(
        !source.contains("TryRecvError::Empty"),
        "processing thread must not sleep after an empty try_recv"
    );
}

#[test]
fn recycle_queue_prefill_is_generous() {
    let source = include_str!("../manager_thread/config_update_queue.rs");
    assert!(
        source.contains("queue_capacity * 4"),
        "recycle queues should be pre-filled with a generous multiple of queue capacity"
    );
    assert!(
        source.contains("frame_size * 64"),
        "recycle buffers should be sized for high channel counts and resampler headroom"
    );
}

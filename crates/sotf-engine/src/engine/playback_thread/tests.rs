use super::super::plan_output_access;
use super::apply::apply_volume;
use super::apply::apply_volume_clamp;
use super::misc::fallback_output_format;
use super::misc::initial_buffer_size;
use super::misc::is_virtual_output_device_name;
use super::pick::pick_preferred_output_format;
use super::playback::playback_buffer_capacity;
use super::playback::playback_recovery_reason;
use super::playback_state::PlaybackState;
use super::playback_state::flush_completed;
use super::playback_state::read_ring_buffer;
use super::playback_state::request_flush;
use super::runtime::minimum_ring_space_required;
use super::runtime::should_emit_underrun_milestone;
use crate::{OutputAccessMode, OutputAccessStatus};
use cpal::SampleFormat;
use rtrb::RingBuffer;
use std::sync::atomic::Ordering;

#[cfg(test)]
fn output_access_status_for_device(
    mode: OutputAccessMode,
    output_device: Option<&str>,
) -> OutputAccessStatus {
    plan_output_access(mode, output_device).status
}

#[test]
fn output_meter_tracks_post_volume_peak_before_clamp() {
    let state = PlaybackState::new(16);
    state.volume.store(2.0f32.to_bits(), Ordering::Relaxed);
    let mut samples = [0.25, -0.6, 0.1];

    apply_volume_clamp(&mut samples, &state);

    assert_eq!(samples, [0.5, -1.0, 0.2]);
    let peak = f32::from_bits(state.output_peak_bits.load(Ordering::Relaxed));
    assert!((peak - 1.2).abs() < 1e-6);
    assert_eq!(state.clipped_sample_count.load(Ordering::Relaxed), 1);
}

#[test]
fn output_meter_treats_non_finite_samples_as_clipping() {
    let state = PlaybackState::new(16);
    let mut samples = [0.5, f32::NAN];

    apply_volume(&mut samples, &state);

    let peak = f32::from_bits(state.output_peak_bits.load(Ordering::Relaxed));
    assert!((peak - 0.5).abs() < 1e-6);
    assert_eq!(state.clipped_sample_count.load(Ordering::Relaxed), 1);
}

#[test]
fn output_meter_reset_drops_partial_previous_window() {
    let state = PlaybackState::new(16);
    state
        .output_peak_bits
        .store(1.25f32.to_bits(), Ordering::Relaxed);
    state.clipped_sample_count.store(3, Ordering::Relaxed);

    state.reset_output_meter();

    assert_eq!(
        f32::from_bits(state.output_peak_bits.load(Ordering::Relaxed)),
        0.0
    );
    assert_eq!(state.clipped_sample_count.load(Ordering::Relaxed), 0);
}

#[test]
fn flush_completion_waits_for_callback_before_meter_reset() {
    let (producer, _consumer) = RingBuffer::<f32>::new(16);
    let state = PlaybackState::new(16);
    request_flush(&state);
    state
        .output_peak_bits
        .store(1.25f32.to_bits(), Ordering::Relaxed);
    state.clipped_sample_count.store(3, Ordering::Relaxed);
    state.output_callback_active.store(true, Ordering::Release);

    assert!(!flush_completed(&state, &producer, 16));
    state.output_callback_active.store(false, Ordering::Release);
    assert!(flush_completed(&state, &producer, 16));

    state.reset_output_meter();
    assert_eq!(
        f32::from_bits(state.output_peak_bits.load(Ordering::Relaxed)),
        0.0
    );
    assert_eq!(state.clipped_sample_count.load(Ordering::Relaxed), 0);
}
use std::time::{Duration, Instant};

#[test]
fn playback_stream_error_callbacks_gate_event_formatting() {
    let source = include_str!("build.rs");

    assert!(
        source.contains("if crate::rate_limit::allow(&EVENT_GATE, 5_000_000_000)"),
        "playback stream-error callbacks must rate-limit event formatting/sending"
    );
}

#[test]
fn primary_f32_output_clamps_after_volume() {
    let source = include_str!("build.rs");

    assert!(
        source.contains("apply_volume_clamp(data, &state_clone);"),
        "primary f32 playback callback must clamp after volume before handing samples to CPAL"
    );
}

#[test]
fn playback_frame_writer_hot_path_has_no_logging_or_formatting() {
    let source = include_str!("frame_writer.rs");

    assert!(
        !source.contains("log::")
            && !source.contains("rate_limited_log!")
            && !source.contains("format!")
            && !source.contains("ThreadEvent::"),
        "playback frame writer hot path must not log or format events"
    );
}

#[test]
fn playback_rebuild_start_failure_resumes_previous_stream() {
    let source = include_str!("runtime.rs");

    assert!(
        source.contains("resume_previous_stream_after_start_failure")
            && source.contains("resumed previous playback stream"),
        "sample-rate/channel rebuild start failures must resume the previous stream instead of leaving playback paused"
    );
}

#[test]
fn playback_does_not_retry_with_device_native_channel_count() {
    let source = include_str!("runtime.rs");
    let forbidden = ["retrying", "with", "device", "native"].join(" ");

    assert!(
        !source.contains(&forbidden),
        "playback must not silently inflate requested stream width to device-native channels"
    );
}

#[test]
fn macos_playback_recovers_when_coreaudio_device_id_changes() {
    let source = include_str!("runtime.rs");

    assert!(
        source.contains("coreaudio_output_device_id(&device_name)")
            && source.contains("CoreAudio device id changed")
            && source.contains("rebuild_playback_stream("),
        "playback should rebuild the output stream when macOS resurrects a named device under a new CoreAudio device id"
    );
}

#[test]
fn macos_exclusive_output_uses_coreaudio_hog_mode_guard() {
    let source = [
        include_str!("core_audio_exclusive_mode_guard.rs"),
        include_str!("runtime.rs"),
        include_str!("misc.rs"),
    ]
    .join("\n");

    assert!(
        source.contains("struct CoreAudioExclusiveModeGuard")
            && source.contains("get_hogging_pid(device_id)")
            && source.contains("toggle_hog_mode(device_id)")
            && source.contains("impl Drop for CoreAudioExclusiveModeGuard")
            && source.contains("activate_for_device(&device_name, output_access)")
            && source.contains("PlaybackOutputAccessChanged(new_status)"),
        "macOS exclusive output should acquire CoreAudio hog mode before stream build, publish access changes, and release ownership on drop"
    );
}

#[test]
fn playback_recovery_ignores_coreaudio_identity_change_while_callbacks_advance() {
    let mut last_stream_error_count = 0;
    let mut last_callback_count = 41;
    let mut last_callback_check = Instant::now() - Duration::from_secs(10);

    let recovery = playback_recovery_reason(
        0,
        &mut last_stream_error_count,
        42,
        &mut last_callback_count,
        &mut last_callback_check,
        Duration::from_secs(3),
        100,
        99,
        Some("CoreAudio device id changed".to_string()),
    );

    assert_eq!(recovery, None);
    assert_eq!(last_callback_count, 42);
}

#[test]
fn explicit_output_device_lookup_does_not_silently_fallback() {
    let source = include_str!("misc.rs");
    let explicit_lookup = source
        .split("match crate::devices::find_device(host, device_identifier, false)")
        .nth(1)
        .expect("explicit lookup branch should exist")
        .split("} else {")
        .next()
        .expect("explicit lookup branch should end before default-device branch");

    assert!(
        explicit_lookup.contains("Selected output device")
            && !explicit_lookup.contains("find_fallback()"),
        "an explicit user-selected output must fail loudly instead of falling back to another physical device"
    );
}

#[test]
fn explicit_virtual_output_device_is_honored_when_selected() {
    let source = include_str!("misc.rs");

    assert!(
        source
            .contains("is_virtual_output_device_name(device_identifier) && !allow_virtual_output")
            && source.contains("Explicit virtual output device")
            && source.contains("honoring selection"),
        "explicit virtual output selection should be honored; only implicit defaults are guarded"
    );
}

#[test]
fn default_selection_avoids_opening_virtual_output_when_not_allowed() {
    let source = include_str!("misc.rs");
    let default_branch = source
        .split("} else if !allow_virtual_output {")
        .nth(1)
        .expect("safe default branch should exist")
        .split("} else {")
        .next()
        .expect("safe default branch should end before virtual-allowed branch");

    assert!(
        default_branch.contains("find_physical_output()")
            && !default_branch.contains("default_output_device()"),
        "systemwide default selection must scan physical outputs without opening the virtual default device"
    );
}

#[test]
fn playback_stats_publish_even_before_frames_arrive() {
    let source = include_str!("runtime.rs");
    let diagnostics_block = source
        .split("fn emit_periodic_diagnostics(&mut self)")
        .nth(1)
        .expect("periodic diagnostics handler should exist")
        .split("fn handle_next_message(&mut self)")
        .next()
        .expect("periodic diagnostics handler should end before queue handling");

    assert!(
        diagnostics_block.contains("ThreadEvent::PlaybackStats")
            && !diagnostics_block.contains("frames_received > 0"),
        "playback stats must report callbacks/underruns during upstream starvation"
    );
}

#[test]
fn playback_underruns_are_not_reported_during_end_of_stream_drain() {
    assert!(should_emit_underrun_milestone(false, 1, 0));
    assert!(should_emit_underrun_milestone(false, 100, 1));
    assert!(!should_emit_underrun_milestone(false, 2, 1));
    assert!(!should_emit_underrun_milestone(false, 100, 100));
    assert!(!should_emit_underrun_milestone(true, 1, 0));
    assert!(!should_emit_underrun_milestone(true, 200, 100));
}

#[test]
fn ios_stub_writes_frame_data_to_ring_buffer_in_bulk() {
    let source = include_str!("../playback_thread_stub.rs");

    assert!(
        !source.contains(concat!("fill_from_iter(frame.data.", "iter().copied())")),
        "iOS playback feeder should bulk-copy frame data into ring-buffer chunks"
    );
}

#[test]
fn read_ring_buffer_discards_samples_while_flush_requested() {
    let (mut producer, mut consumer) = RingBuffer::<f32>::new(8);
    let chunk = producer.write_chunk_uninit(4).unwrap();
    chunk.fill_from_iter([0.25, 0.5, 0.75, 1.0]);

    let state = PlaybackState::new(8);
    request_flush(&state);
    let mut scratch = [1.0; 4];

    let underrun = read_ring_buffer(&mut consumer, &mut scratch, 4, &state, 8);

    assert!(!underrun);
    assert_eq!(scratch, [0.0; 4]);
    assert_eq!(consumer.slots(), 0);
    assert!(!state.flush_requested.load(Ordering::Relaxed));
}

#[test]
fn read_ring_buffer_keeps_flush_requested_until_buffer_is_empty() {
    let (mut producer, mut consumer) = RingBuffer::<f32>::new(8);
    let chunk = producer.write_chunk_uninit(8).unwrap();
    chunk.fill_from_iter([0.0; 8]);

    let state = PlaybackState::new(8);
    request_flush(&state);
    let mut scratch = [1.0; 4];

    read_ring_buffer(&mut consumer, &mut scratch, 4, &state, 8);
    assert_eq!(scratch, [0.0; 4]);
    assert_eq!(consumer.slots(), 4);
    assert!(state.flush_requested.load(Ordering::Relaxed));

    read_ring_buffer(&mut consumer, &mut scratch, 4, &state, 8);
    assert_eq!(scratch, [0.0; 4]);
    assert_eq!(consumer.slots(), 0);
    assert!(!state.flush_requested.load(Ordering::Relaxed));
}

#[test]
fn read_ring_buffer_counts_only_consumed_samples_on_underrun() {
    let (mut producer, mut consumer) = RingBuffer::<f32>::new(8);
    let chunk = producer.write_chunk_uninit(3).unwrap();
    chunk.fill_from_iter([0.25, 0.5, 0.75]);

    let state = PlaybackState::new(8);
    let mut scratch = [1.0; 6];

    let underrun = read_ring_buffer(&mut consumer, &mut scratch, 6, &state, 8);

    assert!(underrun);
    assert_eq!(scratch, [0.25, 0.5, 0.75, 0.0, 0.0, 0.0]);
    assert_eq!(state.total_callback_samples.load(Ordering::Relaxed), 3);
}

#[test]
fn f32_output_volume_at_unity_does_not_clip_samples() {
    let state = PlaybackState::new(8);
    let mut scratch = [-1.25, -0.5, 0.5, 1.25];

    apply_volume(&mut scratch, &state);

    assert_eq!(scratch, [-1.25, -0.5, 0.5, 1.25]);
}

#[test]
fn playback_buffer_capacity_uses_configured_buffer_ms() {
    assert_eq!(playback_buffer_capacity(48_000, 2, 200), 19_200);
}

#[test]
fn playback_buffer_capacity_scales_with_latency_budget() {
    assert_eq!(playback_buffer_capacity(48_000, 2, 100), 9_600);
    assert_eq!(playback_buffer_capacity(48_000, 2, 250), 24_000);
}

#[test]
fn playback_buffer_capacity_rounds_up_after_channel_scaling() {
    assert_eq!(playback_buffer_capacity(44_100, 6, 1), 265);
}

#[test]
fn playback_ring_space_gate_clamps_to_buffer_capacity() {
    assert_eq!(minimum_ring_space_required(512, 2, 4096), 1024);
    assert_eq!(minimum_ring_space_required(4096, 2, 96), 96);
    assert_eq!(minimum_ring_space_required(usize::MAX, 2, 96), 96);
}

#[test]
fn exclusive_preferred_reports_platform_initial_status_for_cpal_devices() {
    #[cfg(target_os = "macos")]
    let expected = OutputAccessStatus::ExclusivePending;
    #[cfg(not(target_os = "macos"))]
    let expected = OutputAccessStatus::FallbackShared;

    assert_eq!(
        output_access_status_for_device(OutputAccessMode::ExclusivePreferred, None),
        expected
    );
}

#[test]
fn exclusive_required_reports_platform_initial_status_without_exclusive_backend() {
    #[cfg(target_os = "macos")]
    let expected = OutputAccessStatus::ExclusivePending;
    #[cfg(not(target_os = "macos"))]
    let expected = OutputAccessStatus::Unsupported;

    assert_eq!(
        output_access_status_for_device(OutputAccessMode::ExclusiveRequired, None),
        expected
    );
}

#[test]
fn asio_output_reports_exclusive_active() {
    assert_eq!(
        output_access_status_for_device(
            OutputAccessMode::ExclusivePreferred,
            Some("ASIO:Focusrite USB ASIO"),
        ),
        OutputAccessStatus::ExclusiveActive
    );
}

#[test]
fn exclusive_active_uses_fixed_initial_buffer_size() {
    assert_eq!(
        initial_buffer_size(OutputAccessStatus::ExclusiveActive, 256),
        cpal::BufferSize::Fixed(256)
    );
    assert_eq!(
        initial_buffer_size(OutputAccessStatus::FallbackShared, 256),
        cpal::BufferSize::Default
    );
    assert_eq!(
        initial_buffer_size(OutputAccessStatus::ExclusivePending, 256),
        cpal::BufferSize::Default
    );
}

#[test]
fn pick_preferred_output_format_falls_back_to_unsigned_formats() {
    let candidates = vec![
        (SampleFormat::U16, 2, 44_100, 48_000),
        (SampleFormat::U32, 2, 44_100, 48_000),
    ];

    assert_eq!(
        pick_preferred_output_format(&candidates, 2, 48_000),
        Some(SampleFormat::U32)
    );
}

#[test]
fn pick_preferred_output_format_prefers_signed_formats_before_unsigned() {
    let candidates = vec![
        (SampleFormat::U32, 2, 44_100, 48_000),
        (SampleFormat::I16, 2, 44_100, 48_000),
    ];

    assert_eq!(
        pick_preferred_output_format(&candidates, 2, 48_000),
        Some(SampleFormat::I16)
    );
}

#[test]
fn fallback_output_format_prefers_device_default_when_available() {
    assert_eq!(
        fallback_output_format(Some((SampleFormat::U16, 6)), 2),
        (SampleFormat::U16, 6)
    );
}

#[test]
fn fallback_output_format_defaults_to_f32_requested_channels_when_missing() {
    assert_eq!(fallback_output_format(None, 2), (SampleFormat::F32, 2));
}

#[test]
fn is_virtual_output_device_name_matches_known_virtual_outputs() {
    assert!(is_virtual_output_device_name("SotF Virtual Output"));
    assert!(is_virtual_output_device_name("BlackHole 2ch"));
    assert!(is_virtual_output_device_name("ZoomAudioDevice"));
    assert!(is_virtual_output_device_name("Loopback Audio"));
    assert!(is_virtual_output_device_name("Soundflower (2ch)"));
    assert!(is_virtual_output_device_name("Background Music"));
    assert!(is_virtual_output_device_name("Audio Bridge"));
    assert!(is_virtual_output_device_name("Generic Virtual Device"));
    assert!(is_virtual_output_device_name("blackhole 2ch"));
    assert!(is_virtual_output_device_name("zoomaudiodevice"));
    assert!(is_virtual_output_device_name("loopback audio"));
}

#[test]
fn is_virtual_output_device_name_allows_regular_physical_outputs() {
    assert!(!is_virtual_output_device_name("Built-in Output"));
}

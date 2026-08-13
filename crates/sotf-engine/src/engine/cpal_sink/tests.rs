use super::CpalSink;
use super::cpal_playback_state::CpalPlaybackState;
use super::cpal_playback_state::read_ring_buffer;
use super::misc::fallback_output_format;
use super::misc::is_virtual_output_device_name;
use super::misc::playback_buffer_capacity;
use super::misc::should_fallback_from_virtual_default;
use super::pick::pick_format_any_channels;
use super::pick::pick_preferred_output_format;
use super::pick::pick_wider_hardware_format;
use cpal::SampleFormat;
use rtrb::RingBuffer;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::*;

fn sink_config(sample_rate: u32, channels: usize, buffer_ms: u32) -> SinkConfig {
    SinkConfig {
        sample_rate,
        channels,
        buffer_ms,
        device: None,
        allow_virtual_output: false,
    }
}

#[test]
fn cpal_sink_rejects_zero_sample_rate_before_opening_device() {
    let error = CpalSink::validate_config(&sink_config(0, 2, 200)).unwrap_err();
    assert!(error.contains("sample rate"));
}

#[test]
fn cpal_sink_rejects_zero_channels_before_opening_device() {
    let error = CpalSink::validate_config(&sink_config(48_000, 0, 200)).unwrap_err();
    assert!(error.contains("channel count"));
}

#[test]
fn cpal_sink_rejects_channel_counts_that_cpal_cannot_represent() {
    let error =
        CpalSink::validate_config(&sink_config(48_000, u16::MAX as usize + 1, 200)).unwrap_err();
    assert!(error.contains("cpal limit"));
}

#[test]
fn cpal_sink_rejects_zero_buffer_duration() {
    let error = CpalSink::validate_config(&sink_config(48_000, 2, 0)).unwrap_err();
    assert!(error.contains("buffer duration"));
}

#[test]
fn stream_error_callbacks_gate_event_formatting() {
    let source = include_str!("build.rs");

    assert!(
        source.contains("if crate::rate_limit::allow(&EVENT_GATE, 5_000_000_000)"),
        "CPAL sink stream-error callbacks must rate-limit event formatting/sending"
    );
}

#[test]
fn equal_channel_f32_output_clamps_after_volume() {
    let source = include_str!("build.rs");

    assert!(
        source.contains("apply_volume_clamp(data, &state_clone, logical_channels, sample_rate);"),
        "equal-channel f32 CPAL output must clamp after volume"
    );
}

#[test]
fn mapped_channel_f32_output_clamps_after_volume() {
    let source = include_str!("build.rs");

    assert!(
        source.contains("&mut scratch[..logical_len],\n            state,\n            logical_channels,\n            sample_rate,"),
        "mapped-channel f32 CPAL output must clamp before hardware mapping"
    );
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
fn pick_format_any_channels_prefers_float_without_inflating_channels() {
    let candidates = vec![
        (SampleFormat::I16, 5, 44_100, 96_000),
        (SampleFormat::F32, 94, 44_100, 96_000),
    ];

    assert_eq!(
        pick_format_any_channels(&candidates, 48_000),
        Some(SampleFormat::F32)
    );
}

#[test]
fn pick_wider_hardware_format_uses_smallest_compatible_width() {
    let candidates = vec![
        (SampleFormat::I16, 94, 44_100, 96_000),
        (SampleFormat::F32, 12, 44_100, 96_000),
        (SampleFormat::I32, 32, 44_100, 96_000),
    ];

    assert_eq!(
        pick_wider_hardware_format(&candidates, 10, 48_000),
        Some((SampleFormat::F32, 12))
    );
}

#[test]
fn pick_wider_hardware_format_ignores_exact_and_lower_widths() {
    let candidates = vec![
        (SampleFormat::F32, 2, 44_100, 96_000),
        (SampleFormat::F32, 1, 44_100, 96_000),
    ];

    assert_eq!(pick_wider_hardware_format(&candidates, 2, 48_000), None);
}

#[test]
fn mapped_hardware_output_preserves_logical_channels_and_zeros_extras() {
    let logical = [1.0, 2.0, 3.0, 4.0];
    let mut hardware = [9.0; 8];

    super::build::write_logical_to_hardware_f32(&logical, &mut hardware, 2, 4);

    assert_eq!(hardware, [1.0, 2.0, 0.0, 0.0, 3.0, 4.0, 0.0, 0.0]);
}

#[test]
fn fallback_output_format_prefers_device_default_when_available() {
    assert_eq!(
        fallback_output_format(Some((SampleFormat::U16, 6)), 2),
        (SampleFormat::U16, 6)
    );
}

#[test]
fn stall_check_does_not_use_mutex() {
    let source = include_str!("../cpal_sink.rs");
    assert!(
        !source.contains("Mutex<StallCheckState>"),
        "CpalSink stall_check must not be wrapped in a Mutex (real-time safety)"
    );
    assert!(
        !source.contains("stall_check.lock()"),
        "CpalSink stall_check methods must not acquire a mutex"
    );
}

#[test]
fn is_stalled_updates_observed_callback_count_without_external_polling() {
    let mut sink = CpalSink::new();
    let state = Arc::new(CpalPlaybackState::new(128));
    sink.state = Some(Arc::clone(&state));

    // Simulate that the last successful callback check happened 4 seconds ago.
    let epoch_nanos = sink.stall_check.epoch.elapsed().as_nanos() as u64;
    sink.stall_check
        .last_callback_check_nanos
        .store(epoch_nanos.saturating_sub(4_000_000_000), Ordering::Relaxed);

    state.callback_count.store(1, Ordering::Relaxed);

    assert!(!sink.is_stalled());
    assert_eq!(
        sink.stall_check.last_callback_count.load(Ordering::Relaxed),
        1
    );
}

#[test]
fn is_stalled_returns_true_when_callback_count_is_unchanged_for_three_seconds() {
    let mut sink = CpalSink::new();
    let state = Arc::new(CpalPlaybackState::new(128));
    sink.state = Some(Arc::clone(&state));

    // Place the epoch 10 s in the past so the atomic timestamps are meaningful
    // without having to sleep in the test.
    sink.stall_check = super::stall_check_state::StallCheckState {
        epoch: std::time::Instant::now() - std::time::Duration::from_secs(10),
        last_callback_count: std::sync::atomic::AtomicU64::new(5),
        last_callback_check_nanos: std::sync::atomic::AtomicU64::new(6_000_000_000),
    };

    state.callback_count.store(5, Ordering::Relaxed);

    assert!(sink.is_stalled());
}

#[test]
fn fallback_output_format_defaults_to_f32_requested_channels_when_missing() {
    assert_eq!(fallback_output_format(None, 2), (SampleFormat::F32, 2));
}

#[test]
fn read_ring_buffer_counts_only_consumed_samples_on_underrun() {
    let (mut producer, mut consumer) = RingBuffer::<f32>::new(8);
    let chunk = producer.write_chunk_uninit(3).unwrap();
    chunk.fill_from_iter([0.25, 0.5, 0.75]);

    let state = CpalPlaybackState::new(8);
    let mut scratch = [1.0; 6];

    let underrun = read_ring_buffer(&mut consumer, &mut scratch, 6, &state, 8);

    assert!(underrun);
    assert_eq!(scratch, [0.25, 0.5, 0.75, 0.0, 0.0, 0.0]);
    assert_eq!(state.total_callback_samples.load(Ordering::Relaxed), 3);
}

#[test]
fn read_ring_buffer_does_not_count_samples_discarded_by_flush() {
    let (mut producer, mut consumer) = RingBuffer::<f32>::new(8);
    let chunk = producer.write_chunk_uninit(4).unwrap();
    chunk.fill_from_iter([0.25, 0.5, 0.75, 1.0]);

    let state = CpalPlaybackState::new(8);
    state.flush_requested.store(true, Ordering::Relaxed);
    let mut scratch = [1.0; 4];
    read_ring_buffer(&mut consumer, &mut scratch, 4, &state, 8);

    assert_eq!(scratch, [0.0; 4]);
    assert_eq!(state.total_callback_samples.load(Ordering::Relaxed), 0);
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
}

#[test]
fn is_virtual_output_device_name_allows_regular_physical_outputs() {
    assert!(!is_virtual_output_device_name("Built-in Output"));
}

#[test]
fn virtual_output_fallback_only_applies_to_implicit_default_device() {
    assert!(should_fallback_from_virtual_default(
        None,
        "SotF Virtual Output",
        false
    ));
    assert!(!should_fallback_from_virtual_default(
        Some("SotF Virtual Output"),
        "SotF Virtual Output",
        false
    ));
    assert!(!should_fallback_from_virtual_default(
        None,
        "SotF Virtual Output",
        true
    ));
    assert!(!should_fallback_from_virtual_default(
        None,
        "Built-in Output",
        false
    ));
}

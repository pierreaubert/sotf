#[allow(dead_code)]
#[path = "../src/devices_stub.rs"]
mod devices_stub;

use std::sync::{Arc, Mutex};

use devices_stub::{
    AudioConfig, AudioState, IOS_SYSTEM_OUTPUT_ID, get_audio_config, set_audio_device,
    verify_working_sample_rate,
};

fn output_config(sample_rate: u32, channels: u16, sample_format: &str) -> AudioConfig {
    AudioConfig {
        sample_rate,
        channels,
        buffer_size: None,
        sample_format: sample_format.to_string(),
    }
}

#[test]
fn ios_system_output_can_be_selected() {
    let state = Arc::new(Mutex::new(AudioState::default()));
    let config = output_config(48_000, 2, "f32");

    set_audio_device(
        IOS_SYSTEM_OUTPUT_ID.to_string(),
        false,
        config.clone(),
        &state,
    )
    .unwrap();
    let current = get_audio_config(&state).unwrap();

    assert_eq!(
        current.selected_output_device.as_deref(),
        Some(IOS_SYSTEM_OUTPUT_ID)
    );
    let output_config = current.output_config.unwrap();
    assert_eq!(output_config.sample_rate, config.sample_rate);
    assert_eq!(output_config.channels, config.channels);
    assert_eq!(output_config.sample_format, config.sample_format);
}

#[test]
fn ios_system_output_rejects_unsupported_config() {
    let state = Arc::new(Mutex::new(AudioState::default()));

    let err = set_audio_device(
        IOS_SYSTEM_OUTPUT_ID.to_string(),
        false,
        output_config(96_000, 2, "f32"),
        &state,
    )
    .unwrap_err();

    assert!(err.contains("Configuration not supported"));
    assert!(get_audio_config(&state).unwrap().output_config.is_none());
}

#[test]
fn ios_verify_sample_rate_reports_fallback_rate() {
    assert_eq!(
        verify_working_sample_rate(Some(IOS_SYSTEM_OUTPUT_ID), 96_000, 2),
        Some(48_000)
    );
    assert_eq!(
        verify_working_sample_rate(Some(IOS_SYSTEM_OUTPUT_ID), 44_100, 2),
        Some(44_100)
    );
    assert_eq!(
        verify_working_sample_rate(Some(IOS_SYSTEM_OUTPUT_ID), 48_000, 8),
        None
    );
}

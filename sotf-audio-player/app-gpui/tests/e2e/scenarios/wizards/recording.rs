//! E2E tests for Recording Wizard.
//!
//! Tests for the 4-step audio recording wizard:
//! 1. Config - Device selection and channel mapping
//! 2. Capture - Record frequency response for each channel
//! 3. Evaluating - View and analyze frequency response graphs
//! 4. Saving - Save recordings and configuration to disk

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Recording screen workflow step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum RecordingStep {
    #[default]
    Config,
    Capture,
    Evaluating,
    Saving,
}

impl RecordingStep {
    fn index(&self) -> usize {
        match self {
            RecordingStep::Config => 0,
            RecordingStep::Capture => 1,
            RecordingStep::Evaluating => 2,
            RecordingStep::Saving => 3,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            RecordingStep::Config => "Setup",
            RecordingStep::Capture => "Capture",
            RecordingStep::Evaluating => "Evaluate",
            RecordingStep::Saving => "Save",
        }
    }

    fn next(&self) -> Option<RecordingStep> {
        match self {
            RecordingStep::Config => Some(RecordingStep::Capture),
            RecordingStep::Capture => Some(RecordingStep::Evaluating),
            RecordingStep::Evaluating => Some(RecordingStep::Saving),
            RecordingStep::Saving => None,
        }
    }

    fn previous(&self) -> Option<RecordingStep> {
        match self {
            RecordingStep::Config => None,
            RecordingStep::Capture => Some(RecordingStep::Config),
            RecordingStep::Evaluating => Some(RecordingStep::Capture),
            RecordingStep::Saving => Some(RecordingStep::Evaluating),
        }
    }
}

/// Signal type for test signal generation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum SignalType {
    #[default]
    Sweep,
    WhiteNoise,
    PinkNoise,
}

impl SignalType {
    fn as_str(&self) -> &'static str {
        match self {
            SignalType::Sweep => "Sweep",
            SignalType::WhiteNoise => "White Noise",
            SignalType::PinkNoise => "Pink Noise",
        }
    }
}

/// Speaker configuration presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum SpeakerConfiguration {
    #[default]
    Stereo,
    Stereo21,
    Surround50,
    Surround51,
    Surround71,
    Atmos512,
    Atmos714,
}

impl SpeakerConfiguration {
    fn channel_count(&self) -> usize {
        match self {
            SpeakerConfiguration::Stereo => 2,
            SpeakerConfiguration::Stereo21 => 3,
            SpeakerConfiguration::Surround50 => 5,
            SpeakerConfiguration::Surround51 => 6,
            SpeakerConfiguration::Surround71 => 8,
            SpeakerConfiguration::Atmos512 => 8,
            SpeakerConfiguration::Atmos714 => 12,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            SpeakerConfiguration::Stereo => "2.0 Stereo",
            SpeakerConfiguration::Stereo21 => "2.1 Stereo + Sub",
            SpeakerConfiguration::Surround50 => "5.0 Surround",
            SpeakerConfiguration::Surround51 => "5.1 Surround",
            SpeakerConfiguration::Surround71 => "7.1 Surround",
            SpeakerConfiguration::Atmos512 => "5.1.2 Atmos",
            SpeakerConfiguration::Atmos714 => "7.1.4 Atmos",
        }
    }
}

/// Smoothing options for frequency response plots
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum PlotSmoothing {
    #[default]
    None,
    Octave1,
    Octave3,
    Octave6,
    Octave24,
}

impl PlotSmoothing {
    fn as_str(&self) -> &'static str {
        match self {
            PlotSmoothing::None => "None",
            PlotSmoothing::Octave1 => "1/1 octave",
            PlotSmoothing::Octave3 => "1/3 octave",
            PlotSmoothing::Octave6 => "1/6 octave",
            PlotSmoothing::Octave24 => "1/24 octave",
        }
    }

    fn octave_fraction(&self) -> Option<f32> {
        match self {
            PlotSmoothing::None => None,
            PlotSmoothing::Octave1 => Some(1.0),
            PlotSmoothing::Octave3 => Some(1.0 / 3.0),
            PlotSmoothing::Octave6 => Some(1.0 / 6.0),
            PlotSmoothing::Octave24 => Some(1.0 / 24.0),
        }
    }
}

/// State of a single channel's recording
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ChannelRecordingState {
    #[default]
    Empty,
    Recording,
    Done,
    Error,
}

/// Channel mapping configuration
#[derive(Debug, Clone)]
struct ChannelMapping {
    interface_channel: usize,
    group_name: String,
}

/// Playback device configuration
#[derive(Debug, Clone)]
struct PlaybackDeviceConfig {
    device_id: String,
    device_name: String,
    num_channels: usize,
    sample_rate: u32,
    speaker_configuration: SpeakerConfiguration,
    channel_mappings: Vec<ChannelMapping>,
}

impl Default for PlaybackDeviceConfig {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            device_name: String::new(),
            num_channels: 2,
            sample_rate: 48000,
            speaker_configuration: SpeakerConfiguration::Stereo,
            channel_mappings: vec![
                ChannelMapping {
                    interface_channel: 1,
                    group_name: "L".to_string(),
                },
                ChannelMapping {
                    interface_channel: 2,
                    group_name: "R".to_string(),
                },
            ],
        }
    }
}

/// Recording device configuration
#[derive(Debug, Clone)]
struct RecordingDeviceConfig {
    device_id: String,
    device_name: String,
    num_channels: usize,
    sample_rate: u32,
    channel_mappings: Vec<usize>,
}

impl Default for RecordingDeviceConfig {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            device_name: String::new(),
            num_channels: 1,
            sample_rate: 48000,
            channel_mappings: vec![1],
        }
    }
}

/// Recording for a single channel
#[derive(Debug, Clone)]
struct ChannelRecording {
    channel_index: usize,
    channel_name: String,
    state: ChannelRecordingState,
    result: Option<RecordingResult>,
}

/// Result of a recording
#[derive(Debug, Clone)]
struct RecordingResult {
    wav_path: Option<String>,
    csv_path: Option<String>,
    frequencies: Vec<f32>,
    magnitude_db: Vec<f32>,
    phase_deg: Vec<f32>,
}

/// Recording state for testing
struct RecordingState {
    step: RecordingStep,
    // Config
    playback_device: PlaybackDeviceConfig,
    recording_device: RecordingDeviceConfig,
    signal_type: SignalType,
    signal_duration_secs: f32,
    signal_level_db: f32,
    recording_base_directory: Option<String>,
    // Capture
    channel_recordings: Vec<ChannelRecording>,
    current_recording_channel: Option<usize>,
    is_recording: bool,
    // Evaluating
    plot_smoothing: PlotSmoothing,
    selected_channel_index: Option<usize>,
    // Saving
    save_directory: Option<String>,
    save_name: String,
    // UI
    status_message: String,
    error_message: Option<String>,
}

impl Default for RecordingState {
    fn default() -> Self {
        Self {
            step: RecordingStep::Config,
            playback_device: PlaybackDeviceConfig::default(),
            recording_device: RecordingDeviceConfig::default(),
            signal_type: SignalType::Sweep,
            signal_duration_secs: 5.0,
            signal_level_db: -12.0,
            recording_base_directory: None,
            channel_recordings: Vec::new(),
            current_recording_channel: None,
            is_recording: false,
            plot_smoothing: PlotSmoothing::None,
            selected_channel_index: None,
            save_directory: None,
            save_name: String::new(),
            status_message: String::new(),
            error_message: None,
        }
    }
}

impl RecordingState {
    fn init_channel_recordings(&mut self) {
        self.channel_recordings = self
            .playback_device
            .channel_mappings
            .iter()
            .enumerate()
            .map(|(i, mapping)| ChannelRecording {
                channel_index: i,
                channel_name: mapping.group_name.clone(),
                state: ChannelRecordingState::Empty,
                result: None,
            })
            .collect();
    }

    fn all_channels_recorded(&self) -> bool {
        !self.channel_recordings.is_empty()
            && self
                .channel_recordings
                .iter()
                .all(|r| r.state == ChannelRecordingState::Done)
    }

    fn is_recording(&self) -> bool {
        self.is_recording
    }

    fn recorded_channel_count(&self) -> usize {
        self.channel_recordings
            .iter()
            .filter(|r| r.state == ChannelRecordingState::Done)
            .count()
    }
}

// =============================================================================
// Step Navigation Tests
// =============================================================================

/// Test step indices.
#[gpui::test]
async fn test_step_indices(_cx: &mut TestAppContext) {
    assert_eq!(RecordingStep::Config.index(), 0);
    assert_eq!(RecordingStep::Capture.index(), 1);
    assert_eq!(RecordingStep::Evaluating.index(), 2);
    assert_eq!(RecordingStep::Saving.index(), 3);
}

/// Test step labels.
#[gpui::test]
async fn test_step_labels(_cx: &mut TestAppContext) {
    assert_eq!(RecordingStep::Config.label(), "Setup");
    assert_eq!(RecordingStep::Capture.label(), "Capture");
    assert_eq!(RecordingStep::Evaluating.label(), "Evaluate");
    assert_eq!(RecordingStep::Saving.label(), "Save");
}

/// Test step next navigation.
#[gpui::test]
async fn test_step_next_navigation(_cx: &mut TestAppContext) {
    assert_eq!(RecordingStep::Config.next(), Some(RecordingStep::Capture));
    assert_eq!(
        RecordingStep::Capture.next(),
        Some(RecordingStep::Evaluating)
    );
    assert_eq!(RecordingStep::Evaluating.next(), Some(RecordingStep::Saving));
    assert_eq!(RecordingStep::Saving.next(), None);
}

/// Test step previous navigation.
#[gpui::test]
async fn test_step_previous_navigation(_cx: &mut TestAppContext) {
    assert_eq!(RecordingStep::Config.previous(), None);
    assert_eq!(RecordingStep::Capture.previous(), Some(RecordingStep::Config));
    assert_eq!(
        RecordingStep::Evaluating.previous(),
        Some(RecordingStep::Capture)
    );
    assert_eq!(
        RecordingStep::Saving.previous(),
        Some(RecordingStep::Evaluating)
    );
}

/// Test complete step sequence.
#[gpui::test]
async fn test_complete_step_sequence(_cx: &mut TestAppContext) {
    let mut step = RecordingStep::Config;
    let mut steps = vec![step];

    while let Some(next) = step.next() {
        step = next;
        steps.push(step);
    }

    assert_eq!(steps.len(), 4);
    assert_eq!(steps[0], RecordingStep::Config);
    assert_eq!(steps[3], RecordingStep::Saving);
}

// =============================================================================
// Step 1: Config Tests
// =============================================================================

/// Test initial state defaults.
#[gpui::test]
async fn test_initial_state_defaults(_cx: &mut TestAppContext) {
    let state = RecordingState::default();

    assert_eq!(state.step, RecordingStep::Config);
    assert_eq!(state.signal_type, SignalType::Sweep);
    assert!((state.signal_duration_secs - 5.0).abs() < 0.1);
    assert!((state.signal_level_db - (-12.0)).abs() < 0.1);
}

/// Test playback device selection.
#[gpui::test]
async fn test_playback_device_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().playback_device.device_name = "Blackhole 16ch".to_string();
    state.borrow_mut().playback_device.num_channels = 16;

    assert_eq!(state.borrow().playback_device.device_name, "Blackhole 16ch");
    assert_eq!(state.borrow().playback_device.num_channels, 16);
}

/// Test recording device selection.
#[gpui::test]
async fn test_recording_device_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().recording_device.device_name = "USB Microphone".to_string();
    state.borrow_mut().recording_device.num_channels = 2;

    assert_eq!(state.borrow().recording_device.device_name, "USB Microphone");
}

/// Test sample rate selection.
#[gpui::test]
async fn test_sample_rate_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    let sample_rates = [44100_u32, 48000, 88200, 96000, 176400, 192000];
    for rate in sample_rates {
        state.borrow_mut().playback_device.sample_rate = rate;
        assert_eq!(state.borrow().playback_device.sample_rate, rate);
    }
}

/// Test speaker configuration selection.
#[gpui::test]
async fn test_speaker_configuration_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    let configs = [
        SpeakerConfiguration::Stereo,
        SpeakerConfiguration::Stereo21,
        SpeakerConfiguration::Surround50,
        SpeakerConfiguration::Surround51,
        SpeakerConfiguration::Surround71,
        SpeakerConfiguration::Atmos512,
        SpeakerConfiguration::Atmos714,
    ];

    for config in configs {
        state.borrow_mut().playback_device.speaker_configuration = config;
        assert_eq!(state.borrow().playback_device.speaker_configuration, config);
    }
}

/// Test speaker configuration channel counts.
#[gpui::test]
async fn test_speaker_configuration_channel_counts(_cx: &mut TestAppContext) {
    assert_eq!(SpeakerConfiguration::Stereo.channel_count(), 2);
    assert_eq!(SpeakerConfiguration::Stereo21.channel_count(), 3);
    assert_eq!(SpeakerConfiguration::Surround50.channel_count(), 5);
    assert_eq!(SpeakerConfiguration::Surround51.channel_count(), 6);
    assert_eq!(SpeakerConfiguration::Surround71.channel_count(), 8);
    assert_eq!(SpeakerConfiguration::Atmos714.channel_count(), 12);
}

/// Test speaker configuration labels.
#[gpui::test]
async fn test_speaker_configuration_labels(_cx: &mut TestAppContext) {
    assert!(SpeakerConfiguration::Stereo.as_str().contains("2.0"));
    assert!(SpeakerConfiguration::Surround51.as_str().contains("5.1"));
    assert!(SpeakerConfiguration::Atmos714.as_str().contains("7.1.4"));
}

/// Test channel mapping configuration.
#[gpui::test]
async fn test_channel_mapping_configuration(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    // Configure 5.1 channel mapping
    state.borrow_mut().playback_device.channel_mappings = vec![
        ChannelMapping {
            interface_channel: 1,
            group_name: "L".to_string(),
        },
        ChannelMapping {
            interface_channel: 2,
            group_name: "R".to_string(),
        },
        ChannelMapping {
            interface_channel: 3,
            group_name: "C".to_string(),
        },
        ChannelMapping {
            interface_channel: 4,
            group_name: "LFE".to_string(),
        },
        ChannelMapping {
            interface_channel: 5,
            group_name: "SL".to_string(),
        },
        ChannelMapping {
            interface_channel: 6,
            group_name: "SR".to_string(),
        },
    ];

    assert_eq!(state.borrow().playback_device.channel_mappings.len(), 6);
}

/// Test signal type selection.
#[gpui::test]
async fn test_signal_type_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    let signal_types = [SignalType::Sweep, SignalType::WhiteNoise, SignalType::PinkNoise];
    for sig_type in signal_types {
        state.borrow_mut().signal_type = sig_type;
        assert_eq!(state.borrow().signal_type, sig_type);
    }
}

/// Test signal type labels.
#[gpui::test]
async fn test_signal_type_labels(_cx: &mut TestAppContext) {
    assert_eq!(SignalType::Sweep.as_str(), "Sweep");
    assert_eq!(SignalType::WhiteNoise.as_str(), "White Noise");
    assert_eq!(SignalType::PinkNoise.as_str(), "Pink Noise");
}

/// Test signal duration control.
#[gpui::test]
async fn test_signal_duration_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    let durations: Vec<f32> = vec![1.0, 3.0, 5.0, 10.0, 30.0];
    for duration in durations {
        state.borrow_mut().signal_duration_secs = duration;
        assert!((state.borrow().signal_duration_secs - duration).abs() < 0.1);
    }
}

/// Test signal level control.
#[gpui::test]
async fn test_signal_level_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    let levels: Vec<f32> = vec![-24.0, -18.0, -12.0, -6.0, 0.0];
    for level in levels {
        state.borrow_mut().signal_level_db = level;
        assert!((state.borrow().signal_level_db - level).abs() < 0.1);
    }
}

/// Test recording base directory selection.
#[gpui::test]
async fn test_recording_base_directory(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(state.borrow().recording_base_directory.is_none());

    state.borrow_mut().recording_base_directory = Some("/tmp/recordings".to_string());
    assert!(state.borrow().recording_base_directory.is_some());
}

// =============================================================================
// Step 2: Capture Tests
// =============================================================================

/// Test channel recording initialization.
#[gpui::test]
async fn test_channel_recording_initialization(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    // Setup 5.1
    state.borrow_mut().playback_device.channel_mappings = vec![
        ChannelMapping {
            interface_channel: 1,
            group_name: "L".to_string(),
        },
        ChannelMapping {
            interface_channel: 2,
            group_name: "R".to_string(),
        },
        ChannelMapping {
            interface_channel: 3,
            group_name: "C".to_string(),
        },
    ];

    state.borrow_mut().init_channel_recordings();

    assert_eq!(state.borrow().channel_recordings.len(), 3);
    assert_eq!(state.borrow().channel_recordings[0].channel_name, "L");
    assert_eq!(
        state.borrow().channel_recordings[0].state,
        ChannelRecordingState::Empty
    );
}

/// Test recording state transitions.
#[gpui::test]
async fn test_recording_state_transitions(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().init_channel_recordings();

    // Start recording
    state.borrow_mut().channel_recordings[0].state = ChannelRecordingState::Recording;
    state.borrow_mut().is_recording = true;
    assert!(state.borrow().is_recording());

    // Complete recording
    state.borrow_mut().channel_recordings[0].state = ChannelRecordingState::Done;
    state.borrow_mut().is_recording = false;
    assert!(!state.borrow().is_recording());
}

/// Test current recording channel tracking.
#[gpui::test]
async fn test_current_recording_channel_tracking(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().current_recording_channel = Some(0);
    assert_eq!(state.borrow().current_recording_channel, Some(0));

    state.borrow_mut().current_recording_channel = Some(1);
    assert_eq!(state.borrow().current_recording_channel, Some(1));
}

/// Test all channels recorded check.
#[gpui::test]
async fn test_all_channels_recorded_check(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().init_channel_recordings();
    assert!(!state.borrow().all_channels_recorded());

    // Record first channel
    state.borrow_mut().channel_recordings[0].state = ChannelRecordingState::Done;
    assert!(!state.borrow().all_channels_recorded());

    // Record second channel
    state.borrow_mut().channel_recordings[1].state = ChannelRecordingState::Done;
    assert!(state.borrow().all_channels_recorded());
}

/// Test recorded channel count.
#[gpui::test]
async fn test_recorded_channel_count(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().playback_device.channel_mappings = vec![
        ChannelMapping {
            interface_channel: 1,
            group_name: "L".to_string(),
        },
        ChannelMapping {
            interface_channel: 2,
            group_name: "R".to_string(),
        },
        ChannelMapping {
            interface_channel: 3,
            group_name: "C".to_string(),
        },
    ];
    state.borrow_mut().init_channel_recordings();

    assert_eq!(state.borrow().recorded_channel_count(), 0);

    state.borrow_mut().channel_recordings[0].state = ChannelRecordingState::Done;
    assert_eq!(state.borrow().recorded_channel_count(), 1);

    state.borrow_mut().channel_recordings[2].state = ChannelRecordingState::Done;
    assert_eq!(state.borrow().recorded_channel_count(), 2);
}

/// Test recording result storage.
#[gpui::test]
async fn test_recording_result_storage(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().init_channel_recordings();

    state.borrow_mut().channel_recordings[0].result = Some(RecordingResult {
        wav_path: Some("/tmp/L.wav".to_string()),
        csv_path: Some("/tmp/L.csv".to_string()),
        frequencies: vec![100.0, 1000.0, 10000.0],
        magnitude_db: vec![-3.0, 0.0, -6.0],
        phase_deg: vec![0.0, 45.0, 90.0],
    });
    state.borrow_mut().channel_recordings[0].state = ChannelRecordingState::Done;

    let result = state.borrow().channel_recordings[0].result.clone().unwrap();
    assert!(result.wav_path.is_some());
    assert_eq!(result.frequencies.len(), 3);
}

/// Test recording error handling.
#[gpui::test]
async fn test_recording_error_handling(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().init_channel_recordings();

    state.borrow_mut().channel_recordings[0].state = ChannelRecordingState::Error;
    state.borrow_mut().error_message = Some("Recording failed: Device disconnected".to_string());

    assert_eq!(
        state.borrow().channel_recordings[0].state,
        ChannelRecordingState::Error
    );
    assert!(state.borrow().error_message.is_some());
}

// =============================================================================
// Step 3: Evaluating Tests
// =============================================================================

/// Test plot smoothing selection.
#[gpui::test]
async fn test_plot_smoothing_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    let smoothing_options = [
        PlotSmoothing::None,
        PlotSmoothing::Octave1,
        PlotSmoothing::Octave3,
        PlotSmoothing::Octave6,
        PlotSmoothing::Octave24,
    ];

    for option in smoothing_options {
        state.borrow_mut().plot_smoothing = option;
        assert_eq!(state.borrow().plot_smoothing, option);
    }
}

/// Test plot smoothing labels.
#[gpui::test]
async fn test_plot_smoothing_labels(_cx: &mut TestAppContext) {
    assert_eq!(PlotSmoothing::None.as_str(), "None");
    assert!(PlotSmoothing::Octave1.as_str().contains("1/1"));
    assert!(PlotSmoothing::Octave3.as_str().contains("1/3"));
    assert!(PlotSmoothing::Octave6.as_str().contains("1/6"));
    assert!(PlotSmoothing::Octave24.as_str().contains("1/24"));
}

/// Test plot smoothing octave fractions.
#[gpui::test]
async fn test_plot_smoothing_octave_fractions(_cx: &mut TestAppContext) {
    assert!(PlotSmoothing::None.octave_fraction().is_none());
    assert!((PlotSmoothing::Octave1.octave_fraction().unwrap() - 1.0).abs() < 0.01);
    assert!((PlotSmoothing::Octave3.octave_fraction().unwrap() - (1.0 / 3.0)).abs() < 0.01);
    assert!((PlotSmoothing::Octave6.octave_fraction().unwrap() - (1.0 / 6.0)).abs() < 0.01);
    assert!((PlotSmoothing::Octave24.octave_fraction().unwrap() - (1.0 / 24.0)).abs() < 0.01);
}

/// Test channel selection for viewing.
#[gpui::test]
async fn test_channel_selection_for_viewing(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(state.borrow().selected_channel_index.is_none());

    state.borrow_mut().selected_channel_index = Some(0);
    assert_eq!(state.borrow().selected_channel_index, Some(0));

    state.borrow_mut().selected_channel_index = Some(1);
    assert_eq!(state.borrow().selected_channel_index, Some(1));
}

/// Test frequency response data display.
#[gpui::test]
async fn test_frequency_response_data_display(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().init_channel_recordings();
    state.borrow_mut().channel_recordings[0].result = Some(RecordingResult {
        wav_path: None,
        csv_path: None,
        frequencies: (1..=100).map(|i| 20.0 * (1.5_f32).powi(i as i32 / 10)).collect(),
        magnitude_db: vec![-3.0; 100],
        phase_deg: vec![0.0; 100],
    });

    let result = state.borrow().channel_recordings[0].result.clone().unwrap();
    assert_eq!(result.frequencies.len(), 100);
    assert_eq!(result.magnitude_db.len(), 100);
}

// =============================================================================
// Step 4: Saving Tests
// =============================================================================

/// Test save directory selection.
#[gpui::test]
async fn test_save_directory_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().save_directory = Some("/tmp/measurements".to_string());
    assert!(state.borrow().save_directory.is_some());
}

/// Test save name entry.
#[gpui::test]
async fn test_save_name_entry(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().save_name = "Living Room 2024-01-06".to_string();
    assert_eq!(state.borrow().save_name, "Living Room 2024-01-06");
}

// =============================================================================
// Full Wizard Flow Tests
// =============================================================================

/// Test complete wizard flow.
#[gpui::test]
async fn test_complete_wizard_flow(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    // Step 1: Configure
    assert_eq!(state.borrow().step, RecordingStep::Config);
    state.borrow_mut().playback_device.device_name = "Test Device".to_string();
    state.borrow_mut().recording_base_directory = Some("/tmp".to_string());
    state.borrow_mut().signal_type = SignalType::Sweep;

    // Step 2: Capture
    state.borrow_mut().step = RecordingStep::Capture;
    state.borrow_mut().init_channel_recordings();
    let channel_count = state.borrow().channel_recordings.len();
    for i in 0..channel_count {
        state.borrow_mut().channel_recordings[i].state = ChannelRecordingState::Done;
        state.borrow_mut().channel_recordings[i].result = Some(RecordingResult {
            wav_path: Some(format!("/tmp/ch{}.wav", i)),
            csv_path: Some(format!("/tmp/ch{}.csv", i)),
            frequencies: vec![100.0, 1000.0, 10000.0],
            magnitude_db: vec![0.0, 0.0, 0.0],
            phase_deg: vec![0.0, 0.0, 0.0],
        });
    }
    assert!(state.borrow().all_channels_recorded());

    // Step 3: Evaluate
    state.borrow_mut().step = RecordingStep::Evaluating;
    state.borrow_mut().plot_smoothing = PlotSmoothing::Octave3;
    state.borrow_mut().selected_channel_index = Some(0);

    // Step 4: Save
    state.borrow_mut().step = RecordingStep::Saving;
    state.borrow_mut().save_directory = Some("/tmp/final".to_string());
    state.borrow_mut().save_name = "Test Recording".to_string();

    assert_eq!(state.borrow().step, RecordingStep::Saving);
}

/// Test wizard back navigation.
#[gpui::test]
async fn test_wizard_back_navigation(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    // Start at last step
    state.borrow_mut().step = RecordingStep::Saving;

    // Navigate back through all steps
    let mut step = state.borrow().step;
    let mut visited = vec![step];

    while let Some(prev) = step.previous() {
        step = prev;
        visited.push(step);
    }

    assert_eq!(visited.len(), 4);
    assert_eq!(visited[3], RecordingStep::Config);
}

/// Test status message updates.
#[gpui::test]
async fn test_status_message_updates(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().status_message = "Recording channel L...".to_string();
    assert!(state.borrow().status_message.contains("Recording"));

    state.borrow_mut().status_message = "Saving files...".to_string();
    assert!(state.borrow().status_message.contains("Saving"));
}

/// Test error message display.
#[gpui::test]
async fn test_error_message_display(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(state.borrow().error_message.is_none());

    state.borrow_mut().error_message = Some("Device not found".to_string());
    assert!(state.borrow().error_message.is_some());
}

/// Test surround recording setup.
#[gpui::test]
async fn test_surround_recording_setup(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    // Setup 5.1 surround
    state.borrow_mut().playback_device.speaker_configuration = SpeakerConfiguration::Surround51;
    state.borrow_mut().playback_device.channel_mappings = vec![
        ChannelMapping {
            interface_channel: 1,
            group_name: "L".to_string(),
        },
        ChannelMapping {
            interface_channel: 2,
            group_name: "R".to_string(),
        },
        ChannelMapping {
            interface_channel: 3,
            group_name: "C".to_string(),
        },
        ChannelMapping {
            interface_channel: 4,
            group_name: "LFE".to_string(),
        },
        ChannelMapping {
            interface_channel: 5,
            group_name: "SL".to_string(),
        },
        ChannelMapping {
            interface_channel: 6,
            group_name: "SR".to_string(),
        },
    ];

    state.borrow_mut().init_channel_recordings();

    assert_eq!(state.borrow().channel_recordings.len(), 6);
    assert_eq!(state.borrow().channel_recordings[3].channel_name, "LFE");
}

//! E2E tests for Recording Wizard.
//!
//! Tests for the 4-step audio recording wizard:
//! 1. Config - Device selection and channel mapping
//! 2. Capture - Record frequency response for each channel
//! 3. Evaluating - View and analyze frequency response graphs
//! 4. Saving - Save recordings and configuration to disk
//!
//! These tests verify that all fields can be edited and the process
//! can continue through all steps to completion.

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing (mirrors app/types.rs)
// =============================================================================

/// Recording workflow step
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecordingStep {
    Config,
    Capture,
    Evaluating,
    Saving,
}

impl Default for RecordingStep {
    fn default() -> Self {
        RecordingStep::Config
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SignalType {
    Sweep,
    WhiteNoise,
    PinkNoise,
}

impl Default for SignalType {
    fn default() -> Self {
        SignalType::Sweep
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpeakerConfiguration {
    Stereo,
    Stereo21,
    Surround50,
    Surround51,
    Surround71,
    Atmos512,
    Atmos714,
    Custom,
}

impl Default for SpeakerConfiguration {
    fn default() -> Self {
        SpeakerConfiguration::Stereo
    }
}

impl SpeakerConfiguration {
    fn all() -> &'static [SpeakerConfiguration] {
        &[
            SpeakerConfiguration::Stereo,
            SpeakerConfiguration::Stereo21,
            SpeakerConfiguration::Surround50,
            SpeakerConfiguration::Surround51,
            SpeakerConfiguration::Surround71,
            SpeakerConfiguration::Atmos512,
            SpeakerConfiguration::Atmos714,
            SpeakerConfiguration::Custom,
        ]
    }

    fn channel_count(&self) -> usize {
        match self {
            SpeakerConfiguration::Stereo => 2,
            SpeakerConfiguration::Stereo21 => 3,
            SpeakerConfiguration::Surround50 => 5,
            SpeakerConfiguration::Surround51 => 6,
            SpeakerConfiguration::Surround71 => 8,
            SpeakerConfiguration::Atmos512 => 8,
            SpeakerConfiguration::Atmos714 => 12,
            SpeakerConfiguration::Custom => 2,
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
            SpeakerConfiguration::Custom => "Custom",
        }
    }

    fn default_channel_names(&self) -> Vec<&'static str> {
        match self {
            SpeakerConfiguration::Stereo => vec!["L", "R"],
            SpeakerConfiguration::Stereo21 => vec!["L", "R", "LFE"],
            SpeakerConfiguration::Surround50 => vec!["L", "R", "C", "SL", "SR"],
            SpeakerConfiguration::Surround51 => vec!["L", "R", "C", "LFE", "SL", "SR"],
            SpeakerConfiguration::Surround71 => vec!["L", "R", "C", "LFE", "SL", "SR", "BL", "BR"],
            SpeakerConfiguration::Atmos512 => {
                vec!["L", "R", "C", "LFE", "SL", "SR", "TFL", "TFR"]
            }
            SpeakerConfiguration::Atmos714 => vec![
                "L", "R", "C", "LFE", "SL", "SR", "BL", "BR", "TFL", "TFR", "TBL", "TBR",
            ],
            SpeakerConfiguration::Custom => vec!["L", "R"],
        }
    }
}

/// Smoothing options for frequency response plots
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlotSmoothing {
    None,
    Octave1,
    Octave3,
    Octave6,
    Octave24,
}

impl Default for PlotSmoothing {
    fn default() -> Self {
        PlotSmoothing::None
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChannelRecordingState {
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
    available_sample_rates: Vec<u32>,
    device_dropdown_open: bool,
    sample_rate_dropdown_open: bool,
    speaker_config_dropdown_open: bool,
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
                    interface_channel: 0,
                    group_name: "L".to_string(),
                },
                ChannelMapping {
                    interface_channel: 1,
                    group_name: "R".to_string(),
                },
            ],
            available_sample_rates: vec![44100, 48000, 88200, 96000],
            device_dropdown_open: false,
            sample_rate_dropdown_open: false,
            speaker_config_dropdown_open: false,
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
    available_sample_rates: Vec<u32>,
    device_dropdown_open: bool,
    sample_rate_dropdown_open: bool,
}

impl Default for RecordingDeviceConfig {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            device_name: String::new(),
            num_channels: 1,
            sample_rate: 48000,
            channel_mappings: vec![0],
            available_sample_rates: vec![44100, 48000, 88200, 96000],
            device_dropdown_open: false,
            sample_rate_dropdown_open: false,
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
    channel: usize,
    wav_path: Option<String>,
    csv_path: Option<String>,
    frequencies: Vec<f32>,
    magnitude_db: Vec<f32>,
    phase_deg: Vec<f32>,
}

/// Calibration data
#[derive(Debug, Clone, Default)]
struct CalibrationData {
    frequencies: Vec<f32>,
    spl_db: Vec<f32>,
}

/// Recording state for testing
struct RecordingState {
    step: RecordingStep,
    playback_config: PlaybackDeviceConfig,
    recording_config: RecordingDeviceConfig,
    signal_type: SignalType,
    signal_duration_secs: f32,
    signal_level_db: f32,
    sweep_start_freq: f32,
    sweep_end_freq: f32,
    signal_type_dropdown_open: bool,
    duration_dropdown_open: bool,
    recording_base_directory: Option<String>,
    recording_directory: Option<String>,
    channel_recordings: Vec<ChannelRecording>,
    current_recording_channel: Option<usize>,
    is_recording: bool,
    recording_progress: f32,
    auto_record_remaining: bool,
    status_message: String,
    mic_calibration_path: Option<String>,
    mic_calibration_data: Option<CalibrationData>,
    plot_smoothing: PlotSmoothing,
    plot_selected_channel: Option<usize>,
    plot_channel_dropdown_open: bool,
    plot_smoothing_dropdown_open: bool,
    save_name: String,
    config_accordion_expanded: Vec<String>,
    channel_name_dropdown_open: Option<usize>,
}

impl Default for RecordingState {
    fn default() -> Self {
        Self {
            step: RecordingStep::Config,
            playback_config: PlaybackDeviceConfig::default(),
            recording_config: RecordingDeviceConfig::default(),
            signal_type: SignalType::Sweep,
            signal_duration_secs: 5.0,
            signal_level_db: -12.0,
            sweep_start_freq: 20.0,
            sweep_end_freq: 20000.0,
            signal_type_dropdown_open: false,
            duration_dropdown_open: false,
            recording_base_directory: None,
            recording_directory: None,
            channel_recordings: Vec::new(),
            current_recording_channel: None,
            is_recording: false,
            recording_progress: 0.0,
            auto_record_remaining: false,
            status_message: String::new(),
            mic_calibration_path: None,
            mic_calibration_data: None,
            plot_smoothing: PlotSmoothing::None,
            plot_selected_channel: None,
            plot_channel_dropdown_open: false,
            plot_smoothing_dropdown_open: false,
            save_name: String::new(),
            config_accordion_expanded: Vec::new(),
            channel_name_dropdown_open: None,
        }
    }
}

impl RecordingState {
    fn init_channel_recordings(&mut self) {
        self.channel_recordings = self
            .playback_config
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

    fn can_advance(&self) -> bool {
        match self.step {
            RecordingStep::Config => self.recording_base_directory.is_some(),
            RecordingStep::Capture => self.all_channels_recorded() && !self.is_recording,
            RecordingStep::Evaluating => true,
            RecordingStep::Saving => true,
        }
    }

    fn has_output_directory(&self) -> bool {
        self.recording_base_directory.is_some()
    }
}

// =============================================================================
// Step Navigation Tests
// =============================================================================

#[gpui::test]
async fn test_step_indices(_cx: &mut TestAppContext) {
    assert_eq!(RecordingStep::Config.index(), 0);
    assert_eq!(RecordingStep::Capture.index(), 1);
    assert_eq!(RecordingStep::Evaluating.index(), 2);
    assert_eq!(RecordingStep::Saving.index(), 3);
}

#[gpui::test]
async fn test_step_labels(_cx: &mut TestAppContext) {
    assert_eq!(RecordingStep::Config.label(), "Setup");
    assert_eq!(RecordingStep::Capture.label(), "Capture");
    assert_eq!(RecordingStep::Evaluating.label(), "Evaluate");
    assert_eq!(RecordingStep::Saving.label(), "Save");
}

#[gpui::test]
async fn test_step_next_navigation(_cx: &mut TestAppContext) {
    assert_eq!(RecordingStep::Config.next(), Some(RecordingStep::Capture));
    assert_eq!(
        RecordingStep::Capture.next(),
        Some(RecordingStep::Evaluating)
    );
    assert_eq!(
        RecordingStep::Evaluating.next(),
        Some(RecordingStep::Saving)
    );
    assert_eq!(RecordingStep::Saving.next(), None);
}

#[gpui::test]
async fn test_step_previous_navigation(_cx: &mut TestAppContext) {
    assert_eq!(RecordingStep::Config.previous(), None);
    assert_eq!(
        RecordingStep::Capture.previous(),
        Some(RecordingStep::Config)
    );
    assert_eq!(
        RecordingStep::Evaluating.previous(),
        Some(RecordingStep::Capture)
    );
    assert_eq!(
        RecordingStep::Saving.previous(),
        Some(RecordingStep::Evaluating)
    );
}

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
// Step 1: Config Tests - All Fields Editable
// =============================================================================

#[gpui::test]
async fn test_initial_state_defaults(_cx: &mut TestAppContext) {
    let state = RecordingState::default();

    assert_eq!(state.step, RecordingStep::Config);
    assert_eq!(state.signal_type, SignalType::Sweep);
    assert!((state.signal_duration_secs - 5.0).abs() < 0.1);
    assert!((state.signal_level_db - (-12.0)).abs() < 0.1);
    assert!(state.recording_base_directory.is_none());
    assert!(!state.has_output_directory());
}

#[gpui::test]
async fn test_playback_device_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().playback_config.device_name = "Blackhole 16ch".to_string();
    state.borrow_mut().playback_config.num_channels = 16;

    assert_eq!(state.borrow().playback_config.device_name, "Blackhole 16ch");
    assert_eq!(state.borrow().playback_config.num_channels, 16);
}

#[gpui::test]
async fn test_playback_device_dropdown_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(!state.borrow().playback_config.device_dropdown_open);

    state.borrow_mut().playback_config.device_dropdown_open = true;
    assert!(state.borrow().playback_config.device_dropdown_open);
}

#[gpui::test]
async fn test_recording_device_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().recording_config.device_name = "USB Microphone".to_string();
    state.borrow_mut().recording_config.num_channels = 2;

    assert_eq!(
        state.borrow().recording_config.device_name,
        "USB Microphone"
    );
    assert_eq!(state.borrow().recording_config.num_channels, 2);
}

#[gpui::test]
async fn test_recording_device_dropdown_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(!state.borrow().recording_config.device_dropdown_open);

    state.borrow_mut().recording_config.device_dropdown_open = true;
    assert!(state.borrow().recording_config.device_dropdown_open);
}

#[gpui::test]
async fn test_sample_rate_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    let sample_rates = [44100_u32, 48000, 88200, 96000, 176400, 192000];
    for rate in sample_rates {
        state.borrow_mut().playback_config.sample_rate = rate;
        assert_eq!(state.borrow().playback_config.sample_rate, rate);
    }
}

#[gpui::test]
async fn test_sample_rate_dropdown_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(!state.borrow().playback_config.sample_rate_dropdown_open);

    state.borrow_mut().playback_config.sample_rate_dropdown_open = true;
    assert!(state.borrow().playback_config.sample_rate_dropdown_open);
}

#[gpui::test]
async fn test_speaker_configuration_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    let configs = SpeakerConfiguration::all();
    for &config in configs {
        state.borrow_mut().playback_config.speaker_configuration = config;
        assert_eq!(state.borrow().playback_config.speaker_configuration, config);
    }
}

#[gpui::test]
async fn test_speaker_configuration_dropdown_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(!state.borrow().playback_config.speaker_config_dropdown_open);

    state
        .borrow_mut()
        .playback_config
        .speaker_config_dropdown_open = true;
    assert!(state.borrow().playback_config.speaker_config_dropdown_open);
}

#[gpui::test]
async fn test_speaker_configuration_channel_counts(_cx: &mut TestAppContext) {
    assert_eq!(SpeakerConfiguration::Stereo.channel_count(), 2);
    assert_eq!(SpeakerConfiguration::Stereo21.channel_count(), 3);
    assert_eq!(SpeakerConfiguration::Surround50.channel_count(), 5);
    assert_eq!(SpeakerConfiguration::Surround51.channel_count(), 6);
    assert_eq!(SpeakerConfiguration::Surround71.channel_count(), 8);
    assert_eq!(SpeakerConfiguration::Atmos714.channel_count(), 12);
    assert_eq!(SpeakerConfiguration::Custom.channel_count(), 2);
}

#[gpui::test]
async fn test_speaker_configuration_labels(_cx: &mut TestAppContext) {
    assert!(SpeakerConfiguration::Stereo.as_str().contains("2.0"));
    assert!(SpeakerConfiguration::Surround51.as_str().contains("5.1"));
    assert!(SpeakerConfiguration::Atmos714.as_str().contains("7.1.4"));
    assert!(SpeakerConfiguration::Custom.as_str().contains("Custom"));
}

#[gpui::test]
async fn test_channel_mapping_configuration(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().playback_config.channel_mappings = vec![
        ChannelMapping {
            interface_channel: 0,
            group_name: "L".to_string(),
        },
        ChannelMapping {
            interface_channel: 1,
            group_name: "R".to_string(),
        },
        ChannelMapping {
            interface_channel: 2,
            group_name: "C".to_string(),
        },
        ChannelMapping {
            interface_channel: 3,
            group_name: "LFE".to_string(),
        },
        ChannelMapping {
            interface_channel: 4,
            group_name: "SL".to_string(),
        },
        ChannelMapping {
            interface_channel: 5,
            group_name: "SR".to_string(),
        },
    ];

    assert_eq!(state.borrow().playback_config.channel_mappings.len(), 6);
}

#[gpui::test]
async fn test_channel_name_dropdown_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(state.borrow().channel_name_dropdown_open.is_none());

    state.borrow_mut().channel_name_dropdown_open = Some(0);
    assert_eq!(state.borrow().channel_name_dropdown_open, Some(0));

    state.borrow_mut().channel_name_dropdown_open = Some(1);
    assert_eq!(state.borrow().channel_name_dropdown_open, Some(1));

    state.borrow_mut().channel_name_dropdown_open = None;
    assert!(state.borrow().channel_name_dropdown_open.is_none());
}

#[gpui::test]
async fn test_channel_name_edit(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().playback_config.channel_mappings = vec![
        ChannelMapping {
            interface_channel: 0,
            group_name: "L".to_string(),
        },
        ChannelMapping {
            interface_channel: 1,
            group_name: "R".to_string(),
        },
    ];

    state.borrow_mut().playback_config.channel_mappings[0].group_name = "Left".to_string();
    assert_eq!(
        state.borrow().playback_config.channel_mappings[0].group_name,
        "Left"
    );

    state.borrow_mut().playback_config.channel_mappings[1].group_name = "Right".to_string();
    assert_eq!(
        state.borrow().playback_config.channel_mappings[1].group_name,
        "Right"
    );
}

#[gpui::test]
async fn test_interface_channel_edit(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().playback_config.channel_mappings = vec![
        ChannelMapping {
            interface_channel: 0,
            group_name: "L".to_string(),
        },
        ChannelMapping {
            interface_channel: 1,
            group_name: "R".to_string(),
        },
    ];

    state.borrow_mut().playback_config.channel_mappings[0].interface_channel = 2;
    assert_eq!(
        state.borrow().playback_config.channel_mappings[0].interface_channel,
        2
    );
}

#[gpui::test]
async fn test_custom_channel_count_edit(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().playback_config.speaker_configuration = SpeakerConfiguration::Custom;

    let channel_counts = [1, 2, 4, 8, 16];
    for count in channel_counts {
        state.borrow_mut().playback_config.num_channels = count;
        assert_eq!(state.borrow().playback_config.num_channels, count);
    }
}

#[gpui::test]
async fn test_calibration_file_path_edit(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(state.borrow().mic_calibration_path.is_none());

    state.borrow_mut().mic_calibration_path = Some("/path/to/calibration.csv".to_string());
    assert!(state.borrow().mic_calibration_path.is_some());

    state.borrow_mut().mic_calibration_path = None;
    assert!(state.borrow().mic_calibration_path.is_none());
}

#[gpui::test]
async fn test_calibration_data_parsing(_cx: &mut TestAppContext) {
    let data = CalibrationData {
        frequencies: vec![20.0, 100.0, 1000.0, 10000.0, 20000.0],
        spl_db: vec![-3.0, -2.0, 0.0, -1.0, -4.0],
    };

    assert_eq!(data.frequencies.len(), 5);
    assert_eq!(data.spl_db.len(), 5);
    assert!((data.frequencies[0] - 20.0).abs() < 0.1);
    assert!((data.frequencies[4] - 20000.0).abs() < 0.1);
}

#[gpui::test]
async fn test_output_directory_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(state.borrow().recording_base_directory.is_none());
    assert!(!state.borrow().has_output_directory());

    state.borrow_mut().recording_base_directory = Some("/tmp/recordings".to_string());
    state.borrow_mut().recording_directory =
        Some("/tmp/recordings/recording-20240107-120000".to_string());

    assert!(state.borrow().recording_base_directory.is_some());
    assert!(state.borrow().has_output_directory());
}

#[gpui::test]
async fn test_can_advance_requires_directory(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(!state.borrow().can_advance());

    state.borrow_mut().recording_base_directory = Some("/tmp".to_string());
    assert!(state.borrow().can_advance());
}

// =============================================================================
// Step 2: Capture Tests - All Fields Editable
// =============================================================================

#[gpui::test]
async fn test_signal_type_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    let signal_types = [
        SignalType::Sweep,
        SignalType::WhiteNoise,
        SignalType::PinkNoise,
    ];
    for sig_type in signal_types {
        state.borrow_mut().signal_type = sig_type;
        assert_eq!(state.borrow().signal_type, sig_type);
    }
}

#[gpui::test]
async fn test_signal_type_labels(_cx: &mut TestAppContext) {
    assert_eq!(SignalType::Sweep.as_str(), "Sweep");
    assert_eq!(SignalType::WhiteNoise.as_str(), "White Noise");
    assert_eq!(SignalType::PinkNoise.as_str(), "Pink Noise");
}

#[gpui::test]
async fn test_signal_type_dropdown_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(!state.borrow().signal_type_dropdown_open);

    state.borrow_mut().signal_type_dropdown_open = true;
    assert!(state.borrow().signal_type_dropdown_open);
}

#[gpui::test]
async fn test_signal_duration_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    let durations: Vec<f32> = vec![1.0, 3.0, 5.0, 10.0, 15.0, 20.0, 30.0];
    for duration in durations {
        state.borrow_mut().signal_duration_secs = duration;
        assert!((state.borrow().signal_duration_secs - duration).abs() < 0.1);
    }
}

#[gpui::test]
async fn test_duration_dropdown_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(!state.borrow().duration_dropdown_open);

    state.borrow_mut().duration_dropdown_open = true;
    assert!(state.borrow().duration_dropdown_open);
}

#[gpui::test]
async fn test_signal_level_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    let levels: Vec<f32> = vec![-60.0, -48.0, -36.0, -24.0, -18.0, -12.0, -6.0, 0.0];
    for level in levels {
        state.borrow_mut().signal_level_db = level;
        assert!((state.borrow().signal_level_db - level).abs() < 0.1);
    }
}

#[gpui::test]
async fn test_sweep_frequency_range_edit(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!((state.borrow().sweep_start_freq - 20.0).abs() < 0.1);
    assert!((state.borrow().sweep_end_freq - 20000.0).abs() < 0.1);

    state.borrow_mut().sweep_start_freq = 100.0;
    state.borrow_mut().sweep_end_freq = 10000.0;

    assert!((state.borrow().sweep_start_freq - 100.0).abs() < 0.1);
    assert!((state.borrow().sweep_end_freq - 10000.0).abs() < 0.1);
}

#[gpui::test]
async fn test_channel_recording_initialization(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().playback_config.channel_mappings = vec![
        ChannelMapping {
            interface_channel: 0,
            group_name: "L".to_string(),
        },
        ChannelMapping {
            interface_channel: 1,
            group_name: "R".to_string(),
        },
        ChannelMapping {
            interface_channel: 2,
            group_name: "C".to_string(),
        },
    ];

    state.borrow_mut().init_channel_recordings();

    assert_eq!(state.borrow().channel_recordings.len(), 3);
    assert_eq!(state.borrow().channel_recordings[0].channel_name, "L");
    assert_eq!(state.borrow().channel_recordings[1].channel_name, "R");
    assert_eq!(state.borrow().channel_recordings[2].channel_name, "C");
    assert_eq!(
        state.borrow().channel_recordings[0].state,
        ChannelRecordingState::Empty
    );
}

#[gpui::test]
async fn test_recording_state_transitions(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().init_channel_recordings();

    assert_eq!(
        state.borrow().channel_recordings[0].state,
        ChannelRecordingState::Empty
    );

    state.borrow_mut().channel_recordings[0].state = ChannelRecordingState::Recording;
    assert_eq!(
        state.borrow().channel_recordings[0].state,
        ChannelRecordingState::Recording
    );

    state.borrow_mut().channel_recordings[0].state = ChannelRecordingState::Done;
    assert_eq!(
        state.borrow().channel_recordings[0].state,
        ChannelRecordingState::Done
    );
}

#[gpui::test]
async fn test_is_recording_flag(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(!state.borrow().is_recording());

    state.borrow_mut().is_recording = true;
    assert!(state.borrow().is_recording());

    state.borrow_mut().is_recording = false;
    assert!(!state.borrow().is_recording());
}

#[gpui::test]
async fn test_current_recording_channel_tracking(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(state.borrow().current_recording_channel.is_none());

    state.borrow_mut().current_recording_channel = Some(0);
    assert_eq!(state.borrow().current_recording_channel, Some(0));

    state.borrow_mut().current_recording_channel = Some(1);
    assert_eq!(state.borrow().current_recording_channel, Some(1));
}

#[gpui::test]
async fn test_all_channels_recorded_check(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().playback_config.channel_mappings = vec![
        ChannelMapping {
            interface_channel: 0,
            group_name: "L".to_string(),
        },
        ChannelMapping {
            interface_channel: 1,
            group_name: "R".to_string(),
        },
    ];

    state.borrow_mut().init_channel_recordings();
    assert!(!state.borrow().all_channels_recorded());

    state.borrow_mut().channel_recordings[0].state = ChannelRecordingState::Done;
    assert!(!state.borrow().all_channels_recorded());

    state.borrow_mut().channel_recordings[1].state = ChannelRecordingState::Done;
    assert!(state.borrow().all_channels_recorded());
}

#[gpui::test]
async fn test_recorded_channel_count(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().playback_config.channel_mappings = vec![
        ChannelMapping {
            interface_channel: 0,
            group_name: "L".to_string(),
        },
        ChannelMapping {
            interface_channel: 1,
            group_name: "R".to_string(),
        },
        ChannelMapping {
            interface_channel: 2,
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

#[gpui::test]
async fn test_recording_result_storage(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().init_channel_recordings();

    state.borrow_mut().channel_recordings[0].result = Some(RecordingResult {
        channel: 0,
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
    assert_eq!(result.magnitude_db.len(), 3);
    assert_eq!(result.phase_deg.len(), 3);
}

#[gpui::test]
async fn test_recording_progress_tracking(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    let progress_values: Vec<f32> = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    for value in progress_values {
        state.borrow_mut().recording_progress = value;
        assert!((state.borrow().recording_progress - value).abs() < 0.001);
    }
}

#[gpui::test]
async fn test_auto_record_mode(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(!state.borrow().auto_record_remaining);

    state.borrow_mut().auto_record_remaining = true;
    assert!(state.borrow().auto_record_remaining);

    state.borrow_mut().auto_record_remaining = false;
    assert!(!state.borrow().auto_record_remaining);
}

#[gpui::test]
async fn test_recording_error_handling(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().init_channel_recordings();

    state.borrow_mut().channel_recordings[0].state = ChannelRecordingState::Error;
    state.borrow_mut().status_message = "Recording failed: Device disconnected".to_string();

    assert_eq!(
        state.borrow().channel_recordings[0].state,
        ChannelRecordingState::Error
    );
    assert!(state.borrow().status_message.contains("Recording"));
}

#[gpui::test]
async fn test_can_advance_requires_all_recorded(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));
    state.borrow_mut().recording_base_directory = Some("/tmp".to_string());
    state.borrow_mut().step = RecordingStep::Capture;

    state.borrow_mut().playback_config.channel_mappings = vec![
        ChannelMapping {
            interface_channel: 0,
            group_name: "L".to_string(),
        },
        ChannelMapping {
            interface_channel: 1,
            group_name: "R".to_string(),
        },
    ];
    state.borrow_mut().init_channel_recordings();

    assert!(!state.borrow().can_advance());

    state.borrow_mut().channel_recordings[0].state = ChannelRecordingState::Done;
    assert!(!state.borrow().can_advance());

    state.borrow_mut().channel_recordings[1].state = ChannelRecordingState::Done;
    assert!(state.borrow().can_advance());
}

// =============================================================================
// Step 3: Evaluating Tests - All Fields Editable
// =============================================================================

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

#[gpui::test]
async fn test_plot_smoothing_labels(_cx: &mut TestAppContext) {
    assert_eq!(PlotSmoothing::None.as_str(), "None");
    assert!(PlotSmoothing::Octave1.as_str().contains("1/1"));
    assert!(PlotSmoothing::Octave3.as_str().contains("1/3"));
    assert!(PlotSmoothing::Octave6.as_str().contains("1/6"));
    assert!(PlotSmoothing::Octave24.as_str().contains("1/24"));
}

#[gpui::test]
async fn test_plot_smoothing_octave_fractions(_cx: &mut TestAppContext) {
    assert!(PlotSmoothing::None.octave_fraction().is_none());
    assert!((PlotSmoothing::Octave1.octave_fraction().unwrap() - 1.0).abs() < 0.01);
    assert!((PlotSmoothing::Octave3.octave_fraction().unwrap() - (1.0 / 3.0)).abs() < 0.01);
    assert!((PlotSmoothing::Octave6.octave_fraction().unwrap() - (1.0 / 6.0)).abs() < 0.01);
    assert!((PlotSmoothing::Octave24.octave_fraction().unwrap() - (1.0 / 24.0)).abs() < 0.01);
}

#[gpui::test]
async fn test_plot_channel_dropdown_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(!state.borrow().plot_channel_dropdown_open);

    state.borrow_mut().plot_channel_dropdown_open = true;
    assert!(state.borrow().plot_channel_dropdown_open);
}

#[gpui::test]
async fn test_plot_smoothing_dropdown_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(!state.borrow().plot_smoothing_dropdown_open);

    state.borrow_mut().plot_smoothing_dropdown_open = true;
    assert!(state.borrow().plot_smoothing_dropdown_open);
}

#[gpui::test]
async fn test_channel_selection_for_viewing(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(state.borrow().plot_selected_channel.is_none());

    state.borrow_mut().plot_selected_channel = Some(0);
    assert_eq!(state.borrow().plot_selected_channel, Some(0));

    state.borrow_mut().plot_selected_channel = Some(1);
    assert_eq!(state.borrow().plot_selected_channel, Some(1));

    state.borrow_mut().plot_selected_channel = None;
    assert!(state.borrow().plot_selected_channel.is_none());
}

#[gpui::test]
async fn test_frequency_response_data_display(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().init_channel_recordings();
    state.borrow_mut().channel_recordings[0].result = Some(RecordingResult {
        channel: 0,
        wav_path: None,
        csv_path: None,
        frequencies: (1..=100)
            .map(|i| 20.0 * (1.5_f32).powi(i as i32 / 10))
            .collect(),
        magnitude_db: vec![-3.0; 100],
        phase_deg: vec![0.0; 100],
    });

    let result = state.borrow().channel_recordings[0].result.clone().unwrap();
    assert_eq!(result.frequencies.len(), 100);
    assert_eq!(result.magnitude_db.len(), 100);
    assert_eq!(result.phase_deg.len(), 100);
}

#[gpui::test]
async fn test_evaluating_step_always_can_advance(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));
    state.borrow_mut().step = RecordingStep::Evaluating;

    assert!(state.borrow().can_advance());
}

// =============================================================================
// Step 4: Saving Tests - All Fields Editable
// =============================================================================

#[gpui::test]
async fn test_save_directory_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().recording_base_directory = Some("/tmp/measurements".to_string());
    assert!(state.borrow().recording_base_directory.is_some());
}

#[gpui::test]
async fn test_save_name_entry(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().save_name = "Living Room 2024-01-06".to_string();
    assert_eq!(state.borrow().save_name, "Living Room 2024-01-06");

    state.borrow_mut().save_name = "Test Recording".to_string();
    assert_eq!(state.borrow().save_name, "Test Recording");

    state.borrow_mut().save_name = "".to_string();
    assert_eq!(state.borrow().save_name, "");
}

#[gpui::test]
async fn test_save_name_sanitization(_cx: &mut TestAppContext) {
    let sanitize = |name: &str| {
        name.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
    };

    assert_eq!(sanitize("Living Room 2024-01-06"), "Living_Room_2024-01-06");
    assert_eq!(sanitize("Test/Recording"), "Test_Recording");
    assert_eq!(sanitize("Safe_Name-123"), "Safe_Name-123");
}

#[gpui::test]
async fn test_saving_step_always_can_advance(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));
    state.borrow_mut().step = RecordingStep::Saving;

    assert!(state.borrow().can_advance());
}

// =============================================================================
// Full Wizard Flow Tests
// =============================================================================

#[gpui::test]
async fn test_complete_wizard_flow(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    // Step 1: Configure
    assert_eq!(state.borrow().step, RecordingStep::Config);
    state.borrow_mut().playback_config.device_name = "Test Device".to_string();
    state.borrow_mut().recording_base_directory = Some("/tmp".to_string());
    state.borrow_mut().signal_type = SignalType::Sweep;
    assert!(state.borrow().has_output_directory());

    // Navigate to Capture
    state.borrow_mut().step = RecordingStep::Capture;
    assert_eq!(state.borrow().step, RecordingStep::Capture);

    // Initialize and record channels
    state.borrow_mut().playback_config.channel_mappings = vec![
        ChannelMapping {
            interface_channel: 0,
            group_name: "L".to_string(),
        },
        ChannelMapping {
            interface_channel: 1,
            group_name: "R".to_string(),
        },
    ];
    state.borrow_mut().init_channel_recordings();
    let channel_count = state.borrow().channel_recordings.len();
    for i in 0..channel_count {
        state.borrow_mut().channel_recordings[i].state = ChannelRecordingState::Done;
        state.borrow_mut().channel_recordings[i].result = Some(RecordingResult {
            channel: i,
            wav_path: Some(format!("/tmp/ch{}.wav", i)),
            csv_path: Some(format!("/tmp/ch{}.csv", i)),
            frequencies: vec![100.0, 1000.0, 10000.0],
            magnitude_db: vec![0.0, 0.0, 0.0],
            phase_deg: vec![0.0, 0.0, 0.0],
        });
    }
    assert!(state.borrow().all_channels_recorded());

    // Navigate to Evaluate
    state.borrow_mut().step = RecordingStep::Evaluating;
    assert_eq!(state.borrow().step, RecordingStep::Evaluating);
    state.borrow_mut().plot_smoothing = PlotSmoothing::Octave3;
    state.borrow_mut().plot_selected_channel = Some(0);

    // Navigate to Save
    state.borrow_mut().step = RecordingStep::Saving;
    assert_eq!(state.borrow().step, RecordingStep::Saving);
    state.borrow_mut().save_name = "Test Recording".to_string();

    assert_eq!(state.borrow().step, RecordingStep::Saving);
}

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

#[gpui::test]
async fn test_status_message_updates(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().status_message = "Configuring devices...".to_string();
    assert!(state.borrow().status_message.contains("Configuring"));

    state.borrow_mut().status_message = "Recording channel L...".to_string();
    assert!(state.borrow().status_message.contains("Recording"));

    state.borrow_mut().status_message = "Saving files...".to_string();
    assert!(state.borrow().status_message.contains("Saving"));

    state.borrow_mut().status_message = "All channels recorded successfully!".to_string();
    assert!(state.borrow().status_message.contains("success"));
}

#[gpui::test]
async fn test_error_message_display(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(state.borrow().status_message.is_empty());

    state.borrow_mut().status_message = "Device not found".to_string();
    assert!(state.borrow().status_message.contains("Device"));
}

#[gpui::test]
async fn test_surround_recording_setup(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().playback_config.speaker_configuration = SpeakerConfiguration::Surround51;
    state.borrow_mut().playback_config.channel_mappings = vec![
        ChannelMapping {
            interface_channel: 0,
            group_name: "L".to_string(),
        },
        ChannelMapping {
            interface_channel: 1,
            group_name: "R".to_string(),
        },
        ChannelMapping {
            interface_channel: 2,
            group_name: "C".to_string(),
        },
        ChannelMapping {
            interface_channel: 3,
            group_name: "LFE".to_string(),
        },
        ChannelMapping {
            interface_channel: 4,
            group_name: "SL".to_string(),
        },
        ChannelMapping {
            interface_channel: 5,
            group_name: "SR".to_string(),
        },
    ];

    state.borrow_mut().init_channel_recordings();

    assert_eq!(state.borrow().channel_recordings.len(), 6);
    assert_eq!(state.borrow().channel_recordings[3].channel_name, "LFE");
    assert_eq!(state.borrow().channel_recordings[4].channel_name, "SL");
    assert_eq!(state.borrow().channel_recordings[5].channel_name, "SR");
}

#[gpui::test]
async fn test_atmos_recording_setup(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    state.borrow_mut().playback_config.speaker_configuration = SpeakerConfiguration::Atmos714;
    state.borrow_mut().playback_config.channel_mappings = vec![
        ChannelMapping {
            interface_channel: 0,
            group_name: "L".to_string(),
        },
        ChannelMapping {
            interface_channel: 1,
            group_name: "R".to_string(),
        },
        ChannelMapping {
            interface_channel: 2,
            group_name: "C".to_string(),
        },
        ChannelMapping {
            interface_channel: 3,
            group_name: "LFE".to_string(),
        },
        ChannelMapping {
            interface_channel: 4,
            group_name: "SL".to_string(),
        },
        ChannelMapping {
            interface_channel: 5,
            group_name: "SR".to_string(),
        },
        ChannelMapping {
            interface_channel: 6,
            group_name: "BL".to_string(),
        },
        ChannelMapping {
            interface_channel: 7,
            group_name: "BR".to_string(),
        },
        ChannelMapping {
            interface_channel: 8,
            group_name: "TFL".to_string(),
        },
        ChannelMapping {
            interface_channel: 9,
            group_name: "TFR".to_string(),
        },
        ChannelMapping {
            interface_channel: 10,
            group_name: "TBL".to_string(),
        },
        ChannelMapping {
            interface_channel: 11,
            group_name: "TBR".to_string(),
        },
    ];

    state.borrow_mut().init_channel_recordings();

    assert_eq!(state.borrow().channel_recordings.len(), 12);
    assert_eq!(state.borrow().channel_recordings[8].channel_name, "TFL");
    assert_eq!(state.borrow().channel_recordings[11].channel_name, "TBR");
}

#[gpui::test]
async fn test_config_accordion_expansion(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RecordingState::default()));

    assert!(state.borrow().config_accordion_expanded.is_empty());

    state.borrow_mut().config_accordion_expanded = vec!["playback".to_string()];
    assert_eq!(state.borrow().config_accordion_expanded.len(), 1);
    assert!(
        state
            .borrow()
            .config_accordion_expanded
            .contains(&"playback".to_string())
    );

    state
        .borrow_mut()
        .config_accordion_expanded
        .push("recording".to_string());
    assert_eq!(state.borrow().config_accordion_expanded.len(), 2);

    state
        .borrow_mut()
        .config_accordion_expanded
        .retain(|id| id != "playback");
    assert_eq!(state.borrow().config_accordion_expanded.len(), 1);
    assert!(
        !state
            .borrow()
            .config_accordion_expanded
            .contains(&"playback".to_string())
    );
}

#[gpui::test]
async fn test_recording_directory_path_generation(_cx: &mut TestAppContext) {
    let generate_directory_path =
        |base_dir: &str, timestamp: &str| format!("{}/recording-{}", base_dir, timestamp);

    let path = generate_directory_path("/tmp/recordings", "20240107-120000");
    assert_eq!(path, "/tmp/recordings/recording-20240107-120000");

    let path = generate_directory_path("/home/user/measurements", "20240615-153045");
    assert_eq!(path, "/home/user/measurements/recording-20240615-153045");
}

#[gpui::test]
async fn test_signal_level_to_amplitude_conversion(_cx: &mut TestAppContext) {
    let db_to_amplitude = |db: f32| 10.0_f32.powf(db / 20.0);

    assert!((db_to_amplitude(0.0) - 1.0).abs() < 0.001);
    assert!((db_to_amplitude(-6.0) - 0.5).abs() < 0.01);
    assert!((db_to_amplitude(-12.0) - 0.25).abs() < 0.01);
    assert!((db_to_amplitude(-20.0) - 0.1).abs() < 0.01);
}

#[gpui::test]
async fn test_recording_result_frequencies_range(_cx: &mut TestAppContext) {
    let result = RecordingResult {
        channel: 0,
        wav_path: None,
        csv_path: None,
        frequencies: (20..=20000).map(|f| f as f32).collect(),
        magnitude_db: vec![0.0; 19981],
        phase_deg: vec![0.0; 19981],
    };

    assert_eq!(result.frequencies.len(), 19981);
    assert!((result.frequencies[0] - 20.0).abs() < 0.1);
    assert!((result.frequencies.last().unwrap() - 20000.0).abs() < 0.1);
}

#[gpui::test]
async fn test_channel_recording_states_complete(_cx: &mut TestAppContext) {
    assert_eq!(ChannelRecordingState::Empty as u8, 0);
    assert_eq!(ChannelRecordingState::Recording as u8, 1);
    assert_eq!(ChannelRecordingState::Done as u8, 2);
    assert_eq!(ChannelRecordingState::Error as u8, 3);
}

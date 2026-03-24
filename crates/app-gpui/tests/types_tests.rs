//! Type tests for GPUI App types.
//!
//! These tests verify the behavior of types defined in the app module.
//! They are extracted from inline tests to work around GPUI macro recursion issues.

use sotf_audio_player::PluginType;
use sotf_audio_player::library::{Album, Track};
use sotf_audio_player_gpui::app::types::{HeadphoneEqState, SpinoramaEqState};
use sotf_audio_player_gpui::{
    App, CalibrationData, ChannelMapping, ChannelRecording, ChannelRecordingState, ContextMenuType,
    CrossoverType, InputMode, LayoutMode, LibraryStats, MeasureState, MeterDisplayMode,
    PlaybackDeviceConfig, PlotSmoothing, PluginViewMode, QueueItem, RecordingDeviceConfig,
    RecordingSignalType, RecordingState, RecordingStep, ReplayGainMode, RoomEqAlgorithm,
    RoomEqOptimizerConfig, RoomEqStep, Screen, SpeakerConfiguration, ToastMessage, ToastType,
};
use std::path::PathBuf;

// ============================================================================
// Screen Enum Tests
// ============================================================================

#[test]
fn test_screen_variants() {
    let screens = [
        Screen::Library,
        Screen::Queue,
        Screen::Spectrum,
        Screen::Settings,
        Screen::Studio,
        Screen::Recording,
        Screen::RoomEq,
        Screen::HeadphoneEq,
        Screen::Spinorama,
        Screen::PluginGraph,
    ];
    assert_eq!(screens.len(), 10);
    assert_ne!(Screen::Library, Screen::Queue);
}

#[test]
fn test_screen_copy_clone() {
    let screen = Screen::Library;
    let copied = screen;
    let cloned = screen;
    assert_eq!(screen, copied);
    assert_eq!(screen, cloned);
}

#[test]
fn test_headphone_eq_ui_loss_type_uses_optimizer_source_of_truth() {
    let state = HeadphoneEqState::default();
    assert_eq!(state.ui_loss_type(), "score");
    assert_eq!(state.optimizer_config.loss, "headphone-score");
}

#[test]
fn test_headphone_eq_set_ui_loss_type_updates_optimizer_loss() {
    let mut state = HeadphoneEqState::default();

    state.set_ui_loss_type("flat");
    assert_eq!(state.ui_loss_type(), "flat");
    assert_eq!(state.loss_type, "flat");
    assert_eq!(state.optimizer_config.loss, "headphone-flat");

    state.set_ui_loss_type("score");
    assert_eq!(state.ui_loss_type(), "score");
    assert_eq!(state.loss_type, "score");
    assert_eq!(state.optimizer_config.loss, "headphone-score");
}

#[test]
fn test_headphone_eq_set_ui_loss_type_normalizes_unknown_values() {
    let mut state = HeadphoneEqState::default();

    // Unknown values should normalize to "score" (matching optimizer fallback)
    state.set_ui_loss_type("garbage");
    assert_eq!(state.ui_loss_type(), "score");
    assert_eq!(
        state.loss_type, "score",
        "loss_type should be normalized to 'score' for unknown input"
    );
    assert_eq!(state.optimizer_config.loss, "headphone-score");

    // Prefixed variants should also normalize
    state.set_ui_loss_type("headphone-flat");
    assert_eq!(state.ui_loss_type(), "flat");
    assert_eq!(state.loss_type, "flat");
    assert_eq!(state.optimizer_config.loss, "headphone-flat");

    state.set_ui_loss_type("headphone-score");
    assert_eq!(state.ui_loss_type(), "score");
    assert_eq!(state.loss_type, "score");
    assert_eq!(state.optimizer_config.loss, "headphone-score");
}

#[test]
fn test_headphone_eq_custom_target_path_helpers() {
    let mut state = HeadphoneEqState::default();
    assert!(!state.requires_custom_target_path());
    assert!(!state.has_custom_target_path());

    state.target_preset = "custom".to_string();
    assert!(state.requires_custom_target_path());
    assert!(!state.has_custom_target_path());

    state.custom_target_path = Some(" /tmp/custom-target.csv ".to_string());
    assert!(state.has_custom_target_path());
}

#[test]
fn test_spinorama_supported_eq_modes_are_iir_only() {
    let state = SpinoramaEqState::default();
    assert_eq!(state.supported_eq_modes(), &["iir"]);
    assert_eq!(state.selected_eq_mode(), "iir");
}

#[test]
fn test_spinorama_set_selected_eq_mode_normalizes_to_iir() {
    let mut state = SpinoramaEqState::default();

    state.set_selected_eq_mode("mixed");
    assert_eq!(state.selected_eq_mode(), "iir");
    assert_eq!(state.dropdowns.opt_mode, "iir");

    state.set_selected_eq_mode("fir");
    assert_eq!(state.selected_eq_mode(), "iir");
    assert_eq!(state.dropdowns.opt_mode, "iir");
}

// ============================================================================
// InputMode Enum Tests
// ============================================================================

#[test]
fn test_input_mode_variants() {
    let modes = [
        InputMode::Normal,
        InputMode::Search,
        InputMode::AddDirectory,
        InputMode::SavePlugins,
        InputMode::LoadPlugins,
        InputMode::LoadApoFile,
        InputMode::LoadSofaFile,
        InputMode::Help,
        InputMode::KeyboardShortcuts,
        InputMode::About,
        InputMode::EditingParam,
        InputMode::SpinoramaSpeakerSearch,
    ];
    assert_eq!(modes.len(), 12);
}

// ============================================================================
// ToastMessage Tests
// ============================================================================

#[test]
fn test_toast_message_new() {
    let toast = ToastMessage::new("Test message".to_string(), ToastType::Info);
    assert_eq!(toast.message, "Test message");
    assert_eq!(toast.toast_type, ToastType::Info);
    assert_eq!(toast.auto_dismiss_ms, Some(5000));
}

#[test]
fn test_toast_message_success() {
    let toast = ToastMessage::success("Success!");
    assert_eq!(toast.message, "Success!");
    assert_eq!(toast.toast_type, ToastType::Success);
}

#[test]
fn test_toast_message_error() {
    let toast = ToastMessage::error("Error occurred");
    assert_eq!(toast.message, "Error occurred");
    assert_eq!(toast.toast_type, ToastType::Error);
}

#[test]
fn test_toast_message_info() {
    let toast = ToastMessage::info("Info message");
    assert_eq!(toast.message, "Info message");
    assert_eq!(toast.toast_type, ToastType::Info);
}

#[test]
fn test_toast_message_warning() {
    let toast = ToastMessage::warning("Warning!");
    assert_eq!(toast.message, "Warning!");
    assert_eq!(toast.toast_type, ToastType::Warning);
}

#[test]
fn test_toast_message_persistent() {
    let toast = ToastMessage::persistent("Persistent message", ToastType::Error);
    assert_eq!(toast.message, "Persistent message");
    assert_eq!(toast.toast_type, ToastType::Error);
    assert_eq!(toast.auto_dismiss_ms, None);
}

#[test]
fn test_toast_message_should_dismiss_not_expired() {
    let toast = ToastMessage::new("Test".to_string(), ToastType::Info);
    // Just created, should not dismiss yet
    assert!(!toast.should_dismiss());
}

#[test]
fn test_toast_message_persistent_never_dismisses() {
    let toast = ToastMessage::persistent("Test", ToastType::Info);
    assert!(!toast.should_dismiss());
}

#[test]
fn test_app_rollback_failed_plugin_update_restores_snapshot_and_sets_toast() {
    let mut app = App::new();
    let original_plugin_count = app.plugin_state.chain.len();
    let snapshot = app.plugin_state.clone();

    let effect = app.plugin_state.add_plugin(&PluginType::Upmixer);
    assert!(matches!(
        effect,
        sotf_audio_player::PluginUpdateEffect::Structural
    ));
    assert_eq!(app.plugin_state.chain.len(), original_plugin_count + 1);

    app.rollback_failed_plugin_update(snapshot, "device only supports 2 channels");

    assert_eq!(app.plugin_state.chain.len(), original_plugin_count);
    assert!(
        app.ui_state
            .toast_message
            .as_ref()
            .is_some_and(|toast| toast.message.contains("device only supports 2 channels"))
    );
    assert_eq!(
        app.ui_state
            .toast_message
            .as_ref()
            .map(|toast| toast.toast_type),
        Some(ToastType::Error)
    );
}

// ============================================================================
// QueueItem Tests
// ============================================================================

fn create_test_album(track_count: usize) -> Album {
    let tracks: Vec<Track> = (0..track_count)
        .map(|i| Track {
            path: PathBuf::from(format!("/music/track_{}.flac", i)),
            track_number: Some(i as u32 + 1),
            title: Some(format!("Track {}", i + 1)),
            duration_secs: Some(180),
            sample_rate: Some(44100),
            channels: Some(2),
            bit_depth: Some(16),
            disc_number: None,
            artist: Some("Test Artist".to_string()),
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: None,
            composer: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
            edition: None,
            is_favorite: false,
            play_count: 0,
            source: None,
            uuid: None,
        })
        .collect();

    Album {
        id: None,
        title: "Test Album".to_string(),
        year: Some(2024),
        tracks,
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
        edition: None,
        dynamic_range: None,
        is_favorite: false,
        uuid: None,
    }
}

#[test]
fn test_queue_item_new() {
    let album = create_test_album(5);
    let item = QueueItem::new(album);
    assert_eq!(item.current_track_index, 0);
    assert_eq!(item.album.tracks.len(), 5);
}

#[test]
fn test_queue_item_current_track() {
    let album = create_test_album(3);
    let item = QueueItem::new(album);
    let track = item.current_track().unwrap();
    assert_eq!(track.title, Some("Track 1".to_string()));
}

#[test]
fn test_queue_item_next_track() {
    let album = create_test_album(3);
    let mut item = QueueItem::new(album);

    let track = item.next_track().unwrap();
    assert_eq!(track.title, Some("Track 2".to_string()));
    assert_eq!(item.current_track_index, 1);

    let track = item.next_track().unwrap();
    assert_eq!(track.title, Some("Track 3".to_string()));
    assert_eq!(item.current_track_index, 2);

    // No more tracks
    assert!(item.next_track().is_none());
    assert_eq!(item.current_track_index, 2);
}

#[test]
fn test_queue_item_previous_track() {
    let album = create_test_album(3);
    let mut item = QueueItem::new(album);
    item.current_track_index = 2;

    let track = item.previous_track().unwrap();
    assert_eq!(track.title, Some("Track 2".to_string()));
    assert_eq!(item.current_track_index, 1);

    let track = item.previous_track().unwrap();
    assert_eq!(track.title, Some("Track 1".to_string()));
    assert_eq!(item.current_track_index, 0);

    // Can't go before first track
    assert!(item.previous_track().is_none());
    assert_eq!(item.current_track_index, 0);
}

#[test]
fn test_queue_item_empty_album() {
    let album = create_test_album(0);
    let item = QueueItem::new(album);
    assert!(item.current_track().is_none());
}

// ============================================================================
// Auto-advance regression tests
//
// The UI polling loop must use the *previous* is_playing value to detect
// end-of-track, not the engine's current state. A past bug overwrote
// is_playing before the check, so auto-advance never triggered.
// ============================================================================

/// Simulates the auto-advance detection from ui/mod.rs polling loop.
/// Returns the path of the next track if auto-advance triggered.
fn simulate_auto_advance(
    app_is_playing: bool,
    engine_is_playing: bool,
    current_queue_index: &mut Option<usize>,
    queue: &mut [QueueItem],
) -> Option<PathBuf> {
    // Step 1: save previous state (the fix)
    let was_playing = app_is_playing;
    // Step 2: app_is_playing would be overwritten to engine_is_playing here

    // Step 3: auto-advance check uses was_playing, NOT the overwritten value
    if was_playing && !engine_is_playing && current_queue_index.is_some() {
        let idx = current_queue_index.unwrap();
        if let Some(item) = queue.get_mut(idx) {
            // Try next track in current album
            if let Some(track) = item.next_track() {
                return Some(track.path.clone());
            }
        }
        // Try next album
        if idx + 1 < queue.len() {
            *current_queue_index = Some(idx + 1);
            queue[idx + 1].current_track_index = 0;
            return queue[idx + 1].current_track().map(|t| t.path.clone());
        }
    }
    None
}

#[test]
fn test_auto_advance_within_album() {
    let album = create_test_album(3);
    let mut queue = vec![QueueItem::new(album)];
    let mut current_idx = Some(0);

    // Track ends: app was playing, engine stopped
    let next = simulate_auto_advance(true, false, &mut current_idx, &mut queue);

    assert!(next.is_some(), "Should advance to next track in album");
    assert_eq!(queue[0].current_track_index, 1);
}

#[test]
fn test_auto_advance_across_albums() {
    let album1 = create_test_album(1); // single-track album
    let album2 = create_test_album(2);
    let mut queue = vec![QueueItem::new(album1), QueueItem::new(album2)];
    let mut current_idx = Some(0);

    // Last track of album 1 ends
    let next = simulate_auto_advance(true, false, &mut current_idx, &mut queue);

    assert!(
        next.is_some(),
        "Should advance to first track of next album"
    );
    assert_eq!(current_idx, Some(1), "Queue index should move to album 2");
    assert_eq!(queue[1].current_track_index, 0);
}

#[test]
fn test_auto_advance_stops_at_end_of_queue() {
    let album = create_test_album(1);
    let mut queue = vec![QueueItem::new(album)];
    let mut current_idx = Some(0);

    // Only track in only album ends
    let next = simulate_auto_advance(true, false, &mut current_idx, &mut queue);

    assert!(next.is_none(), "Should not advance — queue is finished");
}

#[test]
fn test_no_auto_advance_when_paused() {
    let album = create_test_album(3);
    let mut queue = vec![QueueItem::new(album)];
    let mut current_idx = Some(0);

    // User paused: app not playing, engine not playing
    let next = simulate_auto_advance(false, false, &mut current_idx, &mut queue);

    assert!(next.is_none(), "Should not advance when user paused");
    assert_eq!(
        queue[0].current_track_index, 0,
        "Track index should not change"
    );
}

#[test]
fn test_no_auto_advance_while_still_playing() {
    let album = create_test_album(3);
    let mut queue = vec![QueueItem::new(album)];
    let mut current_idx = Some(0);

    // Normal playback: both app and engine report playing
    let next = simulate_auto_advance(true, true, &mut current_idx, &mut queue);

    assert!(
        next.is_none(),
        "Should not advance while track is still playing"
    );
    assert_eq!(queue[0].current_track_index, 0);
}

// ============================================================================
// SpeakerConfiguration Tests
// ============================================================================

#[test]
fn test_speaker_configuration_as_str() {
    assert_eq!(SpeakerConfiguration::Stereo.as_str(), "2.0");
    assert_eq!(SpeakerConfiguration::Stereo21.as_str(), "2.1");
    assert_eq!(SpeakerConfiguration::Surround50.as_str(), "5.0");
    assert_eq!(SpeakerConfiguration::Surround51.as_str(), "5.1");
    assert_eq!(SpeakerConfiguration::Surround71.as_str(), "7.1");
    assert_eq!(SpeakerConfiguration::Immersive714.as_str(), "7.1.4");
    assert_eq!(SpeakerConfiguration::Custom.as_str(), "Custom");
}

#[test]
fn test_speaker_configuration_channel_count() {
    assert_eq!(SpeakerConfiguration::Stereo.channel_count(), 2);
    assert_eq!(SpeakerConfiguration::Stereo21.channel_count(), 3);
    assert_eq!(SpeakerConfiguration::Surround50.channel_count(), 5);
    assert_eq!(SpeakerConfiguration::Surround51.channel_count(), 6);
    assert_eq!(SpeakerConfiguration::Surround71.channel_count(), 8);
    assert_eq!(SpeakerConfiguration::Surround91.channel_count(), 10);
    assert_eq!(SpeakerConfiguration::Immersive714.channel_count(), 12);
    assert_eq!(SpeakerConfiguration::Immersive916.channel_count(), 16);
}

#[test]
fn test_speaker_configuration_default_channel_names() {
    let stereo = SpeakerConfiguration::Stereo.default_channel_names();
    assert_eq!(stereo, vec!["L", "R"]);

    let surround51 = SpeakerConfiguration::Surround51.default_channel_names();
    assert_eq!(surround51, vec!["L", "R", "C", "LFE", "SL", "SR"]);

    let immersive714 = SpeakerConfiguration::Immersive714.default_channel_names();
    assert_eq!(immersive714.len(), 12);
    assert!(immersive714.contains(&"TFL"));
    assert!(immersive714.contains(&"TBR"));
}

#[test]
fn test_speaker_configuration_from_channel_count() {
    assert_eq!(
        SpeakerConfiguration::from_channel_count(2),
        SpeakerConfiguration::Stereo
    );
    assert_eq!(
        SpeakerConfiguration::from_channel_count(6),
        SpeakerConfiguration::Surround51
    );
    assert_eq!(
        SpeakerConfiguration::from_channel_count(8),
        SpeakerConfiguration::Surround71
    );
    assert_eq!(
        SpeakerConfiguration::from_channel_count(99),
        SpeakerConfiguration::Custom
    );
}

#[test]
fn test_speaker_configuration_all() {
    let all = SpeakerConfiguration::all();
    assert_eq!(all.len(), 14);
    assert!(all.contains(&SpeakerConfiguration::Stereo));
    assert!(all.contains(&SpeakerConfiguration::Custom));
}

// ============================================================================
// PlotSmoothing Tests
// ============================================================================

#[test]
fn test_plot_smoothing_as_str() {
    assert_eq!(PlotSmoothing::None.as_str(), "None");
    assert_eq!(PlotSmoothing::Octave1.as_str(), "1/1 octave");
    assert_eq!(PlotSmoothing::Octave3.as_str(), "1/3 octave");
    assert_eq!(PlotSmoothing::Octave6.as_str(), "1/6 octave");
    assert_eq!(PlotSmoothing::Octave24.as_str(), "1/24 octave");
}

#[test]
fn test_plot_smoothing_octave_fraction() {
    assert_eq!(PlotSmoothing::None.octave_fraction(), None);
    assert_eq!(PlotSmoothing::Octave1.octave_fraction(), Some(1.0));
    assert!((PlotSmoothing::Octave3.octave_fraction().unwrap() - 1.0 / 3.0).abs() < 0.001);
    assert!((PlotSmoothing::Octave6.octave_fraction().unwrap() - 1.0 / 6.0).abs() < 0.001);
    assert!((PlotSmoothing::Octave24.octave_fraction().unwrap() - 1.0 / 24.0).abs() < 0.001);
}

#[test]
fn test_plot_smoothing_default() {
    assert_eq!(PlotSmoothing::default(), PlotSmoothing::None);
}

// ============================================================================
// RecordingSignalType Tests
// ============================================================================

#[test]
fn test_recording_signal_type_as_str() {
    assert_eq!(RecordingSignalType::Sweep.as_str(), "Sweep");
    assert_eq!(RecordingSignalType::WhiteNoise.as_str(), "White Noise");
    assert_eq!(RecordingSignalType::PinkNoise.as_str(), "Pink Noise");
}

#[test]
fn test_recording_signal_type_all() {
    let all = RecordingSignalType::all();
    assert_eq!(all.len(), 3);
    assert!(all.contains(&RecordingSignalType::Sweep));
    assert!(all.contains(&RecordingSignalType::WhiteNoise));
    assert!(all.contains(&RecordingSignalType::PinkNoise));
}

// ============================================================================
// CalibrationData Tests
// ============================================================================

#[test]
fn test_calibration_data_parse_csv() {
    let content = "100, 0.5\n1000, -0.2\n10000, 0.8";
    let data = CalibrationData::parse(content).unwrap();
    assert_eq!(data.frequencies.len(), 3);
    assert_eq!(data.spl_db.len(), 3);
    assert!((data.frequencies[0] - 100.0).abs() < 0.001);
    assert!((data.frequencies[1] - 1000.0).abs() < 0.001);
    assert!((data.spl_db[0] - 0.5).abs() < 0.001);
}

#[test]
fn test_calibration_data_parse_with_comments() {
    let content = "# Calibration file\n// Another comment\n100\t0.5\n1000\t-0.2";
    let data = CalibrationData::parse(content).unwrap();
    assert_eq!(data.frequencies.len(), 2);
}

#[test]
fn test_calibration_data_parse_with_header() {
    let content = "Frequency Hz, SPL dB\n100, 0.5\n1000, -0.2";
    let data = CalibrationData::parse(content).unwrap();
    assert_eq!(data.frequencies.len(), 2);
}

#[test]
fn test_calibration_data_parse_empty() {
    let content = "# Only comments\n// Nothing else";
    assert!(CalibrationData::parse(content).is_none());
}

#[test]
fn test_calibration_data_parse_invalid_frequency() {
    // Frequencies out of range (> 100kHz) should be ignored
    let content = "100, 0.5\n200000, 0.5"; // 200kHz is out of range
    let data = CalibrationData::parse(content).unwrap();
    assert_eq!(data.frequencies.len(), 1);
}

#[test]
fn test_calibration_data_is_valid() {
    let valid = CalibrationData {
        frequencies: vec![100.0, 1000.0],
        spl_db: vec![0.5, -0.2],
    };
    assert!(valid.is_valid());

    let empty = CalibrationData::default();
    assert!(!empty.is_valid());

    let mismatched = CalibrationData {
        frequencies: vec![100.0, 1000.0],
        spl_db: vec![0.5],
    };
    assert!(!mismatched.is_valid());
}

// ============================================================================
// RoomEqStep Tests
// ============================================================================

#[test]
fn test_room_eq_step_all() {
    let all = RoomEqStep::all();
    assert_eq!(all.len(), 5);
}

#[test]
fn test_room_eq_step_index() {
    assert_eq!(RoomEqStep::LoadData.index(), 0);
    assert_eq!(RoomEqStep::Configure.index(), 1);
    assert_eq!(RoomEqStep::Optimize.index(), 2);
    assert_eq!(RoomEqStep::Review.index(), 3);
    assert_eq!(RoomEqStep::Export.index(), 4);
}

#[test]
fn test_room_eq_step_label() {
    assert_eq!(RoomEqStep::LoadData.label(), "Load Data");
    assert_eq!(RoomEqStep::Configure.label(), "Configure");
    assert_eq!(RoomEqStep::Export.label(), "Export");
}

#[test]
fn test_room_eq_step_next() {
    assert_eq!(RoomEqStep::LoadData.next(), Some(RoomEqStep::Configure));
    assert_eq!(RoomEqStep::Configure.next(), Some(RoomEqStep::Optimize));
    assert_eq!(RoomEqStep::Export.next(), None);
}

#[test]
fn test_room_eq_step_previous() {
    assert_eq!(RoomEqStep::LoadData.previous(), None);
    assert_eq!(RoomEqStep::Configure.previous(), Some(RoomEqStep::LoadData));
    assert_eq!(RoomEqStep::Export.previous(), Some(RoomEqStep::Review));
}

// ============================================================================
// CrossoverType Tests
// ============================================================================

#[test]
fn test_crossover_type_all() {
    let all = CrossoverType::all();
    assert_eq!(all.len(), 5);
}

#[test]
fn test_crossover_type_as_str() {
    assert_eq!(CrossoverType::LR12.as_str(), "Linkwitz-Riley 12dB");
    assert_eq!(CrossoverType::LR24.as_str(), "Linkwitz-Riley 24dB");
    assert_eq!(CrossoverType::Butterworth12.as_str(), "Butterworth 12dB");
}

#[test]
fn test_crossover_type_default() {
    assert_eq!(CrossoverType::default(), CrossoverType::LR24);
}

// ============================================================================
// RoomEqAlgorithm Tests
// ============================================================================

#[test]
fn test_room_eq_algorithm_all() {
    let all = RoomEqAlgorithm::all();
    assert_eq!(all.len(), 3);
}

#[test]
fn test_room_eq_algorithm_as_str() {
    assert_eq!(RoomEqAlgorithm::Cobyla.as_str(), "COBYLA");
    assert_eq!(
        RoomEqAlgorithm::DifferentialEvolution.as_str(),
        "Differential Evolution"
    );
    assert_eq!(RoomEqAlgorithm::NelderMead.as_str(), "Nelder-Mead");
}

#[test]
fn test_room_eq_algorithm_to_autoeq_string() {
    assert_eq!(RoomEqAlgorithm::Cobyla.to_autoeq_string(), "cobyla");
    assert_eq!(
        RoomEqAlgorithm::DifferentialEvolution.to_autoeq_string(),
        "autoeq:de"
    );
    assert_eq!(
        RoomEqAlgorithm::NelderMead.to_autoeq_string(),
        "nelder-mead"
    );
}

// ============================================================================
// Default Implementations Tests
// ============================================================================

#[test]
fn test_library_stats_default() {
    let stats = LibraryStats::default();
    assert_eq!(stats.artists_count, 0);
    assert_eq!(stats.total_tracks, 0);
    assert!(!stats.valid);
}

#[test]
fn test_measure_state_default() {
    let state = MeasureState::default();
    assert_eq!(state.signal_type, "sweep");
    assert_eq!(state.level, -20.0);
}

#[test]
fn test_playback_device_config_default() {
    let config = PlaybackDeviceConfig::default();
    assert_eq!(config.num_channels, 2);
    assert_eq!(config.sample_rate, 48000);
    assert_eq!(config.speaker_configuration, SpeakerConfiguration::Stereo);
    assert_eq!(config.channel_mappings.len(), 2);
}

#[test]
fn test_recording_device_config_default() {
    let config = RecordingDeviceConfig::default();
    assert_eq!(config.num_channels, 1);
    assert_eq!(config.sample_rate, 48000);
    assert_eq!(config.channel_mappings.len(), 1);
}

#[test]
fn test_recording_state_default() {
    let state = RecordingState::default();
    assert_eq!(state.step, RecordingStep::Config);
    assert_eq!(state.signal_type, RecordingSignalType::Sweep);
    assert_eq!(state.signal_duration_secs, 5.0);
    assert_eq!(state.signal_level_db, -20.0);
}

#[test]
fn test_room_eq_optimizer_config_default() {
    let config = RoomEqOptimizerConfig::default();
    assert_eq!(config.algorithm, "autoeq:de");
    assert_eq!(config.num_filters, 7);
    assert!((config.min_q - 0.5).abs() < 0.001);
    assert!((config.max_q - 6.0).abs() < 0.001);
}

// ============================================================================
// RecordingState Method Tests
// ============================================================================

#[test]
fn test_recording_state_init_channel_recordings() {
    let mut state = RecordingState::default();
    state.playback_config.channel_mappings = vec![
        ChannelMapping::single(1, "L"),
        ChannelMapping::single(2, "R"),
        ChannelMapping::single(3, "C"),
    ];

    state.init_channel_recordings();

    assert_eq!(state.channel_recordings.len(), 3);
    assert_eq!(state.channel_recordings[0].channel_name, "L");
    assert_eq!(state.channel_recordings[1].channel_name, "R");
    assert_eq!(state.channel_recordings[2].channel_name, "C");
    assert_eq!(
        state.channel_recordings[0].state,
        ChannelRecordingState::Empty
    );
}

#[test]
fn test_recording_state_all_channels_recorded() {
    let mut state = RecordingState::default();
    state.channel_recordings = vec![
        {
            let mut rec = ChannelRecording::new(0, "L".to_string());
            rec.state = ChannelRecordingState::Done;
            rec
        },
        {
            let mut rec = ChannelRecording::new(1, "R".to_string());
            rec.state = ChannelRecordingState::Done;
            rec
        },
    ];

    assert!(state.all_channels_recorded());

    state.channel_recordings[1].state = ChannelRecordingState::Empty;
    assert!(!state.all_channels_recorded());
}

#[test]
fn test_recording_state_is_recording() {
    let mut state = RecordingState::default();
    assert!(!state.is_recording());

    state.current_recording_channel = Some(0);
    assert!(state.is_recording());
}

// ============================================================================
// LayoutMode Tests
// ============================================================================

#[test]
fn test_layout_mode_variants() {
    assert_ne!(LayoutMode::Compact, LayoutMode::Expanded);
}

// ============================================================================
// MeterDisplayMode Tests
// ============================================================================

#[test]
fn test_meter_display_mode_default() {
    assert_eq!(MeterDisplayMode::default(), MeterDisplayMode::Lufs);
}

// ============================================================================
// ContextMenuType Tests
// ============================================================================

#[test]
fn test_context_menu_type_variants() {
    let types = [
        ContextMenuType::Album,
        ContextMenuType::QueueItem,
        ContextMenuType::Plugin,
        ContextMenuType::Directory,
    ];
    assert_eq!(types.len(), 4);
}

// ============================================================================
// ReplayGainMode Tests
// ============================================================================

#[test]
fn test_replay_gain_mode_variants() {
    assert_ne!(ReplayGainMode::Track, ReplayGainMode::Album);
}

// ============================================================================
// PluginViewMode Tests
// ============================================================================

#[test]
fn test_plugin_view_mode_default() {
    assert_eq!(PluginViewMode::default(), PluginViewMode::Rack);
}

#[test]
fn test_plugin_view_mode_variants() {
    assert_ne!(PluginViewMode::Rack, PluginViewMode::Graph);
}

use sotf_audio_player::PluginType;
use sotf_audio_player_gpui::app::types::{DensityMode, HeadphoneEqState, SpinoramaEqState};
use sotf_audio_player_gpui::{
    App, CalibrationData, ChannelMapping, ChannelRecording, ChannelRecordingState, ContextMenuType,
    CrossoverType, InputMode, LayoutMode, LibraryStats, MeasureState, MeterDisplayMode,
    PlatformStyle, PlaybackDeviceConfig, PlotSmoothing, PluginViewMode, RecordingDeviceConfig,
    RecordingSignalType, RecordingState, RecordingStep, ReplayGainMode, RoomEqAlgorithm,
    RoomEqOptimizerConfig, RoomEqStep, Screen, SpeakerConfiguration, ToastMessage, ToastType,
    engine_stop_without_queue_should_clear, screen_shows_rack_data,
};

#[test]
fn test_screen_variants() {
    let screens = [
        Screen::Home,
        Screen::HomeShelf,
        Screen::NowPlaying,
        Screen::Library,
        Screen::Streams,
        Screen::Queue,
        Screen::Playlists,
        Screen::Spectrum,
        Screen::Settings,
        Screen::SettingsDetail,
        Screen::StudioHub,
        Screen::EqCurve,
        Screen::Studio,
        Screen::Recording,
        Screen::RoomEq,
        Screen::HeadphoneEq,
        Screen::Spinorama,
        Screen::PluginGraph,
    ];
    assert_eq!(screens.len(), 18);
    assert_ne!(Screen::Library, Screen::Queue);
}

#[test]
fn test_primary_information_architecture_destinations() {
    assert_eq!(
        Screen::primary_destinations(),
        &[
            Screen::Home,
            Screen::NowPlaying,
            Screen::Library,
            Screen::Streams,
            Screen::Queue,
            Screen::Studio
        ]
    );
    assert_eq!(Screen::NowPlaying.primary_destination_index(), 1);
    assert_eq!(Screen::Home.primary_destination_index(), 0);
    assert_eq!(Screen::HomeShelf.primary_destination_index(), 0);
    assert_eq!(Screen::Library.primary_destination_index(), 2);
    assert_eq!(Screen::Streams.primary_destination_index(), 3);
    assert_eq!(Screen::Queue.primary_destination_index(), 4);
    assert_eq!(Screen::Studio.primary_destination_index(), 5);
    assert_eq!(Screen::StudioHub.primary_destination_index(), 5);
    assert_eq!(Screen::EqCurve.primary_destination_index(), 5);
    assert_eq!(Screen::RoomEq.primary_destination_index(), 5);
    assert_eq!(Screen::PluginGraph.primary_destination_index(), 5);
    assert_eq!(Screen::Settings.primary_destination_index(), 0);
    assert_eq!(Screen::SettingsDetail.primary_destination_index(), 0);
}

#[test]
fn test_view_menu_ids_map_to_screens() {
    assert_eq!(Screen::from_view_menu_id("home"), Some(Screen::Home));
    assert_eq!(
        Screen::from_view_menu_id("now-playing"),
        Some(Screen::NowPlaying)
    );
    assert_eq!(Screen::from_view_menu_id("library"), Some(Screen::Library));
    assert_eq!(Screen::from_view_menu_id("streams"), Some(Screen::Streams));
    assert_eq!(Screen::from_view_menu_id("queue"), Some(Screen::Queue));
    assert_eq!(Screen::from_view_menu_id("studio"), Some(Screen::Studio));
    assert_eq!(
        Screen::from_view_menu_id("plugingraph"),
        Some(Screen::PluginGraph)
    );
    assert_eq!(
        Screen::from_view_menu_id("settings"),
        Some(Screen::Settings)
    );
    assert_eq!(
        Screen::from_view_menu_id("settings-detail"),
        Some(Screen::SettingsDetail)
    );
    assert_eq!(Screen::from_view_menu_id("unknown"), None);
}

#[test]
fn test_density_mode_layout_policy() {
    assert_eq!(
        DensityMode::Standard.layout_mode_for_window(1600.0, 1000.0),
        LayoutMode::Compact
    );
    assert_eq!(
        DensityMode::Expert.layout_mode_for_window(1600.0, 1000.0),
        LayoutMode::Expanded
    );
    assert_eq!(
        DensityMode::Expert.layout_mode_for_window(599.0, 1000.0),
        LayoutMode::Compact
    );
    assert_eq!(
        DensityMode::Expert.layout_mode_for_window(1600.0, 499.0),
        LayoutMode::Compact
    );
    assert_eq!(
        DensityMode::Expert.layout_mode_for_window(390.0, 844.0),
        LayoutMode::Compact
    );
    assert_eq!(
        DensityMode::Expert.layout_mode_for_window(844.0, 390.0),
        LayoutMode::Compact
    );
    assert_eq!(
        DensityMode::Expert.layout_mode_for_window(1024.0, 768.0),
        LayoutMode::Expanded
    );
}

#[test]
fn test_platform_style_phone_is_ios_only_and_iphone_sized() {
    assert_eq!(
        PlatformStyle::for_window(390.0, 844.0, true),
        PlatformStyle::Phone
    );
    assert_eq!(
        PlatformStyle::for_window(844.0, 390.0, true),
        PlatformStyle::Phone
    );
    assert_eq!(
        PlatformStyle::for_window(390.0, 844.0, false),
        PlatformStyle::Desktop
    );
    assert_eq!(
        PlatformStyle::for_window(768.0, 1024.0, true),
        PlatformStyle::Desktop
    );
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
fn test_tick_rack_data_is_screen_and_layout_gated() {
    assert!(screen_shows_rack_data(Screen::Studio, LayoutMode::Compact));
    assert!(screen_shows_rack_data(
        Screen::PluginGraph,
        LayoutMode::Compact
    ));
    assert!(screen_shows_rack_data(
        Screen::NowPlaying,
        LayoutMode::Expanded
    ));
    assert!(screen_shows_rack_data(
        Screen::Library,
        LayoutMode::Expanded
    ));
    assert!(screen_shows_rack_data(Screen::Queue, LayoutMode::Expanded));

    assert!(!screen_shows_rack_data(
        Screen::NowPlaying,
        LayoutMode::Compact
    ));
    assert!(!screen_shows_rack_data(
        Screen::Library,
        LayoutMode::Compact
    ));
    assert!(!screen_shows_rack_data(Screen::Queue, LayoutMode::Compact));
    assert!(!screen_shows_rack_data(
        Screen::Settings,
        LayoutMode::Expanded
    ));
    assert!(!screen_shows_rack_data(
        Screen::Spinorama,
        LayoutMode::Expanded
    ));
}

#[test]
fn test_engine_stop_without_queue_clear_predicate() {
    assert!(engine_stop_without_queue_should_clear(true, false, false));

    assert!(!engine_stop_without_queue_should_clear(false, false, false));
    assert!(!engine_stop_without_queue_should_clear(true, true, false));
    assert!(!engine_stop_without_queue_should_clear(true, false, true));
}

#[test]
fn test_headphone_eq_ui_loss_type_uses_optimizer_source_of_truth() {
    let state = HeadphoneEqState::default();
    assert_eq!(state.ui_loss_type(), "flat");
    assert_eq!(state.optimizer_config.loss, "flat");
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

    state.model.custom_target_path = " /tmp/custom-target.csv ".to_string();
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
    let original_plugin_count = app.plugin_state.graph.len();
    let snapshot = app.plugin_state.clone();

    let effect = app.plugin_state.add_plugin(&PluginType::Upmixer);
    assert!(matches!(
        effect,
        sotf_audio_player::PluginUpdateEffect::Structural
    ));
    assert_eq!(app.plugin_state.graph.len(), original_plugin_count + 1);

    app.rollback_failed_plugin_update(snapshot, "device only supports 2 channels");

    assert_eq!(app.plugin_state.graph.len(), original_plugin_count);
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
    assert_eq!(all.len(), 15);
    assert!(all.contains(&SpeakerConfiguration::Stereo));
    assert!(all.contains(&SpeakerConfiguration::Custom));
}

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

#[test]
fn test_recording_signal_type_as_str() {
    assert_eq!(RecordingSignalType::Sweep.as_str(), "Sweep");
    assert_eq!(RecordingSignalType::WhiteNoise.as_str(), "White Noise");
    assert_eq!(RecordingSignalType::PinkNoise.as_str(), "Pink Noise");
    assert_eq!(RecordingSignalType::Mls.as_str(), "MLS");
    assert_eq!(RecordingSignalType::Dirac.as_str(), "Dirac");
}

#[test]
fn test_recording_signal_type_all() {
    let all = RecordingSignalType::all();
    assert_eq!(all.len(), 5);
    assert!(all.contains(&RecordingSignalType::Sweep));
    assert!(all.contains(&RecordingSignalType::WhiteNoise));
    assert!(all.contains(&RecordingSignalType::PinkNoise));
    assert!(all.contains(&RecordingSignalType::Mls));
    assert!(all.contains(&RecordingSignalType::Dirac));
}

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

#[test]
fn test_room_eq_step_all() {
    let all = RoomEqStep::all();
    assert_eq!(all.len(), 7);
}

#[test]
fn test_room_eq_step_index() {
    assert_eq!(RoomEqStep::LoadData.index(), 0);
    assert_eq!(RoomEqStep::Delay.index(), 1);
    assert_eq!(RoomEqStep::Process.index(), 2);
    assert_eq!(RoomEqStep::Configure.index(), 3);
    assert_eq!(RoomEqStep::Optimize.index(), 4);
    assert_eq!(RoomEqStep::Review.index(), 5);
    assert_eq!(RoomEqStep::Export.index(), 6);
}

#[test]
fn test_room_eq_step_label() {
    assert_eq!(RoomEqStep::LoadData.label(), "Load Data");
    assert_eq!(RoomEqStep::Delay.label(), "Delay");
    assert_eq!(RoomEqStep::Process.label(), "Process");
    assert_eq!(RoomEqStep::Configure.label(), "Configure");
    assert_eq!(RoomEqStep::Export.label(), "Export");
}

#[test]
fn test_room_eq_step_next() {
    assert_eq!(RoomEqStep::LoadData.next(), Some(RoomEqStep::Delay));
    assert_eq!(RoomEqStep::Delay.next(), Some(RoomEqStep::Process));
    assert_eq!(RoomEqStep::Process.next(), Some(RoomEqStep::Configure));
    assert_eq!(RoomEqStep::Configure.next(), Some(RoomEqStep::Optimize));
    assert_eq!(RoomEqStep::Export.next(), None);
}

#[test]
fn test_room_eq_step_previous() {
    assert_eq!(RoomEqStep::LoadData.previous(), None);
    assert_eq!(RoomEqStep::Delay.previous(), Some(RoomEqStep::LoadData));
    assert_eq!(RoomEqStep::Process.previous(), Some(RoomEqStep::Delay));
    assert_eq!(RoomEqStep::Configure.previous(), Some(RoomEqStep::Process));
    assert_eq!(RoomEqStep::Export.previous(), Some(RoomEqStep::Review));
}

#[test]
fn test_crossover_type_all() {
    let all = CrossoverType::all();
    assert_eq!(all.len(), 6);
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

#[test]
fn test_room_eq_algorithm_all() {
    let all = RoomEqAlgorithm::all();
    assert_eq!(all.len(), 4);
    assert!(!all.contains(&RoomEqAlgorithm::NelderMead));
}

#[test]
fn test_room_eq_algorithm_as_str() {
    assert_eq!(RoomEqAlgorithm::Cobyla.as_str(), "COBYLA");
    assert_eq!(
        RoomEqAlgorithm::DifferentialEvolution.as_str(),
        "Differential Evolution"
    );
    assert_eq!(
        RoomEqAlgorithm::BayesianOptimization.as_str(),
        "Bayesian Optimization"
    );
    assert_eq!(RoomEqAlgorithm::CmaEs.as_str(), "CMA-ES");
}

#[test]
fn test_room_eq_algorithm_to_autoeq_string() {
    assert_eq!(RoomEqAlgorithm::Cobyla.to_autoeq_string(), "autoeq:cobyla");
    assert_eq!(
        RoomEqAlgorithm::DifferentialEvolution.to_autoeq_string(),
        "autoeq:de"
    );
    assert_eq!(
        RoomEqAlgorithm::BayesianOptimization.to_autoeq_string(),
        "autoeq:bo"
    );
    assert_eq!(RoomEqAlgorithm::CmaEs.to_autoeq_string(), "autoeq:cmaes");
}

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
    assert!((state.signal_level_db - -6.0206).abs() < 0.0001);
}

#[test]
fn test_room_eq_optimizer_config_default() {
    let config = RoomEqOptimizerConfig::default();
    assert_eq!(config.algorithm, "autoeq:cmaes");
    assert_eq!(config.num_filters, 7);
    assert!((config.min_q - 0.5).abs() < 0.001);
    assert!((config.max_q - 6.0).abs() < 0.001);
    assert_eq!(config.bo_initial_samples, 0);
    assert_eq!(config.bo_batch_size, 0);
    assert_eq!(config.bo_posterior_std_threshold, 0.0);
    assert_eq!(config.bo_acquisition, "qei");
    assert!(!config.bo_ehvi);
}

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
    for rec in &state.channel_recordings {
        assert_eq!(rec.mic_position_index, 0);
        assert_eq!(rec.mic_index, 0);
    }
}

#[test]
fn test_recording_state_init_multi_position_single_mic() {
    let mut state = RecordingState::default();
    state.playback_config.channel_mappings = vec![
        ChannelMapping::single(1, "L"),
        ChannelMapping::single(2, "R"),
    ];
    state.recording_config.num_positions = 2;
    state.recording_config.channel_mappings = vec![0];

    state.init_channel_recordings();

    assert_eq!(state.channel_recordings.len(), 4);
    let names: Vec<&str> = state
        .channel_recordings
        .iter()
        .map(|r| r.channel_name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["L (Pos 1)", "R (Pos 1)", "L (Pos 2)", "R (Pos 2)"]
    );
    let positions: Vec<usize> = state
        .channel_recordings
        .iter()
        .map(|r| r.mic_position_index)
        .collect();
    assert_eq!(positions, vec![0, 0, 1, 1]);
}

#[test]
fn test_recording_state_init_multi_position_multi_mic() {
    let mut state = RecordingState::default();
    state.playback_config.channel_mappings = vec![
        ChannelMapping::single(1, "L"),
        ChannelMapping::single(2, "R"),
    ];
    state.recording_config.num_positions = 2;
    state.recording_config.channel_mappings = vec![0, 1];

    state.init_channel_recordings();

    assert_eq!(state.channel_recordings.len(), 8);
    assert_eq!(
        state.channel_recordings[0].channel_name,
        "L (Pos 1 / Mic 1)"
    );
    assert_eq!(state.channel_recordings[0].mic_index, 0);
    assert_eq!(state.channel_recordings[0].mic_position_index, 0);
    assert_eq!(
        state.channel_recordings[7].channel_name,
        "R (Pos 2 / Mic 2)"
    );
    assert_eq!(state.channel_recordings[7].mic_index, 1);
    assert_eq!(state.channel_recordings[7].mic_position_index, 1);
}

#[test]
fn test_channel_speakers_are_per_playback_channel_for_save() {
    let mut state = RecordingState::default();
    state.playback_config.channel_mappings = vec![
        ChannelMapping::single(1, "L"),
        ChannelMapping::single(2, "R"),
    ];
    state.recording_config.num_positions = 2;
    state.recording_config.channel_mappings = vec![0, 1];

    state.init_channel_recordings();
    state.sync_channel_speakers_length();

    assert_eq!(state.channel_recordings.len(), 8);
    assert_eq!(state.channel_speakers.len(), 2);

    state.channel_speakers[0] = "Acme Left".to_string();
    state.channel_speakers[1] = "Acme Right".to_string();

    let saved = state.channel_speakers_map_for_save().unwrap();
    assert_eq!(saved.len(), 2);
    assert_eq!(saved.get("L").map(String::as_str), Some("Acme Left"));
    assert_eq!(saved.get("R").map(String::as_str), Some("Acme Right"));
    assert!(!saved.contains_key("L (Pos 1 / Mic 1)"));
}

#[test]
fn test_recording_state_position_helpers() {
    let mut state = RecordingState::default();
    state.playback_config.channel_mappings = vec![
        ChannelMapping::single(1, "L"),
        ChannelMapping::single(2, "R"),
    ];
    state.recording_config.num_positions = 2;
    state.init_channel_recordings();

    assert_eq!(state.current_position(), 0);
    assert!(!state.position_complete(0));
    assert_eq!(state.next_channel_in_position(0), Some(0));

    state.channel_recordings[0].state = ChannelRecordingState::Done;
    state.channel_recordings[1].state = ChannelRecordingState::Done;

    assert!(state.position_complete(0));
    assert_eq!(state.current_position(), 1);
    assert_eq!(state.next_channel_in_position(1), Some(2));
    assert!(!state.position_complete(1));
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

#[test]
fn test_layout_mode_variants() {
    assert_ne!(LayoutMode::Compact, LayoutMode::Expanded);
}

#[test]
fn test_meter_display_mode_default() {
    assert_eq!(MeterDisplayMode::default(), MeterDisplayMode::Lufs);
}

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

#[test]
fn test_replay_gain_mode_variants() {
    assert_ne!(ReplayGainMode::Track, ReplayGainMode::Album);
}

#[test]
fn test_plugin_view_mode_default() {
    assert_eq!(PluginViewMode::default(), PluginViewMode::Rack);
}

#[test]
fn test_plugin_view_mode_variants() {
    assert_ne!(PluginViewMode::Rack, PluginViewMode::Graph);
}

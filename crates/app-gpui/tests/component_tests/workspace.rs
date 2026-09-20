use sotf_audio_player::{TrialAnswer, TrialMode};

#[test]
fn headphone_target_labels_are_localized_in_both_workflow_steps() {
    use sotf_audio_player_gpui::app::i18n::{HeadphoneIdentityTranslations, Language};
    for (language, flat, custom) in [
        (Language::French, "Plat", "Personnalisé (fichier)"),
        (Language::German, "Linear", "Benutzerdefiniert (Datei)"),
        (Language::Spanish, "Plano", "Personalizado (archivo)"),
    ] {
        assert_eq!(
            HeadphoneIdentityTranslations::target_label(language, "flat", "Flat"),
            flat
        );
        assert_eq!(
            HeadphoneIdentityTranslations::target_label(language, "custom", "Custom (File Path)"),
            custom
        );
    }
    assert_ne!(
        HeadphoneIdentityTranslations::target_label(Language::Pseudo, "flat", "Flat"),
        "Flat"
    );
}

#[test]
fn headphone_file_identity_follows_the_active_source_and_path() {
    use sotf_audio_player::autoeq::headphone::HeadphoneMeasurementPreview;
    use sotf_audio_player_gpui::app::types::headphone_eq::{
        HeadphoneEqState, HeadphoneMeasurementSource,
    };
    use std::io::Write;
    let mut file = tempfile::NamedTempFile::new().unwrap();
    writeln!(file, "frequency,spl\n20,-4\n1000,0\n20000,-3").unwrap();
    let mut headphone = HeadphoneEqState::default();
    headphone.model.measurement_path = file.path().to_string_lossy().into_owned();
    headphone.file_preview = Some(std::sync::Arc::new(
        HeadphoneMeasurementPreview::load(file.path()).unwrap(),
    ));
    assert_eq!(
        headphone.measurement_frequency_bounds(),
        Some((20.0, 20000.0))
    );
    headphone.model.loading_download = true;
    assert!(!headphone.can_advance());
    headphone.model.loading_download = false;
    assert!(headphone.can_advance());
    headphone.model.measurement_source = HeadphoneMeasurementSource::Spinorama;
    assert!(headphone.active_file_preview().is_none());
    assert_eq!(headphone.measurement_frequency_bounds(), None);
    headphone.model.downloaded_curve = Some(vec![(30.0, 0.0), (18000.0, 1.0)]);
    assert_eq!(
        headphone.measurement_frequency_bounds(),
        Some((30.0, 18000.0))
    );
    headphone.model.measurement_source = HeadphoneMeasurementSource::File;
    headphone.model.measurement_path = "another-measurement.csv".into();
    assert!(headphone.active_file_preview().is_none());
    assert_eq!(headphone.measurement_frequency_bounds(), None);
}

#[test]
fn preference_search_routes_audio_source_only_when_supported() {
    use sotf_audio_player_gpui::app::types::{PreferencesSetting, SettingsTab};
    use sotf_audio_player_gpui::i18n::Language;
    let setting = PreferencesSetting::AudioSource;
    assert_eq!(setting.tab(), SettingsTab::AudioDevice);
    assert!(PreferencesSetting::ALL.contains(&setting));
    assert_eq!(
        setting.is_available(),
        cfg!(all(target_os = "macos", feature = "hal"))
    );
    assert!(setting.matches("systemwide input", Language::English));
    assert!(!setting.matches("remote playback devices", Language::English));
    for language in [
        Language::English,
        Language::French,
        Language::German,
        Language::Spanish,
    ] {
        assert!(setting.matches(setting.label(language), language));
    }
}

#[test]
fn preference_search_routes_connection_controls_without_account_values() {
    use sotf_audio_player_gpui::app::types::{PreferencesSetting, SettingsTab};
    use sotf_audio_player_gpui::i18n::Language;
    for (setting, tab, query) in [
        (
            PreferencesSetting::RemoteSources,
            SettingsTab::Federation,
            "remote music libraries provider accounts",
        ),
        (
            PreferencesSetting::RemotePlayers,
            SettingsTab::Federation,
            "remote playback devices",
        ),
        (
            PreferencesSetting::SharingServers,
            SettingsTab::Servers,
            "servers TLS authentication",
        ),
    ] {
        assert_eq!(setting.tab(), tab);
        assert!(PreferencesSetting::ALL.contains(&setting));
        assert!(setting.matches(query, Language::English));
        for language in [
            Language::English,
            Language::French,
            Language::German,
            Language::Spanish,
        ] {
            assert!(setting.matches(setting.label(language), language));
        }
        assert!(!setting.matches("private-test-token", Language::English));
    }
}

#[test]
fn preference_search_routes_playback_shortcuts_with_localized_labels() {
    use sotf_audio_player_gpui::app::types::{PreferencesSetting, SettingsTab};
    use sotf_audio_player_gpui::i18n::Language;
    let setting = PreferencesSetting::Shortcuts;
    assert_eq!(setting.tab(), SettingsTab::Keybindings);
    assert!(PreferencesSetting::ALL.contains(&setting));
    assert!(setting.matches("playback keyboard shortcuts", Language::English));
    assert!(!setting.matches("audio output device", Language::English));
    for language in [
        Language::English,
        Language::French,
        Language::German,
        Language::Spanish,
    ] {
        assert!(setting.matches(setting.label(language), language));
    }
}

#[test]
fn preference_search_routes_library_actions_and_playback_loudness() {
    use sotf_audio_player_gpui::app::types::{PreferencesSetting, SettingsTab};
    use sotf_audio_player_gpui::i18n::Language;
    for (setting, tab, query) in [
        (
            PreferencesSetting::ReplayGain,
            SettingsTab::AudioDevice,
            "replaygain playback",
        ),
        (
            PreferencesSetting::MusicFolders,
            SettingsTab::Library,
            "music folders",
        ),
        (
            PreferencesSetting::AudioAnalysis,
            SettingsTab::Library,
            "waveform similarity",
        ),
    ] {
        assert_eq!(setting.tab(), tab);
        assert!(PreferencesSetting::ALL.contains(&setting));
        assert!(setting.matches(query, Language::English));
        for language in [
            Language::English,
            Language::French,
            Language::German,
            Language::Spanish,
        ] {
            assert!(setting.matches(setting.label(language), language));
        }
    }
    assert!(!PreferencesSetting::AudioAnalysis.matches("replaygain playback", Language::English));
    assert!(!PreferencesSetting::ReplayGain.matches("waveform similarity", Language::English));
}

#[test]
fn output_device_search_routes_to_audio_and_matches_localized_labels() {
    use sotf_audio_player_gpui::app::types::{PreferencesSetting, SettingsTab};
    use sotf_audio_player_gpui::i18n::Language;
    let setting = PreferencesSetting::OutputDevice;
    assert_eq!(setting.tab(), SettingsTab::AudioDevice);
    assert!(setting.matches("output DAC", Language::English));
    assert!(!setting.matches("library folders", Language::English));
    for language in [
        Language::English,
        Language::French,
        Language::German,
        Language::Spanish,
    ] {
        assert!(setting.matches(setting.label(language), language));
    }
}

#[test]
fn preference_search_distinguishes_typography_settings_and_localized_names() {
    use sotf_audio_player_gpui::app::types::PreferencesSetting;
    use sotf_audio_player_gpui::i18n::Language;
    assert!(PreferencesSetting::MinimumFont.matches("MINIMUM font", Language::English));
    assert!(!PreferencesSetting::MaximumFont.matches("minimum font", Language::English));
    assert!(PreferencesSetting::TextSize.matches("textgröße", Language::German));
    assert!(PreferencesSetting::MinimumFont.matches("police minimale", Language::French));
    assert!(!PreferencesSetting::TextSize.matches("server port", Language::English));
}

#[test]
fn preference_search_routes_worker_and_cpu_controls_to_their_own_tabs() {
    use sotf_audio_player_gpui::app::types::{PreferencesSetting, SettingsTab};
    use sotf_audio_player_gpui::i18n::Language;
    assert_eq!(PreferencesSetting::ScanWorkers.tab(), SettingsTab::Library);
    assert_eq!(PreferencesSetting::CpuLimit.tab(), SettingsTab::Misc);
    assert!(PreferencesSetting::ScanWorkers.matches("scan workers", Language::English));
    assert!(!PreferencesSetting::CpuLimit.matches("scan workers", Language::English));
    assert!(PreferencesSetting::CpuLimit.matches("cpu limit", Language::English));
}

#[test]
fn preference_search_routes_accessibility_and_language_with_localized_labels() {
    use sotf_audio_player_gpui::app::types::{PreferencesSetting, SettingsTab};
    use sotf_audio_player_gpui::i18n::Language;
    assert_eq!(PreferencesSetting::ReduceMotion.tab(), SettingsTab::Theme);
    assert_eq!(PreferencesSetting::Language.tab(), SettingsTab::Language);
    for language in Language::all() {
        for setting in [
            PreferencesSetting::ReduceMotion,
            PreferencesSetting::Language,
        ] {
            assert!(setting.matches(setting.label(*language), *language));
        }
    }
    assert!(PreferencesSetting::ReduceMotion.matches("reduce motion", Language::English));
    assert!(!PreferencesSetting::Language.matches("reduce motion", Language::English));
    assert!(PreferencesSetting::Language.matches("locale", Language::English));
}

#[test]
fn preference_search_routes_theme_modes_and_external_plugin_discovery() {
    use sotf_audio_player_gpui::app::types::{PreferencesSetting, SettingsTab};
    use sotf_audio_player_gpui::i18n::Language;
    assert_eq!(PreferencesSetting::ThemeMode.tab(), SettingsTab::Theme);
    assert_eq!(PreferencesSetting::PluginDiscovery.tab(), SettingsTab::Misc);
    assert!(PreferencesSetting::ThemeMode.matches("scheduled theme", Language::English));
    assert!(PreferencesSetting::PluginDiscovery.matches("plugin discovery", Language::English));
    assert!(!PreferencesSetting::CpuLimit.matches("plugin discovery", Language::English));
    assert_eq!(
        PreferencesSetting::PluginDiscovery.is_available(),
        cfg!(any(
            target_os = "macos",
            target_os = "linux",
            target_os = "windows"
        ))
    );
    for language in Language::all() {
        for setting in [
            PreferencesSetting::ThemeMode,
            PreferencesSetting::PluginDiscovery,
        ] {
            assert!(setting.matches(setting.label(*language), *language));
        }
    }
}

#[test]
fn preference_search_finds_metadata_by_provider_and_localized_name() {
    use sotf_audio_player_gpui::app::types::{PreferencesSetting, SettingsTab};
    use sotf_audio_player_gpui::i18n::Language;
    assert_eq!(
        PreferencesSetting::MetadataSearch.tab(),
        SettingsTab::Metadata
    );
    assert!(PreferencesSetting::MetadataSearch.matches("MusicBrainz", Language::English));
    assert!(PreferencesSetting::MetadataSearch.matches("metadata search", Language::English));
    assert!(PreferencesSetting::MetadataSearch.matches("métadonnées", Language::French));
    assert!(PreferencesSetting::MetadataSearch.matches("Metadatendienste", Language::German));
    assert!(!PreferencesSetting::ScanWorkers.matches("metadata search", Language::English));
    for language in Language::all() {
        assert!(PreferencesSetting::MetadataSearch.matches(
            PreferencesSetting::MetadataSearch.label(*language),
            *language
        ));
    }
}

#[test]
fn metadata_cache_refreshes_on_preferences_entry_without_interrupting_a_save() {
    use sotf_audio_player_gpui::{App, Screen};
    let mut app = App::new();
    app.ui_state.current_screen = Screen::Home;
    app.settings.metadata_config = Some(Default::default());
    app.set_screen(Screen::Settings, "metadata-reopen");
    assert!(app.settings.metadata_config.is_none());
    app.settings.metadata_config = Some(Default::default());
    app.set_screen(Screen::Settings, "metadata-same-screen");
    assert!(app.settings.metadata_config.is_some());
    app.settings.metadata_loading = true;
    app.set_screen(Screen::Home, "metadata-save-leave");
    app.set_screen(Screen::Settings, "metadata-save-return");
    assert!(app.settings.metadata_config.is_some());
    assert!(app.settings.metadata_loading);
}

#[test]
fn preference_search_finds_feature_availability_in_supported_locales() {
    use sotf_audio_player_gpui::app::types::{PreferencesSetting, SettingsTab};
    use sotf_audio_player_gpui::i18n::Language;
    assert_eq!(
        PreferencesSetting::FeatureAvailability.tab(),
        SettingsTab::ReleaseChannel
    );
    for (language, query) in [
        (Language::English, "feature availability"),
        (Language::French, "disponibilité"),
        (Language::German, "Funktionsumfang"),
        (Language::Spanish, "disponibilidad"),
    ] {
        assert!(PreferencesSetting::FeatureAvailability.matches(query, language));
    }
    assert!(PreferencesSetting::FeatureAvailability.matches("beta alpha", Language::English));
    assert!(
        !PreferencesSetting::PluginDiscovery.matches("feature availability", Language::English)
    );
}

#[test]
fn leaving_library_search_restores_navigation_without_clearing_query() {
    use sotf_audio_player_gpui::{App, InputMode, Screen};
    let mut app = App::new();
    app.ui_state.current_screen = Screen::Library;
    app.library_state.search_query = "Retain this query".into();
    app.ui_state.input_mode = InputMode::Search;
    app.set_screen(Screen::Library, "same-screen-search");
    assert_eq!(app.ui_state.input_mode, InputMode::Search);
    app.set_screen(Screen::Home, "leave-search");
    assert_eq!(app.ui_state.input_mode, InputMode::Normal);
    assert_eq!(app.library_state.search_query, "Retain this query");
    app.set_screen(Screen::Settings, "preferences-after-search");
    assert_eq!(app.ui_state.input_mode, InputMode::Normal);
    assert_eq!(app.library_state.search_query, "Retain this query");
}

fn headphone_audition_app() -> sotf_audio_player_gpui::App {
    use sotf_audio_player_gpui::{App, HeadphoneEqStep, Screen};
    let mut app = App::new();
    app.ui_state.current_screen = Screen::HeadphoneEq;
    let headphone = &mut app.measurement_state.headphone_eq_state;
    headphone.model.step = HeadphoneEqStep::Listen;
    headphone.model.result = Some(
        serde_json::from_value(serde_json::json!({
            "biquads": [{"filter_type": "peak", "freq": 1000.0, "q": 1.0, "db_gain": 6.0}],
            "pre_score": 1.0, "post_score": 0.0
        }))
        .unwrap(),
    );
    headphone.result_inputs = Some(headphone.model.optimization_input_snapshot());
    app
}

#[test]
fn headphone_audition_is_removed_before_export_or_navigation() {
    use sotf_audio_player_gpui::{HeadphoneEqStep, Screen};
    let mut app = headphone_audition_app();
    let node_count = app.plugin_state.graph.nodes.len();
    app.preview_headphone_eq(true).unwrap();
    assert_eq!(app.plugin_state.graph.nodes.len(), node_count + 2);
    app.finish_headphone_audition_update(true);
    assert!(app.move_workflow_step(true));
    assert_eq!(
        app.measurement_state.headphone_eq_state.step,
        HeadphoneEqStep::Export
    );
    assert_eq!(app.plugin_state.graph.nodes.len(), node_count);
    app.finish_headphone_audition_update(true);
    assert!(app.measurement_state.headphone_eq_state.audition.is_none());
    app.measurement_state.headphone_eq_state.model.step = HeadphoneEqStep::Listen;
    app.preview_headphone_eq(true).unwrap();
    app.set_screen(Screen::Library, "audition-test");
    assert_eq!(app.plugin_state.graph.nodes.len(), node_count);
    app.finish_headphone_audition_update(true);
    assert!(app.measurement_state.headphone_eq_state.audition.is_none());
}

#[test]
fn headphone_audition_stale_inputs_stop_preview_and_failed_restore_can_retry() {
    let mut app = headphone_audition_app();
    app.preview_headphone_eq(true).unwrap();
    app.finish_headphone_audition_update(true);
    app.measurement_state.headphone_eq_state.result_inputs = None;
    assert!(app.preview_headphone_eq(false).is_err());
    app.synchronize_headphone_audition().unwrap();
    assert!(
        app.finish_headphone_audition_update(false),
        "failed restoration must pause playback"
    );
    app.plugin_state.update_state.pending_plugin_update = None;
    app.synchronize_headphone_audition().unwrap();
    assert!(
        app.plugin_state
            .update_state
            .pending_plugin_update
            .is_none(),
        "do not retry a failing restore every frame"
    );
    app.stop_headphone_audition().unwrap();
    app.finish_headphone_audition_update(true);
    assert!(app.measurement_state.headphone_eq_state.audition.is_none());
}

#[test]
fn headphone_audition_pending_graph_update_has_priority_over_parameter_edits() {
    use sotf_audio_player_gpui::app::types::PluginUpdateType;
    let mut app = headphone_audition_app();
    app.preview_headphone_eq(true).unwrap();
    app.plugin_state.update_state.pending_plugin_update = Some(PluginUpdateType::Parameter {
        plugin_index: 0,
        param_index: 0,
    });
    app.synchronize_headphone_audition().unwrap();
    assert!(matches!(
        app.plugin_state.update_state.pending_plugin_update,
        Some(PluginUpdateType::Structural)
    ));
}

#[test]
fn headphone_audition_rejects_callbacks_after_leaving_review() {
    use sotf_audio_player_gpui::Screen;
    let mut app = headphone_audition_app();
    let graph = serde_json::to_value(&app.plugin_state.graph).unwrap();
    app.ui_state.current_screen = Screen::Library;
    assert!(app.preview_headphone_eq(true).is_err());
    assert_eq!(
        serde_json::to_value(&app.plugin_state.graph).unwrap(),
        graph
    );
}

#[test]
fn recording_review_is_invalidated_by_capture_input_changes_and_retakes() {
    use sotf_audio_player_gpui::{ChannelMapping, ChannelRecordingState, RecordingState};
    let mut recording = RecordingState::default();
    recording.playback_config.channel_mappings = vec![ChannelMapping::single(0, "L")];
    recording.init_channel_recordings();
    recording.channel_recordings[0].state = ChannelRecordingState::Done;
    recording.channel_recordings[0].result = Some(
        serde_json::from_value(serde_json::json!({
            "channel": 0, "frequencies": [100.0], "magnitude_db": [0.0], "phase_deg": [0.0]
        }))
        .unwrap(),
    );
    assert!(!recording.all_takes_accepted());
    assert!(recording.accept_take(0));
    assert!(recording.all_takes_accepted());
    let original_end_hz = recording.channel_recordings[0].sweep_end_freq;
    recording.channel_recordings[0].sweep_end_freq = original_end_hz / 2.0;
    assert!(!recording.all_takes_accepted());
    assert!(!recording.accept_take(0));
    recording.channel_recordings[0].sweep_end_freq = original_end_hz;
    recording.invalidate_take(0);
    assert!(!recording.all_takes_accepted());
    assert!(recording.accept_take(0));
    recording.signal_duration_secs += 1.0;
    recording.channel_recordings[0].sweep_end_freq = original_end_hz / 2.0;
    assert!(!recording.all_takes_accepted());
    assert!(!recording.accept_take(0));
    recording.prepare_capture_inputs();
    assert_eq!(
        recording.channel_recordings[0].sweep_end_freq,
        original_end_hz / 2.0
    );
    assert!(!recording.all_takes_accepted());
    assert!(recording.channel_recordings[0].result.is_none());
}

#[test]
fn workflow_step_selector_preserves_validation_and_capture_initialization() {
    use sotf_audio_player_gpui::{App, ChannelMapping, RecordingStep, Screen};
    let mut app = App::new();
    app.ui_state.current_screen = Screen::Recording;
    app.measurement_state.recording_state.step = RecordingStep::Config;
    app.measurement_state.recording_state.recording_directory = None;
    assert!(!app.select_workflow_step(1));
    assert!(!app.select_workflow_step(2));
    app.measurement_state.recording_state.recording_directory =
        Some("/tmp/workflow-selector".into());
    app.measurement_state
        .recording_state
        .playback_config
        .channel_mappings = vec![ChannelMapping::single(0, "L")];
    assert!(app.select_workflow_step(1));
    assert_eq!(
        app.measurement_state.recording_state.step,
        RecordingStep::SplCalibration
    );
    assert_eq!(
        app.measurement_state
            .recording_state
            .channel_recordings
            .len(),
        1
    );
    assert!(app.select_workflow_step(2));
    assert_eq!(
        app.measurement_state.recording_state.step,
        RecordingStep::Capture
    );
    assert!(!app.select_workflow_step(3));
    assert!(!app.select_workflow_step(usize::MAX));
    assert!(app.select_workflow_step(0));
    assert_eq!(
        app.measurement_state.recording_state.step,
        RecordingStep::Config
    );
    assert_eq!(app.ui_state.current_screen, Screen::Recording);
}

#[test]
fn workflow_step_selector_cannot_leave_a_busy_capture() {
    use sotf_audio_player::recording_types::ProbeCaptureStatus;
    use sotf_audio_player_gpui::{App, RecordingStep, Screen};
    let mut app = App::new();
    app.ui_state.current_screen = Screen::Recording;
    app.measurement_state.recording_state.step = RecordingStep::Probe;
    app.measurement_state.recording_state.probe_capture.status =
        ProbeCaptureStatus::Running { started_at_ms: 0 };
    assert!(!app.select_workflow_step(0));
    assert!(!app.select_workflow_step(4));
    assert_eq!(
        app.measurement_state.recording_state.step,
        RecordingStep::Probe
    );
}

#[test]
fn recording_workflow_busy_includes_calibration_and_probe_jobs() {
    use sotf_audio_player::recording_types::{
        BassAnchorCaptureStatus, ProbeCaptureStatus, SplCalibrationCaptureStatus,
    };
    use sotf_audio_player::ui_models::recording::RecordingScreenModel;
    let mut model = RecordingScreenModel::default();
    assert!(!model.workflow_is_busy());
    model.probe_capture.status = ProbeCaptureStatus::Running { started_at_ms: 0 };
    assert!(model.workflow_is_busy());
    model.probe_capture.status = ProbeCaptureStatus::default();
    model.bass_anchor_capture.status = BassAnchorCaptureStatus::Running { started_at_ms: 0 };
    assert!(model.workflow_is_busy());
    model.bass_anchor_capture.status = BassAnchorCaptureStatus::default();
    model.spl_calibration_capture.status =
        SplCalibrationCaptureStatus::Running { started_at_ms: 0 };
    assert!(model.workflow_is_busy());
}

#[test]
fn routing_validation_rejects_missing_inputs_and_duplicate_connections() {
    use sotf_audio_player_gpui::app::state::plugin::PluginState;
    let mut plugins = PluginState::new();
    assert!(plugins.graph.validate_routing().is_ok());
    let edge = plugins.graph.connections.pop().unwrap();
    assert!(plugins.graph.validate_routing().is_err());
    plugins.graph.connections.push(edge.clone());
    assert!(plugins.graph.validate_routing().is_ok());
    plugins.graph.connections.push(edge);
    assert!(plugins.graph.validate_routing().is_err());
}

#[test]
fn capture_changes_invalidate_accepted_takes_but_save_metadata_does_not() {
    use sotf_audio_player_gpui::app::types::{ChannelRecordingState, RecordingState};
    let mut recording = RecordingState::default();
    recording.init_channel_recordings();
    for channel in &mut recording.channel_recordings {
        channel.state = ChannelRecordingState::Done;
    }
    assert!(recording.all_channels_recorded());
    recording.save_name = "another-session-name".into();
    assert!(recording.all_channels_recorded());
    recording.signal_level_db -= 3.0;
    assert!(!recording.all_channels_recorded());
    recording.prepare_capture_inputs();
    assert!(recording.capture_inputs_are_current());
    assert!(!recording.all_channels_recorded());
}

#[test]
fn audio_output_draft_keeps_selection_reversible() {
    use sotf_audio_player::ui_models::audio_preferences::AudioOutputDraft;
    let mut draft = AudioOutputDraft::default();
    draft.select("Headphones".into(), Some("Speakers"));
    assert_eq!(draft.selected_name(Some("Speakers")), Some("Headphones"));
    draft.error = Some("device disconnected".into());
    draft.discard();
    assert_eq!(draft.selected_name(Some("Speakers")), Some("Speakers"));
    assert!(draft.error.is_none());
    draft.select("Speakers".into(), Some("Speakers"));
    assert!(draft.device_name.is_none());
}

#[test]
fn routing_parameters_do_not_reach_the_active_graph_before_apply() {
    use sotf_audio_player::{PluginType, PluginUpdateEffect};
    use sotf_audio_player_gpui::app::state::plugin::PluginState;
    let mut plugins = PluginState::new();
    plugins.add_plugin(&PluginType::Gain);
    let id = plugins
        .graph
        .nodes
        .iter()
        .find(|(_, node)| node.plugin.plugin_type() == PluginType::Gain)
        .unwrap()
        .0
        .to_owned();
    let original = serde_json::to_value(&plugins.graph).unwrap();
    plugins.begin_routing_draft();
    plugins.graph_state.editing_graph_node_uuid = Some(id);
    let effect = plugins
        .routing_controller_mut()
        .set_plugin_param_by_node_id(id, 0, 6.0);
    assert!(!matches!(effect, PluginUpdateEffect::None));
    plugins.record_editor_effect(effect);
    assert_eq!(original, serde_json::to_value(&plugins.graph).unwrap());
    assert_ne!(
        original,
        serde_json::to_value(&plugins.routing_controller().graph).unwrap()
    );
    assert!(plugins.update_state.pending_plugin_update.is_none());
    plugins.discard_routing_draft();
    assert_eq!(
        original,
        serde_json::to_value(&plugins.routing_controller().graph).unwrap()
    );
}

#[test]
fn routing_draft_isolated_apply_discard_and_failed_apply() {
    use sotf_audio_player::PluginType;
    use sotf_audio_player_gpui::app::state::plugin::PluginState;
    let mut plugins = PluginState::new();
    let original = serde_json::to_value(&plugins.graph).unwrap();
    plugins.begin_routing_draft();
    plugins
        .routing_controller_mut()
        .add_plugin(&PluginType::Gain);
    let edited = serde_json::to_value(&plugins.routing_controller().graph).unwrap();
    assert_ne!(original, edited);
    assert_eq!(original, serde_json::to_value(&plugins.graph).unwrap());
    assert!(plugins.update_state.pending_plugin_update.is_none());
    plugins.discard_routing_draft();
    assert_eq!(
        original,
        serde_json::to_value(&plugins.routing_controller().graph).unwrap()
    );
    plugins.begin_routing_draft();
    plugins
        .routing_controller_mut()
        .add_plugin(&PluginType::Gain);
    plugins.apply_routing_draft().unwrap();
    assert_ne!(original, serde_json::to_value(&plugins.graph).unwrap());
    plugins.finish_routing_apply(false);
    assert_eq!(original, serde_json::to_value(&plugins.graph).unwrap());
    assert_ne!(
        original,
        serde_json::to_value(&plugins.routing_controller().graph).unwrap()
    );
    plugins.apply_routing_draft().unwrap();
    plugins.finish_routing_apply(true);
    assert_ne!(original, serde_json::to_value(&plugins.graph).unwrap());
}

#[test]
fn routing_rejected_apply_preserves_edits_made_while_pending() {
    use sotf_audio_player::PluginType;
    use sotf_audio_player_gpui::app::state::plugin::PluginState;

    let mut plugins = PluginState::new();
    let original = serde_json::to_value(&plugins.graph).unwrap();
    plugins
        .routing_controller_mut()
        .add_plugin(&PluginType::Gain);
    plugins.apply_routing_draft().unwrap();
    let submitted = serde_json::to_value(&plugins.graph).unwrap();

    plugins
        .routing_controller_mut()
        .add_plugin(&PluginType::Gain);
    let latest = serde_json::to_value(&plugins.routing_controller().graph).unwrap();
    assert_ne!(latest, submitted);
    assert!(plugins.apply_routing_draft().is_err());

    plugins.finish_routing_apply(false);
    assert_eq!(serde_json::to_value(&plugins.graph).unwrap(), original);
    assert_eq!(
        serde_json::to_value(&plugins.routing_controller().graph).unwrap(),
        latest
    );
    assert!(plugins.routing_draft_is_dirty());
    plugins.apply_routing_draft().unwrap();
    plugins.finish_routing_apply(true);
    assert_eq!(serde_json::to_value(&plugins.graph).unwrap(), latest);
    assert!(!plugins.routing_draft_is_dirty());
}

#[test]
fn matrix_custom_editor_uses_draft_for_existing_and_new_nodes() {
    use sotf_audio_player::{PluginSettings, PluginType, PluginUpdateEffect};
    use sotf_audio_player_gpui::app::state::plugin::PluginState;

    for draft_only in [false, true] {
        let mut plugins = PluginState::new();
        if !draft_only {
            plugins.add_plugin(&PluginType::Matrix);
        }
        let active = serde_json::to_value(&plugins.graph).unwrap();
        plugins.begin_routing_draft();
        if draft_only {
            plugins
                .routing_controller_mut()
                .add_plugin(&PluginType::Matrix);
        }
        let (node_id, instance_id) = plugins
            .routing_controller()
            .graph
            .nodes
            .iter()
            .find(|(_, node)| node.plugin.plugin_type() == PluginType::Matrix)
            .map(|(id, node)| (*id, node.plugin.id))
            .unwrap();
        plugins.graph_state.editing_graph_node_uuid = Some(node_id);
        let Some(PluginSettings::Matrix { matrix, .. }) =
            plugins.editor_settings_mut_by_instance_id(instance_id)
        else {
            panic!("Matrix editor must resolve draft node")
        };
        matrix[0] = 0.25;
        plugins.record_editor_effect(PluginUpdateEffect::Structural);
        assert_eq!(active, serde_json::to_value(&plugins.graph).unwrap());
        assert!(plugins.update_state.pending_plugin_update.is_none());
        let Some(PluginSettings::Matrix { matrix, .. }) =
            plugins.editor_settings_mut_by_instance_id(instance_id)
        else {
            panic!("Matrix draft must remain editable")
        };
        assert_eq!(matrix[0], 0.25);
        plugins.discard_routing_draft();
        assert_eq!(active, serde_json::to_value(&plugins.graph).unwrap());
    }
}

#[test]
fn matrix_rack_editor_still_updates_active_processing() {
    use sotf_audio_player::{PluginSettings, PluginType, PluginUpdateEffect};
    use sotf_audio_player_gpui::app::state::plugin::PluginState;
    let mut plugins = PluginState::new();
    plugins.add_plugin(&PluginType::Matrix);
    let instance_id = plugins
        .graph
        .nodes
        .values()
        .find(|node| node.plugin.plugin_type() == PluginType::Matrix)
        .unwrap()
        .plugin
        .id;
    let Some(PluginSettings::Matrix { matrix, .. }) =
        plugins.editor_settings_mut_by_instance_id(instance_id)
    else {
        panic!("Matrix rack editor must resolve active node")
    };
    matrix[0] = 0.5;
    plugins.record_editor_effect(PluginUpdateEffect::Structural);
    assert!(plugins.update_state.pending_plugin_update.is_some());
    assert!(plugins.graph_state.draft.is_none());
    let Some(PluginSettings::Matrix { matrix, .. }) =
        plugins.editor_settings_mut_by_instance_id(instance_id)
    else {
        panic!("Matrix rack node must remain available")
    };
    assert_eq!(matrix[0], 0.5);
}

#[test]
fn routing_apply_rejects_changes_to_the_active_chain() {
    use sotf_audio_player::PluginType;
    use sotf_audio_player_gpui::app::state::plugin::PluginState;
    let mut plugins = PluginState::new();
    plugins.begin_routing_draft();
    plugins
        .routing_controller_mut()
        .add_plugin(&PluginType::Gain);
    plugins.add_plugin(&PluginType::Limiter);
    let active = serde_json::to_value(&plugins.graph).unwrap();
    assert!(plugins.apply_routing_draft().is_err());
    assert_eq!(active, serde_json::to_value(&plugins.graph).unwrap());
}

#[test]
fn clean_routing_draft_does_not_schedule_an_audio_rebuild() {
    use sotf_audio_player::PluginType;
    use sotf_audio_player_gpui::app::state::plugin::PluginState;
    let mut plugins = PluginState::new();
    assert!(!plugins.routing_draft_is_dirty());
    plugins.begin_routing_draft();
    assert!(!plugins.routing_draft_is_dirty());
    plugins.apply_routing_draft().unwrap();
    assert!(plugins.update_state.pending_plugin_update.is_none());
    assert!(plugins.graph_state.applying_original.is_none());
    plugins
        .routing_controller_mut()
        .add_plugin(&PluginType::Gain);
    assert!(plugins.routing_draft_is_dirty());
    plugins.discard_routing_draft();
    assert!(!plugins.routing_draft_is_dirty());
}

#[test]
fn routing_add_then_remove_is_clean_without_rewinding_allocator() {
    use sotf_audio_player::{NodePosition, PluginType};
    use sotf_audio_player_gpui::app::state::plugin::PluginState;

    let mut plugins = PluginState::new();
    plugins.begin_routing_draft();
    let base = plugins.graph_state.draft_base.clone();
    let id = plugins
        .routing_controller_mut()
        .graph
        .add_plugin_node(&PluginType::Gain, NodePosition::new(100.0, 100.0))
        .unwrap();
    assert!(plugins.routing_draft_is_dirty());
    plugins.routing_controller_mut().graph.remove_node(id);
    assert!(!plugins.routing_draft_is_dirty());
    assert_eq!(plugins.graph_state.draft_base, base);
    assert_ne!(
        serde_json::to_value(&plugins.routing_controller().graph).ok(),
        base
    );
    plugins.apply_routing_draft().unwrap();
    assert!(plugins.update_state.pending_plugin_update.is_none());
    assert!(plugins.graph_state.applying_original.is_none());

    // Actual layout edits still require Apply, even after an allocator-only edit.
    plugins.routing_controller_mut().graph.canvas_zoom = 1.5;
    assert!(plugins.routing_draft_is_dirty());
}

#[test]
fn correction_input_identity_ignores_navigation_but_tracks_configuration() {
    use sotf_audio_player::ui_models::{
        headphone_eq::HeadphoneEqScreenModel, spinorama_eq::SpinoramaEqScreenModel,
    };
    let mut headphone = HeadphoneEqScreenModel::default();
    let original = headphone.optimization_input_snapshot();
    headphone.progress = 0.5;
    headphone.export_format = "changed-export-only".into();
    assert_eq!(original, headphone.optimization_input_snapshot());
    headphone.target_preset = "custom".into();
    assert_ne!(original, headphone.optimization_input_snapshot());

    let mut speaker = SpinoramaEqScreenModel::default();
    let original = speaker.optimization_input_snapshot();
    speaker.progress = 0.5;
    speaker.export_format = "changed-export-only".into();
    assert_eq!(original, speaker.optimization_input_snapshot());
    speaker.selected_version = "another-dataset-version".into();
    assert_ne!(original, speaker.optimization_input_snapshot());
}

#[test]
fn room_correction_results_become_stale_when_preset_changes() {
    use sotf_audio_player_gpui::app::types::RoomEqState;
    let mut room = RoomEqState::default();
    assert!(!room.result_is_current());
    room.result_inputs = Some(room.optimization_input_snapshot());
    assert!(room.result_is_current());
    room.export_format_index = 2;
    room.overall_progress = 1.0;
    assert!(room.result_is_current());
    room.optimizer_config.max_iter += 1;
    assert!(!room.result_is_current());
}
use sotf_audio_player_gpui::app::state::plugin::{
    ListeningWorkspacePhase, ListeningWorkspaceState,
};

#[test]
fn preference_categories_preserve_every_setting_route() {
    use sotf_audio_player_gpui::app::types::{PreferencesCategory, SettingsTab};
    let tabs: Vec<_> = PreferencesCategory::ALL
        .into_iter()
        .flat_map(|category| category.tabs().iter().copied())
        .collect();
    assert_eq!(tabs.len(), SettingsTab::ALL.len());
    for tab in SettingsTab::ALL {
        assert_eq!(
            tabs.iter().filter(|candidate| **candidate == tab).count(),
            1
        );
        assert!(PreferencesCategory::for_tab(tab).tabs().contains(&tab));
    }
    let ios_tabs: Vec<_> = PreferencesCategory::ALL
        .into_iter()
        .flat_map(|category| category.visible_tabs(true))
        .collect();
    for tab in SettingsTab::ALL {
        assert_eq!(
            ios_tabs.contains(&tab),
            SettingsTab::visible_tabs_for_ios(true).contains(&tab)
        );
    }
}

#[test]
fn preference_search_uses_setting_synonyms_without_secret_values() {
    use sotf_audio_player_gpui::app::types::{PreferencesCategory, SettingsTab};
    assert!(PreferencesCategory::matches(
        SettingsTab::AudioDevice,
        "sample rate",
        "Audio"
    ));
    assert!(PreferencesCategory::matches(
        SettingsTab::Theme,
        "zoom",
        "Appearance"
    ));
    assert!(PreferencesCategory::matches(
        SettingsTab::Servers,
        "password",
        "Connections"
    ));
    assert!(!PreferencesCategory::matches(
        SettingsTab::Servers,
        "an-actual-private-token",
        "Connections"
    ));
    assert!(!PreferencesCategory::matches(
        SettingsTab::Library,
        "sample rate",
        "Library"
    ));
}

#[test]
fn held_spectrum_preserves_data_when_live_frame_changes() {
    use sotf_audio_player_gpui::app::state::playback::HeldSpectrumFrame;
    use std::sync::Arc;
    let original = Arc::new(sotf_audio_player::SpectrumData {
        frequencies: Arc::new(vec![20.0, 1_000.0, 16_000.0]),
        magnitudes: Arc::from(vec![-40.0, -30.0, -50.0]),
        peak_magnitude: -30.0,
    });
    let held = HeldSpectrumFrame {
        data: original.clone(),
        sample_rate: Some(32_000),
    };
    let mut next = original.clone();
    Arc::make_mut(&mut next).update_magnitudes(&[-80.0, -70.0, -90.0]);
    assert_eq!(held.data.magnitudes.as_ref(), &[-40.0, -30.0, -50.0]);
    assert_eq!(held.sample_rate, Some(32_000));
}

#[test]
fn comparison_results_never_replace_an_active_blind_trial() {
    let state = ListeningWorkspaceState {
        results_open: true,
        ..Default::default()
    };
    for mode in [TrialMode::Abx, TrialMode::BlindAb] {
        assert_eq!(state.phase(Some(mode), 4), ListeningWorkspacePhase::Listen);
    }
    assert_eq!(state.phase(None, 4), ListeningWorkspacePhase::Results);
    assert_eq!(state.phase(None, 0), ListeningWorkspacePhase::Results);
}

#[test]
fn submitted_trials_stay_in_listening_until_results_are_requested() {
    let state = ListeningWorkspaceState::default();
    assert_eq!(state.phase(None, 0), ListeningWorkspacePhase::Setup);
    assert_eq!(state.phase(None, 1), ListeningWorkspacePhase::Listen);
    assert_eq!(
        state.phase(None, state.planned_trials),
        ListeningWorkspacePhase::Listen
    );
}

#[test]
fn answer_submission_requires_audition_and_the_current_test_mode() {
    let mut state = ListeningWorkspaceState {
        selected_answer: Some(TrialAnswer::A),
        ..Default::default()
    };
    assert!(!state.can_submit(TrialMode::Abx));
    state.auditioned = true;
    assert!(state.can_submit(TrialMode::Abx));
    state.paused = true;
    assert!(!state.can_submit(TrialMode::Abx));
    state.paused = false;
    state.confirm_end = true;
    assert!(state.interaction_locked());
    assert!(!state.can_submit(TrialMode::Abx));
    assert_eq!(
        state.phase(Some(TrialMode::Abx), 1),
        ListeningWorkspacePhase::Listen
    );
    state.confirm_end = false;
    assert!(state.can_submit(TrialMode::Abx));
    assert!(!state.can_submit(TrialMode::BlindAb));
    state.selected_answer = Some(TrialAnswer::First);
    assert!(state.can_submit(TrialMode::BlindAb));
    assert!(!state.can_submit(TrialMode::Abx));
    state.selected_answer = None;
    assert!(!state.can_submit(TrialMode::BlindAb));
}

#[test]
fn workspace_copy_is_available_for_every_supported_language() {
    use sotf_audio_player_gpui::app::i18n::{Language, Translations};
    for language in [
        Language::English,
        Language::French,
        Language::German,
        Language::Spanish,
        Language::Pseudo,
    ] {
        let labels = Translations::for_language(language)
            .listening_test
            .workspace();
        for label in Translations::for_language(language)
            .listening_test
            .end_confirmation()
        {
            assert!(!label.is_empty());
        }
        for label in Translations::for_language(language)
            .listening_test
            .disclosures()
        {
            assert!(!label.is_empty());
        }
        for label in [
            labels.setup,
            labels.listen,
            labels.results,
            labels.view_results,
            labels.next_trial,
            labels.new_comparison,
            labels.planned_trials,
            labels.locked,
            labels.answers_hidden,
            labels.submit,
        ] {
            assert!(!label.is_empty());
        }
    }
}

#[test]
fn primary_plugin_groups_do_not_disappear_during_resize_or_zoom() {
    use sotf_audio_player::{PluginSettings, PluginType};
    use sotf_audio_player_gpui::components::plugins::ui_layout_renderer::generated_layout_group_ids;
    for plugin in [
        PluginType::Gain,
        PluginType::Limiter,
        PluginType::Compressor,
        PluginType::Convolution,
    ] {
        let settings = PluginSettings::default_for(&plugin).unwrap();
        let reference = generated_layout_group_ids(&settings, 1000.0, 1.0);
        assert!(
            !reference.0.is_empty(),
            "{} must have a primary surface",
            plugin.name()
        );
        for zoom in [1.0, 1.25, 1.5, 2.0] {
            for width in [320.0, 480.0, 720.0, 1000.0] {
                assert_eq!(
                    generated_layout_group_ids(&settings, width, zoom),
                    reference,
                    "{} moved controls at width {width}, zoom {zoom}",
                    plugin.name()
                );
            }
        }
    }
}

#[test]
fn spectrum_inspection_uses_band_centers_and_display_smoothing() {
    use sotf_audio_player::SpectrumData;
    use sotf_audio_player_gpui::app::state::ui::SpectrumViewState;
    use std::sync::Arc;
    let data = SpectrumData {
        frequencies: Arc::new(vec![100.0, 200.0, 400.0, 800.0, 1600.0]),
        magnitudes: Arc::from([-100.0, -80.0, -60.0, -40.0, -20.0]),
        peak_magnitude: -20.0,
    };
    let (low, high) = SpectrumViewState::frequency_range(&data).unwrap();
    assert!((low - 100.0 / 2.0_f32.sqrt()).abs() < 0.001);
    assert!((high - 1600.0 * 2.0_f32.sqrt()).abs() < 0.001);
    assert!(Arc::ptr_eq(
        &data.magnitudes,
        &SpectrumViewState::magnitudes(&data, false)
    ));
    assert_eq!(
        &*SpectrumViewState::magnitudes(&data, true),
        &[-80.0, -70.0, -60.0, -50.0, -40.0]
    );
    let mut view = SpectrumViewState::default();
    view.inspect_frequency(&data, 790.0);
    assert_eq!(view.inspected_band(5), Some(3));
    view.cursor_fraction = Some(1.0);
    assert_eq!(view.inspected_band(5), Some(4));
    assert_eq!(view.inspected_band(0), None);
    view.cursor_fraction = Some(f32::NAN);
    assert_eq!(view.inspected_band(5), None);
}

#[test]
fn spectrum_inspection_rejects_malformed_frequency_metadata() {
    use sotf_audio_player::SpectrumData;
    use sotf_audio_player_gpui::app::state::ui::SpectrumViewState;
    use std::sync::Arc;
    for frequencies in [
        vec![],
        vec![100.0],
        vec![100.0, 100.0],
        vec![200.0, 100.0],
        vec![0.0, 100.0],
        vec![100.0, f32::INFINITY],
    ] {
        let data = SpectrumData {
            magnitudes: vec![-30.0; frequencies.len()].into(),
            frequencies: Arc::new(frequencies),
            peak_magnitude: -30.0,
        };
        assert!(SpectrumViewState::frequency_range(&data).is_none());
    }
    let data = SpectrumData {
        frequencies: Arc::new(vec![100.0, 200.0]),
        magnitudes: Arc::from([f32::NAN, f32::NEG_INFINITY]),
        peak_magnitude: -100.0,
    };
    assert!(SpectrumViewState::magnitudes(&data, true).is_empty());
    assert!(SpectrumViewState::magnitudes(&data, false).is_empty());
}

#[test]
fn spectrum_hold_is_idempotent_and_rejects_empty_capture() {
    use sotf_audio_player::SpectrumData;
    use sotf_audio_player_gpui::app::state::ui::SpectrumViewState;
    use std::sync::Arc;
    let mut view = SpectrumViewState::default();
    view.set_hold(true, None, Some(48000));
    assert!(!view.hold);
    let data = Arc::new(SpectrumData {
        frequencies: Arc::new(vec![100.0, 200.0]),
        magnitudes: Arc::from([-30.0, -40.0]),
        peak_magnitude: -30.0,
    });
    view.set_hold(true, Some(&data), Some(48000));
    view.set_hold(true, None, Some(96000));
    assert!(view.hold);
    let held = view.held_frame.as_ref().unwrap();
    assert_eq!(held.sample_rate, Some(48000));
    assert!(Arc::ptr_eq(&held.data, &data));
    view.set_hold(false, None, None);
    assert!(!view.hold);
    assert!(view.held_frame.is_none());
}

#[test]
fn listening_disclosure_keybindings_follow_overrides_and_displacement() {
    use gpui::Action;
    use sotf_audio_player_gpui::app::keybindings::{
        CustomKeybinding, KeymapPreset, get_documented_keybindings_for_screen_with_overrides,
    };
    use sotf_audio_player_gpui::app::{Screen, actions};
    let action_name = actions::ListeningToggleMetadata.name();
    let bindings = get_documented_keybindings_for_screen_with_overrides(
        Screen::ListeningTest,
        KeymapPreset::Default,
        &[CustomKeybinding {
            action_name: action_name.into(),
            key_spec: "ctrl-alt-j".into(),
        }],
    );
    let metadata = bindings
        .iter()
        .find(|binding| binding.action_name == Some(action_name))
        .expect("custom disclosure binding");
    assert_eq!(
        metadata.raw_key_spec,
        gpui::Keystroke::parse("ctrl-alt-j").unwrap().to_string()
    );
    let displaced = get_documented_keybindings_for_screen_with_overrides(
        Screen::ListeningTest,
        KeymapPreset::Default,
        &[CustomKeybinding {
            action_name: actions::PlayPause.name().into(),
            key_spec: "ctrl-alt-n".into(),
        }],
    );
    assert!(
        !displaced
            .iter()
            .any(|binding| binding.action_name == Some(action_name))
    );
}

#[test]
fn listening_runtime_rebuild_survives_cues_and_repeated_teardown_before_tick() {
    use sotf_audio_player::controllers::ab_compare_path::PathConfig;
    use sotf_audio_player::controllers::ab_test_session::{
        AbTestSession, ChainSnapshot, LevelMatchMeasurement, LevelMatchMetric, ListeningTestSetup,
        MediaSegment, TrialCue,
    };
    use sotf_audio_player_gpui::app::{state::plugin::PluginState, types::PluginUpdateType};
    let setup = ListeningTestSetup {
        path_a: ChainSnapshot::new("A", PathConfig::None).unwrap(),
        path_b: ChainSnapshot::new("B", PathConfig::None).unwrap(),
        media: MediaSegment {
            media_id: "fixture".into(),
            media_path: None,
            start_ms: 0,
            duration_ms: 3000,
        },
        sample_rate: 48000,
        channels: 2,
        level_match: LevelMatchMeasurement {
            metric: LevelMatchMetric::Rms,
            window_ms: 3000,
            path_a_db: -18.0,
            path_b_db: -18.0,
            correction_b_db: 0.0,
            tolerance_db: 0.1,
            max_correction_db: 6.0,
        },
        switch_transition_ms: 20.0,
        participant_id: None,
        app_version: "test".into(),
    };
    let mut plugins = PluginState::default();
    plugins
        .listening_test_state
        .ab_test
        .replace_session(AbTestSession::new("fixture", setup, 42).unwrap())
        .unwrap();
    plugins.start_ab_test_trial(TrialMode::Abx).unwrap();
    plugins.activate_ab_test_cue(TrialCue::Unknown).unwrap();
    assert!(matches!(
        plugins.update_state.pending_plugin_update,
        Some(PluginUpdateType::Structural)
    ));
    // Simulate publication, then a cue update followed by runtime removal.
    plugins.update_state.pending_plugin_update = None;
    plugins.activate_ab_test_cue(TrialCue::ReferenceA).unwrap();
    assert!(matches!(
        plugins.update_state.pending_plugin_update,
        Some(PluginUpdateType::Parameter { .. }) | Some(PluginUpdateType::ParameterByNodeId { .. })
    ));
    plugins.leave_ab_test_runtime().unwrap();
    plugins.leave_ab_test_runtime().unwrap();
    assert!(matches!(
        plugins.update_state.pending_plugin_update,
        Some(PluginUpdateType::Structural)
    ));
}

#[test]
fn album_play_next_keeps_queue_expansion_and_selection_aligned() {
    use sotf_audio_player::{Album, Track};
    use sotf_audio_player_gpui::App;
    let mut app = App::new();
    let album = |name: &str, count: usize| Album {
        title: name.into(),
        tracks: (0..count)
            .map(|i| Track {
                path: format!("/missing/{name}-{i}.flac").into(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    // GPUI tests enable the shared testing feature (no physical file requirement).
    app.queue_state.add_album(album("Current", 3)).unwrap();
    app.queue_state.add_album(album("Later", 1)).unwrap();
    app.queue_state.start();
    app.queue_state.expanded[0] = true;
    app.queue_state.selected_index = 1;
    let source = app.queue_state.current_track_source();
    assert_eq!(app.queue_state.enqueue_next(album("Next", 1)).unwrap(), 1);
    assert_eq!(app.queue_state.current_track_source(), source);
    assert_eq!(app.queue_state.expanded, vec![true, false, false, false]);
    assert_eq!(app.queue_state.selected_index, 3);
    assert_eq!(app.queue_state[3].album.title, "Later");
}

#[test]
fn studio_categories_remember_independent_tools() {
    use sotf_audio_player_gpui::{App, app::types::Screen};
    let mut app = App::new();
    for screen in [
        Screen::HeadphoneEq,
        Screen::Spectrum,
        Screen::Recording,
        Screen::Home,
    ] {
        app.ui_state.navigation.studio_picker_open = true;
        app.ui_state.navigation.studio_picker_highlight = Some(2);
        app.set_screen(screen, "StudioHistoryTest");
        assert_eq!(app.ui_state.current_screen, screen);
        assert!(!app.ui_state.navigation.studio_picker_open);
        assert_eq!(app.ui_state.navigation.studio_picker_highlight, None);
    }
    assert_eq!(
        app.ui_state.navigation.studio_target(0),
        Some(Screen::Spectrum)
    );
    assert_eq!(
        app.ui_state.navigation.studio_target(1),
        Some(Screen::Recording)
    );
    assert_eq!(
        app.ui_state.navigation.studio_target(2),
        Some(Screen::HeadphoneEq)
    );
    assert_eq!(
        app.ui_state.navigation.studio_target(3),
        Some(Screen::ListeningTest)
    );
    assert_eq!(app.ui_state.navigation.studio_target(4), None);
}

#[test]
fn correction_export_history_labels_cover_supported_languages() {
    use sotf_audio_player_gpui::app::i18n::{CorrectionDeliveryTranslations, Language};
    let english = CorrectionDeliveryTranslations::for_language(Language::English);
    for language in [
        Language::French,
        Language::German,
        Language::Spanish,
        Language::Pseudo,
    ] {
        let text = CorrectionDeliveryTranslations::for_language(language);
        assert_ne!(text.title, english.title);
        assert_ne!(text.current_export, english.current_export);
        assert_ne!(text.previous_export, english.previous_export);
        assert_ne!(text.not_exported, english.not_exported);
        assert_ne!(text.current_export, text.previous_export);
    }
}

#[test]
fn correction_application_labels_cover_supported_languages() {
    use sotf_audio_player_gpui::app::i18n::{CorrectionApplicationTranslations, Language};
    let english = CorrectionApplicationTranslations::for_language(Language::English);
    for language in [
        Language::French,
        Language::German,
        Language::Spanish,
        Language::Pseudo,
    ] {
        let text = CorrectionApplicationTranslations::for_language(language);
        assert_ne!(text.pending, english.pending);
        assert_ne!(text.applied, english.applied);
        assert_ne!(text.previous, english.previous);
        assert_ne!(text.changed, english.changed);
        assert_ne!(text.failed, english.failed);
        assert_ne!(text.not_applied, english.not_applied);
    }
}

fn snapshot_measurement(channel: &str) -> sotf_audio_player::room_eq_types::ChannelMeasurement {
    sotf_audio_player::room_eq_types::ChannelMeasurement {
        driver_measurement_sets: Vec::new(),
        provenance: Vec::new(),
        channel_name: channel.to_string(),
        measurement: sotf_audio_player::recording_types::RecordingResult {
            sample_rate_hz: None,
            channel: 0,
            frequencies: vec![100.0, 1000.0, 5000.0],
            magnitude_db: vec![70.0, 75.0, 72.0],
            phase_deg: Vec::new(),
            wav_path: None,
            csv_path: None,
            impulse_response: None,
            impulse_time_ms: None,
            thd_percent: None,
            harmonic_distortion_db: None,
            excess_group_delay_ms: None,
            rt60_ms: None,
            clarity_c50_db: None,
            clarity_c80_db: None,
            spectrogram_db: None,
            quality: None,
        },
        is_group: false,
        group_drivers: Vec::new(),
        multi_mic_measurements: Vec::new(),
    }
}

#[test]
fn room_snapshot_tracks_in_memory_positions_and_drivers_without_panicking() {
    use sotf_audio_player::room_eq_types::{RoomEqSpeakerConfig, RoomEqSpeakerConfigType};
    use sotf_audio_player_gpui::app::types::RoomEqState;
    let mut room = RoomEqState::default();
    room.channel_measurements.push(snapshot_measurement("L"));
    room.speaker_configs.push(RoomEqSpeakerConfig {
        channel_name: "L".into(),
        ..Default::default()
    });
    let original = room.optimization_input_snapshot();
    assert_eq!(original, room.optimization_input_snapshot());
    room.channel_measurements[0].measurement.magnitude_db[1] += 1.0;
    assert_ne!(original, room.optimization_input_snapshot());
    let extra = snapshot_measurement("L").measurement;
    room.channel_measurements[0]
        .multi_mic_measurements
        .push(extra.clone());
    let multiple = room.optimization_input_snapshot();
    room.channel_measurements[0].multi_mic_measurements[0].magnitude_db[1] += 1.0;
    assert_ne!(multiple, room.optimization_input_snapshot());
    room.speaker_configs[0].config_type = RoomEqSpeakerConfigType::MultiDriver;
    room.channel_measurements[0].is_group = true;
    room.channel_measurements[0].group_drivers.push(extra);
    let drivers = room.optimization_input_snapshot();
    room.channel_measurements[0].group_drivers[0].phase_deg = vec![0.0, 20.0, 40.0];
    assert_ne!(drivers, room.optimization_input_snapshot());
}

#[test]
fn systemwide_format_search_targets_are_precise_and_platform_gated() {
    use sotf_audio_player_gpui::app::types::{PreferencesSetting, SettingsTab};
    use sotf_audio_player_gpui::i18n::Language;
    for (setting, query) in [
        (
            PreferencesSetting::SystemwideSampleRate,
            "systemwide sample rate",
        ),
        (
            PreferencesSetting::SystemwideChannels,
            "systemwide channels",
        ),
        (
            PreferencesSetting::SystemwideBuffer,
            "systemwide buffer size",
        ),
    ] {
        assert!(PreferencesSetting::ALL.contains(&setting));
        assert_eq!(setting.tab(), SettingsTab::AudioDevice);
        assert!(setting.is_systemwide_format());
        assert_eq!(
            setting.is_available(),
            cfg!(all(target_os = "macos", feature = "hal"))
        );
        assert!(setting.matches(query, Language::English));
        assert!(!setting.matches("remote library", Language::English));
        for language in [
            Language::English,
            Language::French,
            Language::German,
            Language::Spanish,
            Language::Pseudo,
        ] {
            assert!(setting.matches(setting.label(language), language));
        }
    }
    assert!(!PreferencesSetting::AudioSource.is_systemwide_format());
    assert!(!PreferencesSetting::OutputDevice.is_systemwide_format());
}

#[test]
fn comparison_preparation_snapshot_tracks_all_measurement_inputs() {
    use sotf_audio_player::controllers::ab_compare_path::PathConfig;
    use sotf_audio_player::controllers::ab_test_session::LevelMatchMetric;
    use sotf_audio_player_gpui::app::state::plugin::ListeningTestState;
    let mut state = ListeningTestState::default();
    state.path_a = Some(PathConfig::Plugin {
        plugin_type: "Gain".into(),
        parameters: serde_json::json!({"gain_db": 0.0}),
    });
    state.path_b = state.path_a.clone();
    let path = std::path::PathBuf::from("source.wav");
    let expected = state.preparation_snapshot(Some(path.clone())).unwrap();
    type ChangeInput = fn(&mut ListeningTestState);
    let changes: [(&str, ChangeInput); 9] = [
        ("path A parameters", |s| {
            if let Some(PathConfig::Plugin { parameters, .. }) = &mut s.path_a {
                *parameters = serde_json::json!({"gain_db": -3.0});
            }
        }),
        ("path B structure", |s| {
            s.path_b = Some(PathConfig::Rack {
                plugins: Vec::new(),
            })
        }),
        ("path A label", |s| s.path_a_label.push_str(" edited")),
        ("path B label", |s| s.path_b_label.push_str(" edited")),
        ("excerpt", |s| s.segment_start_ms += 100),
        ("metric", |s| {
            s.level_match_config.metric = LevelMatchMetric::Rms
        }),
        ("window", |s| s.level_match_config.window_ms += 100),
        ("tolerance", |s| s.level_match_config.tolerance_db += 0.1),
        ("correction limit", |s| {
            s.level_match_config.max_correction_db += 1.0
        }),
    ];
    for (name, change) in changes {
        let mut edited = state.clone();
        change(&mut edited);
        assert_ne!(
            edited.preparation_snapshot(Some(path.clone())).as_ref(),
            Some(&expected),
            "{name}"
        );
    }
    assert_ne!(
        state
            .preparation_snapshot(Some("replacement.wav".into()))
            .as_ref(),
        Some(&expected)
    );
    assert!(state.preparation_snapshot(None).is_none());
    state.path_b = None;
    assert!(state.preparation_snapshot(Some(path)).is_none());
}

#[test]
fn comparison_preparation_only_accepts_latest_worker_and_rejects_reset_workers() {
    let mut workspace = ListeningWorkspaceState::default();
    let first = workspace.begin_preparation();
    let second = workspace.begin_preparation();
    assert!(!workspace.finish_preparation(&first));
    assert!(std::sync::Arc::ptr_eq(
        workspace.preparation_request.as_ref().unwrap(),
        &second
    ));
    assert!(workspace.finish_preparation(&second));
    assert!(!workspace.finish_preparation(&second));
    let before_reset = workspace.begin_preparation();
    workspace = ListeningWorkspaceState::default();
    let after_reset = workspace.begin_preparation();
    assert!(!workspace.finish_preparation(&before_reset));
    assert!(workspace.finish_preparation(&after_reset));
}

#[test]
fn comparison_cancelled_preparation_cannot_complete_or_consume_retry() {
    let mut workspace = ListeningWorkspaceState::default();
    let abandoned = workspace.begin_preparation();
    workspace.cancel_preparation();
    assert!(workspace.preparation_request.is_none());
    assert!(!workspace.finish_preparation(&abandoned));
    let retry = workspace.begin_preparation();
    assert!(!workspace.finish_preparation(&abandoned));
    assert!(workspace.finish_preparation(&retry));
}

#[test]
fn renaming_recording_copy_preserves_external_sources() {
    use sotf_audio_player_gpui::app::types::recording::RecordingState;
    use std::path::Path;
    for (original, expected) in [
        (
            "/measurements/original/take.wav",
            "/measurements/original/take.wav",
        ),
        (
            "/recordings/copy/subfolder/take.csv",
            "/recordings/renamed/subfolder/take.csv",
        ),
        (
            "/recordings/copy-other/take.csv",
            "/recordings/copy-other/take.csv",
        ),
    ] {
        let mut path = original.to_string();
        RecordingState::rebase_owned_path(
            &mut path,
            Path::new("/recordings/copy"),
            Path::new("/recordings/renamed"),
        );
        assert_eq!(path, expected);
    }
}

#[test]
fn imported_review_does_not_invent_capture_indices() {
    use sotf_audio_player::recording_types::RecordingImport;
    use sotf_audio_player_gpui::app::types::recording::RecordingState;
    let imported = RecordingImport::from_json(
        r#"{"version":"1.1.0","speakers":{"L":{"measurements":[{"name":"Seat","frequencies":[100,1000],"magnitude_db":[0,1]}]}}}"#,
        None,
    ).unwrap();
    let takes = imported.recordings();
    let mut state = RecordingState {
        imported_session: Some(imported),
        ..Default::default()
    };
    state.channel_recordings = takes.clone();
    assert!(state.has_imported_takes());
    assert_eq!(state.take_capture_indices(&takes[0]), (None, None));
    // Missing retained import state must not turn fallback zeros into real metadata.
    state.imported_session = None;
    assert_eq!(state.take_capture_indices(&takes[0]), (None, None));
    let captured = sotf_audio_player::recording_types::ChannelRecording::with_mic_position(
        0,
        "L".into(),
        1,
        2,
    );
    assert_eq!(state.take_capture_indices(&captured), (Some(1), Some(2)));
}

#[test]
fn imported_retake_routes_are_explicit_and_select_one_source() {
    use sotf_audio_player::recording_types::RecordingImport;
    use sotf_audio_player_gpui::app::types::recording::RecordingState;
    let imported = RecordingImport::from_json(
        r#"{"version":"1.1.0","speakers":{"L":{"measurements":[{"name":"Seat A","frequencies":[100,1000],"magnitude_db":[0,1]},{"name":"Seat B","frequencies":[100,1000],"magnitude_db":[2,3]}]}}}"#,
        None,
    ).unwrap();
    let mut state = RecordingState {
        model: sotf_audio_player::ui_models::recording::RecordingScreenModel {
            channel_recordings: imported.recordings(),
            recording_config: sotf_audio_player::recording_types::RecordingDeviceConfig {
                channel_mappings: vec![3],
                ..Default::default()
            },
            ..Default::default()
        },
        imported_session: Some(imported),
        ..Default::default()
    };
    assert_eq!(state.capture_take_indices(1), vec![1]);
    assert_eq!(state.validated_imported_route(1, 8, 4), None);
    state.select_imported_route(1, true, 5);
    assert_eq!(state.validated_imported_route(1, 8, 4), None);
    state.select_imported_route(1, false, 0);
    assert_eq!(state.validated_imported_route(1, 8, 4), Some((5, 0)));
    assert_eq!(state.validated_imported_route(0, 8, 4), None);
    assert_eq!(state.validated_imported_route(1, 5, 4), None);
    assert_eq!(state.validated_imported_route(1, 8, 3), None);
    state.playback_config.device_name = "Different interface".into();
    assert_eq!(state.validated_imported_route(1, 8, 4), None);
    state.select_imported_route(1, true, 2);
    assert_eq!(state.validated_imported_route(1, 8, 4), None);
    state.select_imported_route(1, false, 0);
    assert_eq!(state.validated_imported_route(1, 8, 4), Some((2, 0)));
    state.recording_config.channel_mappings[0] = 2;
    assert_eq!(state.validated_imported_route(1, 8, 4), None);
    assert!(state.accept_take(0));
    state.signal_level_db -= 3.0;
    assert!(state.take_review.is_accepted(&state.channel_recordings[0]));
    assert!(state.capture_inputs_are_current());
}

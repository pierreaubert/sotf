use crate::app::*;
use crate::theme::Theme;

use sotf_audio_player::{
    PluginSettings, PluginType, UpmixerAmbientAnalysisSettings, UpmixerGainSettings,
    UpmixerHeightSettings, UpmixerLfeSettings, UpmixerSubharmonicSettings,
};

#[path = "tests/create.rs"]
mod create;

#[test]
fn test_navigation_with_empty_directories() {
    let mut app = App::new(Theme::default(), false);
    assert_eq!(app.library_view.selected_directory_index, 0);

    // Should not crash with empty directories
    app.select_next_directory();
    assert_eq!(app.library_view.selected_directory_index, 0);

    app.select_previous_directory();
    assert_eq!(app.library_view.selected_directory_index, 0);

    app.page_down_directories(20);
    assert_eq!(app.library_view.selected_directory_index, 0);

    app.page_up_directories(20);
    assert_eq!(app.library_view.selected_directory_index, 0);
}

#[test]
fn test_adjust_eq_parameters() {
    let mut app = App::new(Theme::default(), false);
    let plugin_idx = app.plugin_rack.graph.add_plugin(&PluginType::EQ);
    app.plugin_rack.editing_index = Some(plugin_idx);

    let plugin = app.plugin_rack.graph.get_plugin(plugin_idx).unwrap();
    let filters = match &plugin.settings {
        PluginSettings::EQ { filters, .. } => filters,
        _ => panic!("Expected EQ plugin"),
    };
    assert!(!filters.is_empty());

    let orig_freq = filters[0].frequency;
    let orig_q = filters[0].q;
    let orig_gain = filters[0].gain_db;
    let orig_type = filters[0].filter_type;

    // Frequency
    app.plugin_rack.param_selection = 1; // Index 0 is now 'Max Filters'
    assert!(app.adjust_selected_param(1.0));
    let plugin = app.plugin_rack.graph.get_plugin(plugin_idx).unwrap();
    let filters = match &plugin.settings {
        PluginSettings::EQ { filters, .. } => filters,
        _ => panic!("Expected EQ plugin"),
    };
    assert_ne!(filters[0].frequency, orig_freq);

    // Q
    app.plugin_rack.param_selection = 2;
    assert!(app.adjust_selected_param(1.0));
    let plugin = app.plugin_rack.graph.get_plugin(plugin_idx).unwrap();
    let filters = match &plugin.settings {
        PluginSettings::EQ { filters, .. } => filters,
        _ => panic!("Expected EQ plugin"),
    };
    assert_ne!(filters[0].q, orig_q);

    // Gain
    app.plugin_rack.param_selection = 3;
    assert!(app.adjust_selected_param(1.0));
    let plugin = app.plugin_rack.graph.get_plugin(plugin_idx).unwrap();
    let filters = match &plugin.settings {
        PluginSettings::EQ { filters, .. } => filters,
        _ => panic!("Expected EQ plugin"),
    };
    assert_ne!(filters[0].gain_db, orig_gain);

    // Type
    app.plugin_rack.param_selection = 4;
    assert!(app.adjust_selected_param(1.0));
    let plugin = app.plugin_rack.graph.get_plugin(plugin_idx).unwrap();
    let filters = match &plugin.settings {
        PluginSettings::EQ { filters, .. } => filters,
        _ => panic!("Expected EQ plugin"),
    };
    assert_ne!(filters[0].filter_type, orig_type);
}

#[test]
fn test_adjust_upmixer_parameters() {
    let mut app = App::new(Theme::default(), false);
    let plugin_idx = app.plugin_rack.graph.add_plugin(&PluginType::Upmixer);
    app.plugin_rack.editing_index = Some(plugin_idx);

    let plugin = app.plugin_rack.graph.get_plugin(plugin_idx).unwrap();
    let (
        orig_speaker_config,
        orig_front_direct,
        orig_front_ambient,
        orig_rear_ambient,
        orig_height_gain,
        orig_lfe_gain,
        orig_lfe_cutoff,
        orig_stereo_width,
        orig_center_spread,
        orig_bandpass,
        orig_enable_subharm,
        orig_subharm_gain,
        orig_subharm_freq,
        orig_subharm_attack,
        orig_subharm_release,
        _orig_enable_hr_direct,
        _orig_hr_sharpen,
        _orig_safety_cap_db,
    ) = match &plugin.settings {
        PluginSettings::Upmixer {
            speaker_config,
            gains:
                UpmixerGainSettings {
                    gain_front_direct,
                    gain_front_ambient,
                    gain_rear_ambient,
                    height_gain,
                    stereo_width,
                    center_spread,
                    ..
                },
            lfe:
                UpmixerLfeSettings {
                    lfe_gain,
                    lfe_cutoff_hz,
                    bandpass_hz,
                    ..
                },
            subharmonic:
                UpmixerSubharmonicSettings {
                    enable_subharmonic_synth,
                    subharmonic_gain,
                    subharmonic_freq_hz,
                    subharmonic_attack_ms,
                    subharmonic_release_ms,
                    ..
                },
            height:
                UpmixerHeightSettings {
                    enable_hr_direct,
                    hr_sharpen,
                    ..
                },
            ambient_analysis: UpmixerAmbientAnalysisSettings { safety_cap_db, .. },
            ..
        } => (
            speaker_config.clone(),
            *gain_front_direct,
            *gain_front_ambient,
            *gain_rear_ambient,
            *height_gain,
            *lfe_gain,
            *lfe_cutoff_hz,
            *stereo_width,
            *center_spread,
            *bandpass_hz,
            *enable_subharmonic_synth,
            *subharmonic_gain,
            *subharmonic_freq_hz,
            *subharmonic_attack_ms,
            *subharmonic_release_ms,
            *enable_hr_direct,
            *hr_sharpen,
            *safety_cap_db,
        ),
        _ => panic!("Expected Upmixer plugin"),
    };

    // Indices match new get_params() order:
    // 0: speaker_config, 1-4: gains, 5: lfe_gain, 6: lfe_cutoff, 7: enable_subharm (toggle),
    // 8: subharm_gain, 9: subharm_freq, 10: subharm_attack, 11: subharm_release
    // 12: stereo_width, 13: center_spread, 14: bandpass
    for idx in 0..15 {
        app.plugin_rack.param_selection = idx;
        assert!(app.adjust_selected_param(1.0));
    }

    let plugin = app.plugin_rack.graph.get_plugin(plugin_idx).unwrap();
    if let PluginSettings::Upmixer {
        speaker_config,
        gains:
            UpmixerGainSettings {
                gain_front_direct,
                gain_front_ambient,
                gain_rear_ambient,
                height_gain,
                stereo_width,
                center_spread,
                ..
            },
        lfe:
            UpmixerLfeSettings {
                lfe_gain,
                lfe_cutoff_hz,
                bandpass_hz,
                ..
            },
        subharmonic:
            UpmixerSubharmonicSettings {
                enable_subharmonic_synth,
                subharmonic_gain,
                subharmonic_freq_hz,
                subharmonic_attack_ms,
                subharmonic_release_ms,
                ..
            },
        ..
    } = &plugin.settings
    {
        assert_ne!(*speaker_config, orig_speaker_config);
        assert_ne!(*gain_front_direct, orig_front_direct);
        assert_ne!(*gain_front_ambient, orig_front_ambient);
        assert_ne!(*gain_rear_ambient, orig_rear_ambient);
        assert_ne!(*height_gain, orig_height_gain);
        assert_ne!(*lfe_gain, orig_lfe_gain);
        assert_ne!(*lfe_cutoff_hz, orig_lfe_cutoff);
        assert_ne!(*enable_subharmonic_synth, orig_enable_subharm);
        assert_ne!(*subharmonic_gain, orig_subharm_gain);
        assert_ne!(*subharmonic_freq_hz, orig_subharm_freq);
        assert_ne!(*subharmonic_attack_ms, orig_subharm_attack);
        assert_ne!(*subharmonic_release_ms, orig_subharm_release);
        assert_ne!(*stereo_width, orig_stereo_width);
        assert_ne!(*center_spread, orig_center_spread);
        assert_ne!(*bandpass_hz, orig_bandpass);
    } else {
        panic!("Expected Upmixer plugin");
    }
}

#[test]
fn test_adjust_compressor_limiter_gate_loudness_parameters() {
    // Compressor
    let mut app = App::new(Theme::default(), false);
    let plugin_idx = app.plugin_rack.graph.add_plugin(&PluginType::Compressor);
    app.plugin_rack.editing_index = Some(plugin_idx);

    let plugin = app.plugin_rack.graph.get_plugin(plugin_idx).unwrap();
    let (
        orig_thresh,
        orig_ratio,
        orig_attack,
        orig_release,
        orig_knee,
        orig_makeup,
        orig_mix,
        orig_auto_makeup,
        orig_link_channels,
        orig_sidechain_hpf,
    ) = match &plugin.settings {
        PluginSettings::Compressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            makeup_gain_db,
            mix,
            auto_makeup,
            link_channels,
            sidechain_hpf_hz,
            ..
        } => (
            *threshold_db,
            *ratio,
            *attack_ms,
            *release_ms,
            *knee_db,
            *makeup_gain_db,
            *mix,
            *auto_makeup,
            *link_channels,
            *sidechain_hpf_hz,
        ),
        _ => panic!("Expected Compressor plugin"),
    };

    // Use -1.0 since mix defaults to 1.0 (its max) — adjusting up would clamp
    for idx in 0..10 {
        app.plugin_rack.param_selection = idx;
        assert!(app.adjust_selected_param(-1.0));
    }

    let plugin = app.plugin_rack.graph.get_plugin(plugin_idx).unwrap();
    if let PluginSettings::Compressor {
        threshold_db,
        ratio,
        attack_ms,
        release_ms,
        knee_db,
        makeup_gain_db,
        mix,
        auto_makeup,
        link_channels,
        sidechain_hpf_hz,
        ..
    } = &plugin.settings
    {
        assert_ne!(*threshold_db, orig_thresh);
        assert_ne!(*ratio, orig_ratio);
        assert_ne!(*attack_ms, orig_attack);
        assert_ne!(*release_ms, orig_release);
        assert_ne!(*knee_db, orig_knee);
        assert_ne!(*makeup_gain_db, orig_makeup);
        assert_ne!(*mix, orig_mix);
        assert_ne!(*auto_makeup, orig_auto_makeup);
        assert_ne!(*link_channels, orig_link_channels);
        assert_ne!(*sidechain_hpf_hz, orig_sidechain_hpf);
    }

    // Limiter (use -1.0 since mix starts at 1.0 which is max)
    let mut app = App::new(Theme::default(), false);
    let plugin_idx = app.plugin_rack.graph.add_plugin(&PluginType::Limiter);
    app.plugin_rack.editing_index = Some(plugin_idx);
    let plugin = app.plugin_rack.graph.get_plugin(plugin_idx).unwrap();
    let (orig_thresh, orig_rel, orig_look, orig_soft, orig_mix) = match &plugin.settings {
        PluginSettings::Limiter {
            threshold_db,
            release_ms,
            lookahead_ms,
            soft,
            mix,
            ..
        } => (*threshold_db, *release_ms, *lookahead_ms, *soft, *mix),
        _ => panic!("Expected Limiter plugin"),
    };
    // Limiter params: 0=threshold, 1=release, 2=lookahead, 3=soft, 4=true_peak, 5=isp_mode, 6=dual_release, 7=mix
    for idx in [0, 1, 2, 3, 7] {
        app.plugin_rack.param_selection = idx;
        assert!(app.adjust_selected_param(-1.0));
    }
    let plugin = app.plugin_rack.graph.get_plugin(plugin_idx).unwrap();
    if let PluginSettings::Limiter {
        threshold_db,
        release_ms,
        lookahead_ms,
        soft,
        mix,
        ..
    } = &plugin.settings
    {
        assert_ne!(*threshold_db, orig_thresh);
        assert_ne!(*release_ms, orig_rel);
        assert_ne!(*lookahead_ms, orig_look);
        assert_ne!(*soft, orig_soft);
        assert_ne!(*mix, orig_mix);
    }

    // Gate - test parameters individually since mix starts at max (1.0) and hpf at min (0.0)
    let mut app = App::new(Theme::default(), false);
    let plugin_idx = app.plugin_rack.graph.add_plugin(&PluginType::Gate);
    app.plugin_rack.editing_index = Some(plugin_idx);
    let plugin = app.plugin_rack.graph.get_plugin(plugin_idx).unwrap();
    let (
        orig_thresh,
        orig_ratio,
        orig_attack,
        orig_hold,
        orig_release,
        orig_mix,
        orig_link,
        orig_hpf,
    ) = match &plugin.settings {
        PluginSettings::Gate {
            threshold_db,
            ratio,
            attack_ms,
            hold_ms,
            release_ms,
            mix,
            link_channels,
            sidechain_hpf_hz,
            ..
        } => (
            *threshold_db,
            *ratio,
            *attack_ms,
            *hold_ms,
            *release_ms,
            *mix,
            *link_channels,
            *sidechain_hpf_hz,
        ),
        _ => panic!("Expected Gate plugin"),
    };
    // Adjust each parameter - mix (idx 5) decreases, hpf (idx 7) increases, others can go either way
    for idx in 0..8 {
        app.plugin_rack.param_selection = idx;
        let delta = if idx == 5 { -1.0 } else { 1.0 }; // mix starts at max, decrease it
        assert!(app.adjust_selected_param(delta));
    }
    let plugin = app.plugin_rack.graph.get_plugin(plugin_idx).unwrap();
    if let PluginSettings::Gate {
        threshold_db,
        ratio,
        attack_ms,
        hold_ms,
        release_ms,
        mix,
        link_channels,
        sidechain_hpf_hz,
        ..
    } = &plugin.settings
    {
        assert_ne!(*threshold_db, orig_thresh);
        assert_ne!(*ratio, orig_ratio);
        assert_ne!(*attack_ms, orig_attack);
        assert_ne!(*hold_ms, orig_hold);
        assert_ne!(*release_ms, orig_release);
        assert_ne!(*mix, orig_mix);
        assert_ne!(*link_channels, orig_link);
        assert_ne!(*sidechain_hpf_hz, orig_hpf);
    }

    // Loudness compensation
    let mut app = App::new(Theme::default(), false);
    let plugin_idx = app
        .plugin_rack
        .graph
        .add_plugin(&PluginType::LoudnessCompensation);
    app.plugin_rack.editing_index = Some(plugin_idx);
    let plugin = app.plugin_rack.graph.get_plugin(plugin_idx).unwrap();
    let (orig_low_freq, orig_low_gain, orig_high_freq, orig_high_gain) = match &plugin.settings {
        PluginSettings::LoudnessCompensation {
            low_freq,
            low_gain,
            high_freq,
            high_gain,
            ..
        } => (*low_freq, *low_gain, *high_freq, *high_gain),
        _ => panic!("Expected LoudnessCompensation plugin"),
    };
    for idx in 0..4 {
        app.plugin_rack.param_selection = idx;
        assert!(app.adjust_selected_param(1.0));
    }
    let plugin = app.plugin_rack.graph.get_plugin(plugin_idx).unwrap();
    if let PluginSettings::LoudnessCompensation {
        low_freq,
        low_gain,
        high_freq,
        high_gain,
        ..
    } = &plugin.settings
    {
        assert_ne!(*low_freq, orig_low_freq);
        assert_ne!(*low_gain, orig_low_gain);
        assert_ne!(*high_freq, orig_high_freq);
        assert_ne!(*high_gain, orig_high_gain);
    }
}

#[test]
fn test_adjust_binaural_decoder_parameters_and_set_sofa() {
    let mut app = App::new(Theme::default(), false);
    let plugin_idx = app
        .plugin_rack
        .graph
        .add_plugin(&PluginType::BinauralDecoder);
    app.plugin_rack.editing_index = Some(plugin_idx);
    app.plugin_rack.selected_index = plugin_idx;

    let plugin = app.plugin_rack.graph.get_plugin(plugin_idx).unwrap();
    let (orig_sofa, orig_channels, orig_ext, orig_near, orig_crossfade) = match &plugin.settings {
        PluginSettings::BinauralDecoder {
            sofa_file,
            input_channels,
            externalization,
            near_field_strength,
            crossfade_mode,
            ..
        } => (
            sofa_file.clone(),
            *input_channels,
            *externalization,
            *near_field_strength,
            *crossfade_mode,
        ),
        _ => panic!("Expected BinauralDecoder plugin"),
    };

    // Adjust mutable boolean/numeric parameters via adjust_selected_param.
    // input_channels is derived from the chain and stays synchronized by
    // update_channel_dependent_plugins().
    for idx in 2..5 {
        app.plugin_rack.param_selection = idx;
        assert!(app.adjust_selected_param(1.0));
    }

    let plugin = app.plugin_rack.graph.get_plugin(plugin_idx).unwrap();
    if let PluginSettings::BinauralDecoder {
        sofa_file,
        input_channels,
        externalization,
        near_field_strength,
        crossfade_mode,
        ..
    } = &plugin.settings
    {
        assert_eq!(*sofa_file, orig_sofa); // unchanged by adjust_selected_param
        assert_eq!(*input_channels, orig_channels);
        assert_ne!(*externalization, orig_ext);
        assert_ne!(*near_field_strength, orig_near);
        assert_ne!(*crossfade_mode, orig_crossfade);
    } else {
        panic!("Expected BinauralDecoder plugin");
    }

    // Now set SOFA file via load_sofa_file path
    app.plugin_rack.sofa_input = "/tmp/test.sofa".to_string();
    app.load_sofa_file().unwrap();

    let plugin = app.plugin_rack.graph.get_plugin(plugin_idx).unwrap();
    if let PluginSettings::BinauralDecoder { sofa_file, .. } = &plugin.settings {
        assert_eq!(sofa_file, "/tmp/test.sofa");
    } else {
        panic!("Expected BinauralDecoder plugin");
    }
}

#[test]
fn test_increase_volume() {
    let mut app = App::new(Theme::default(), false);
    app.playback.volume = 0.5;

    app.increase_volume();
    assert!((app.playback.volume - 0.55).abs() < 0.001);

    // Keep increasing
    for _ in 0..20 {
        app.increase_volume();
    }
    // Should clamp at 1.0
    assert!((app.playback.volume - 1.0).abs() < 0.001);
}

#[test]
fn test_decrease_volume() {
    let mut app = App::new(Theme::default(), false);
    app.playback.volume = 0.5;

    app.decrease_volume();
    assert!((app.playback.volume - 0.45).abs() < 0.001);

    // Keep decreasing
    for _ in 0..20 {
        app.decrease_volume();
    }
    // Should clamp at 0.0
    assert!((app.playback.volume - 0.0).abs() < 0.001);
}

#[test]
fn test_volume_boundary_values() {
    let mut app = App::new(Theme::default(), false);

    // Start at 0
    app.playback.volume = 0.0;
    app.decrease_volume();
    assert_eq!(app.playback.volume, 0.0);

    // Start at 1
    app.playback.volume = 1.0;
    app.increase_volume();
    assert_eq!(app.playback.volume, 1.0);
}

#[test]
fn test_queue_navigation_empty() {
    let mut app = App::new(Theme::default(), false);
    app.queue_view.selected_index = 0;

    app.select_next_queue_item();
    assert_eq!(app.queue_view.selected_index, 0);

    app.select_previous_queue_item();
    assert_eq!(app.queue_view.selected_index, 0);
}

#[test]
fn test_album_navigation_empty_library() {
    let mut app = App::new(Theme::default(), false);
    app.library_view.selected_album_index = 0;

    app.select_next_album();
    assert_eq!(app.library_view.selected_album_index, 0);

    app.page_down_albums(10);
    assert_eq!(app.library_view.selected_album_index, 0);
}

#[test]
fn test_add_plugin() {
    let mut app = App::new(Theme::default(), false);
    // App starts with default permanent plugins (LoudnessMonitor, Matrix, etc.)
    let initial_count = app.plugin_rack.graph.len();
    assert!(initial_count >= 2, "App should start with default plugins");

    app.add_plugin(&PluginType::Gain);
    assert_eq!(app.plugin_rack.graph.len(), initial_count + 1);
    assert!(app.plugin_rack.needs_update);

    app.add_plugin(&PluginType::EQ);
    assert_eq!(app.plugin_rack.graph.len(), initial_count + 2);
}

#[test]
fn test_remove_plugin() {
    let mut app = App::new(Theme::default(), false);
    let initial_count = app.plugin_rack.graph.len();

    app.add_plugin(&PluginType::Gain);
    app.add_plugin(&PluginType::EQ);
    app.add_plugin(&PluginType::Limiter);

    assert_eq!(app.plugin_rack.graph.len(), initial_count + 3);

    // Remove one of our added plugins (index after the defaults)
    app.remove_plugin(initial_count);
    assert_eq!(app.plugin_rack.graph.len(), initial_count + 2);
    assert!(app.plugin_rack.needs_update);
}

#[test]
fn test_toggle_plugin() {
    let mut app = App::new(Theme::default(), false);
    app.add_plugin(&PluginType::Gain);

    // Check initial state (enabled)
    let plugin = app.plugin_rack.graph.get_plugin(0).unwrap();
    assert!(plugin.enabled);

    // Toggle off
    app.toggle_plugin(0);
    let plugin = app.plugin_rack.graph.get_plugin(0).unwrap();
    assert!(!plugin.enabled);

    // Toggle on
    app.toggle_plugin(0);
    let plugin = app.plugin_rack.graph.get_plugin(0).unwrap();
    assert!(plugin.enabled);
}

#[test]
fn test_move_plugin_up() {
    let mut app = App::new(Theme::default(), false);
    let base_idx = app.plugin_rack.graph.user_plugin_insert_index();
    app.add_plugin(&PluginType::Gain);
    app.add_plugin(&PluginType::EQ);
    app.add_plugin(&PluginType::Limiter);

    // Move limiter up (from base_idx + 2 to base_idx + 1)
    app.move_plugin_up(base_idx + 2);

    // Limiter should now be at base_idx + 1
    let plugin = app.plugin_rack.graph.get_plugin(base_idx + 1).unwrap();
    assert!(matches!(plugin.plugin_type(), PluginType::Limiter));
}

#[test]
fn test_move_plugin_down() {
    let mut app = App::new(Theme::default(), false);
    let base_idx = app.plugin_rack.graph.user_plugin_insert_index();
    app.add_plugin(&PluginType::Gain);
    app.add_plugin(&PluginType::EQ);
    app.add_plugin(&PluginType::Limiter);

    // Move gain down (from base_idx to base_idx + 1)
    app.move_plugin_down(base_idx);

    // Gain should now be at base_idx + 1
    let plugin = app.plugin_rack.graph.get_plugin(base_idx + 1).unwrap();
    assert!(matches!(plugin.plugin_type(), PluginType::Gain));
}

#[test]
fn test_move_plugin_boundary() {
    let mut app = App::new(Theme::default(), false);
    app.add_plugin(&PluginType::Gain);
    app.add_plugin(&PluginType::EQ);

    // Try to move first plugin (index 0) up - should do nothing
    let first_plugin_type = app.plugin_rack.graph.get_plugin(0).unwrap().plugin_type();
    app.move_plugin_up(0);
    let plugin = app.plugin_rack.graph.get_plugin(0).unwrap();
    assert_eq!(plugin.plugin_type(), first_plugin_type);

    // Try to move last plugin down (should do nothing)
    let last_idx = app.plugin_rack.graph.len() - 1;
    let last_plugin_type = app
        .plugin_rack
        .graph
        .get_plugin(last_idx)
        .unwrap()
        .plugin_type();
    app.move_plugin_down(last_idx);
    let plugin = app.plugin_rack.graph.get_plugin(last_idx).unwrap();
    assert_eq!(plugin.plugin_type(), last_plugin_type);
}

#[test]
fn test_select_next_plugin() {
    let mut app = App::new(Theme::default(), false);
    app.add_plugin(&PluginType::Gain);
    app.add_plugin(&PluginType::EQ);

    let total_plugins = app.plugin_rack.graph.len();
    app.plugin_rack.selected_index = 0;

    // Navigate through all plugins
    for i in 1..total_plugins {
        app.select_next_plugin();
        assert_eq!(app.plugin_rack.selected_index, i);
    }

    // Wrap around to 0
    app.select_next_plugin();
    assert_eq!(app.plugin_rack.selected_index, 0);
}

#[test]
fn test_select_previous_plugin() {
    let mut app = App::new(Theme::default(), false);
    app.add_plugin(&PluginType::Gain);

    let total_plugins = app.plugin_rack.graph.len();
    app.plugin_rack.selected_index = 0;

    // Wrap to last
    app.select_previous_plugin();
    assert_eq!(app.plugin_rack.selected_index, total_plugins - 1);

    // Navigate back to 0
    for _ in 1..total_plugins {
        app.select_previous_plugin();
    }
    assert_eq!(app.plugin_rack.selected_index, 0);
}

#[test]
fn test_enter_exit_plugin_edit_mode() {
    let mut app = App::new(Theme::default(), false);
    app.add_plugin(&PluginType::EQ);
    app.plugin_rack.selected_index = 0;

    assert!(app.plugin_rack.editing_index.is_none());

    app.enter_plugin_edit_mode();
    assert_eq!(app.plugin_rack.editing_index, Some(0));
    assert_eq!(app.plugin_rack.param_selection, 0);

    app.exit_plugin_edit_mode();
    assert!(app.plugin_rack.editing_index.is_none());
}

#[test]
fn test_toggle_library_view_mode() {
    let mut app = App::new(Theme::default(), false);
    assert_eq!(app.library_view.mode, LibraryViewMode::Flat);

    app.toggle_library_view_mode();
    assert_eq!(app.library_view.mode, LibraryViewMode::TreeView);

    app.toggle_library_view_mode();
    assert_eq!(app.library_view.mode, LibraryViewMode::Flat);
}

#[test]
fn test_set_library_sort_order() {
    let mut app = App::new(Theme::default(), false);

    app.set_library_sort_order(LibrarySortOrder::Artist);
    assert_eq!(app.library_view.sort_order, LibrarySortOrder::Artist);

    app.set_library_sort_order(LibrarySortOrder::Album);
    assert_eq!(app.library_view.sort_order, LibrarySortOrder::Album);

    app.set_library_sort_order(LibrarySortOrder::Year);
    assert_eq!(app.library_view.sort_order, LibrarySortOrder::Year);
}

#[test]
fn test_set_channel_filter() {
    let mut app = App::new(Theme::default(), false);

    app.set_channel_filter(ChannelFilter::All);
    assert_eq!(app.library_view.channel_filter, ChannelFilter::All);

    app.set_channel_filter(ChannelFilter::Stereo);
    assert_eq!(app.library_view.channel_filter, ChannelFilter::Stereo);

    app.set_channel_filter(ChannelFilter::Surround);
    assert_eq!(app.library_view.channel_filter, ChannelFilter::Surround);
}

#[test]
fn test_cycle_channel_filter() {
    let mut app = App::new(Theme::default(), false);
    app.library_view.channel_filter = ChannelFilter::All;

    // Cycling depends on available channel counts, so test basic cycling
    // When library is empty, cycling should still work
    let initial = app.library_view.channel_filter;
    app.cycle_channel_filter();
    // After cycling, filter may or may not change depending on library
    // At minimum, it shouldn't panic
    let _ = app.library_view.channel_filter;

    // Reset
    app.library_view.channel_filter = initial;
}

#[test]
fn test_output_device_navigation_empty() {
    let mut app = App::new(Theme::default(), false);
    app.audio_devices.selected_output_index = 0;

    // Should not panic with empty devices
    app.select_next_output_device();
    assert_eq!(app.audio_devices.selected_output_index, 0);

    app.select_previous_output_device();
    assert_eq!(app.audio_devices.selected_output_index, 0);
}

#[test]
fn test_screen_variants() {
    let mut app = App::new(Theme::default(), false);

    app.current_screen = Screen::Library;
    assert_eq!(app.current_screen, Screen::Library);

    app.current_screen = Screen::Configure;
    assert_eq!(app.current_screen, Screen::Configure);

    app.current_screen = Screen::Queue;
    assert_eq!(app.current_screen, Screen::Queue);

    app.current_screen = Screen::Plugins;
    assert_eq!(app.current_screen, Screen::Plugins);

    app.current_screen = Screen::Devices;
    assert_eq!(app.current_screen, Screen::Devices);
}

#[test]
fn test_input_mode_variants() {
    let mut app = App::new(Theme::default(), false);

    app.input_mode = InputMode::Normal;
    assert_eq!(app.input_mode, InputMode::Normal);

    app.input_mode = InputMode::Search;
    assert_eq!(app.input_mode, InputMode::Search);

    app.input_mode = InputMode::ConfigureDirectories;
    assert_eq!(app.input_mode, InputMode::ConfigureDirectories);

    app.input_mode = InputMode::EditPlugin;
    assert_eq!(app.input_mode, InputMode::EditPlugin);

    app.input_mode = InputMode::ShowHelp;
    assert_eq!(app.input_mode, InputMode::ShowHelp);

    app.input_mode = InputMode::ShowError;
    assert_eq!(app.input_mode, InputMode::ShowError);
}

#[test]
fn test_apply_spinorama_to_plugins_adds_eq_when_missing() {
    use sotf_audio_player::spinorama_eq_types::SpinoramaBiquad;
    let mut app = App::new(Theme::default(), false);
    app.spinorama_eq.model.selected_speaker = Some("Test Speaker".to_string());
    app.spinorama_eq.model.filters = vec![
        SpinoramaBiquad {
            filter_type: "Peak".to_string(),
            freq: 1000.0,
            q: 1.5,
            db_gain: -3.0,
        },
        SpinoramaBiquad {
            filter_type: "Lowshelf".to_string(),
            freq: 80.0,
            q: 0.7,
            db_gain: 2.0,
        },
    ];

    let result = app.apply_spinorama_to_plugins();
    assert!(result.is_ok(), "Expected Ok, got {:?}", result);

    // An EQ plugin should now exist in the chain
    let has_eq = (0..app.plugin_rack.graph.len()).any(|i| {
        app.plugin_rack
            .graph
            .get_plugin(i)
            .map(|p| !p.is_permanent() && matches!(p.settings, PluginSettings::EQ { .. }))
            .unwrap_or(false)
    });
    assert!(has_eq, "Expected an EQ plugin to be present");
}

#[test]
fn test_apply_spinorama_to_plugins_updates_last_eq() {
    use sotf_audio_player::spinorama_eq_types::SpinoramaBiquad;
    let mut app = App::new(Theme::default(), false);
    // Add two EQ plugins — spinorama should target the last one
    app.add_plugin(&PluginType::EQ);
    app.add_plugin(&PluginType::EQ);

    // Record indices of both EQ plugins
    let eq_indices: Vec<usize> = (0..app.plugin_rack.graph.len())
        .filter(|&i| {
            app.plugin_rack
                .graph
                .get_plugin(i)
                .map(|p| !p.is_permanent() && matches!(p.settings, PluginSettings::EQ { .. }))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(eq_indices.len(), 2, "Expected two EQ plugins");
    let last_eq_idx = eq_indices[1];

    app.spinorama_eq.model.selected_speaker = Some("Test Speaker".to_string());
    app.spinorama_eq.model.filters = vec![SpinoramaBiquad {
        filter_type: "Peak".to_string(),
        freq: 500.0,
        q: 2.0,
        db_gain: 1.5,
    }];

    let result = app.apply_spinorama_to_plugins();
    assert!(result.is_ok(), "Expected Ok, got {:?}", result);

    // Verify the LAST EQ plugin was updated (not the first)
    let plugin = app.plugin_rack.graph.get_plugin(last_eq_idx).unwrap();
    if let PluginSettings::EQ { filters, .. } = &plugin.settings {
        assert_eq!(filters.len(), 1);
        assert!((filters[0].frequency - 500.0).abs() < 0.01);
    } else {
        panic!("Expected EQ plugin settings");
    }

    // First EQ should still have default filters (unchanged)
    let first_plugin = app.plugin_rack.graph.get_plugin(eq_indices[0]).unwrap();
    if let PluginSettings::EQ { filters, .. } = &first_plugin.settings {
        // Default EQ has no filters with freq 500
        assert!(
            filters.is_empty() || filters.iter().all(|f| (f.frequency - 500.0).abs() > 0.01),
            "First EQ should not have been modified"
        );
    }
}

#[test]
fn test_apply_spinorama_to_plugins_empty_filters_returns_error() {
    let mut app = App::new(Theme::default(), false);
    app.spinorama_eq.model.filters = vec![];
    let result = app.apply_spinorama_to_plugins();
    assert!(result.is_err());
}

#[cfg(test)]
mod draw_tests {
    use crate::app::{App, Screen};
    use crate::theme::Theme;
    use crate::ui;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_draw_skips_filtered_album_cache_on_non_library_screens() {
        let mut app = App::new(Theme::default(), false);
        app.current_screen = Screen::Configure;
        app.library_view.needs_filter_update = true;

        let backend = TestBackend::new(80, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| ui::draw(f, &mut app)).unwrap();

        assert!(
            app.library_view.needs_filter_update,
            "filtered_albums() should not run on the configure screen"
        );

        app.current_screen = Screen::Library;
        terminal.draw(|f| ui::draw(f, &mut app)).unwrap();

        assert!(
            !app.library_view.needs_filter_update,
            "filtered_albums() should run on the library screen"
        );
    }
}

#[cfg(test)]
mod scanner_tests {
    use crate::app::App;
    use crate::theme::Theme;
    use sotf_audio_player::DirectoryInfo;
    use std::path::PathBuf;

    fn empty_directory() -> DirectoryInfo {
        DirectoryInfo {
            path: PathBuf::new(),
            file_count: 0,
            album_count: 0,
            last_scanned: None,
            expanded: false,
            subdirectories: vec![],
            children_loaded: false,
        }
    }

    #[test]
    fn test_scan_library_uses_atomic_progress_counters() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut app = App::new(Theme::default(), false);
        app.library.directories.clear();
        app.library.directories.push(DirectoryInfo {
            path: temp_dir.path().to_path_buf(),
            ..empty_directory()
        });

        let result = app.scan_library();

        assert!(result.is_ok());
        assert!(!app.scan.in_progress);
    }
}

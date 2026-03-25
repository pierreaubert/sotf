#[cfg(test)]
mod tests {
    use crate::app::*;
    use crate::theme::Theme;
    use sotf_audio::devices::AudioDevice;
    use sotf_audio_player::{Album, DirectoryInfo, Track};
    use sotf_audio_player::{PluginSettings, PluginType};
    use std::path::PathBuf;

    fn create_test_directory_info(path: &str) -> DirectoryInfo {
        DirectoryInfo {
            path: PathBuf::from(path),
            file_count: 10,
            album_count: 2,
            last_scanned: None,
            expanded: false,
            subdirectories: vec![],
            children_loaded: false,
        }
    }

    fn create_test_app_with_directories(num_dirs: usize) -> App {
        let mut app = App::new(Theme::default(), false);
        for i in 0..num_dirs {
            app.library
                .directories
                .push(create_test_directory_info(&format!("/test/dir{}", i)));
        }
        app
    }

    #[test]
    fn test_select_next_directory() {
        let mut app = create_test_app_with_directories(3);

        assert_eq!(app.selected_directory_index, 0);

        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 1);

        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 2);

        // Should wrap around to 0
        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 0);
    }

    #[test]
    fn test_select_previous_directory() {
        let mut app = create_test_app_with_directories(3);

        assert_eq!(app.selected_directory_index, 0);

        // Should wrap around to last item
        app.select_previous_directory();
        assert_eq!(app.selected_directory_index, 2);

        app.select_previous_directory();
        assert_eq!(app.selected_directory_index, 1);

        app.select_previous_directory();
        assert_eq!(app.selected_directory_index, 0);
    }

    #[test]
    fn test_page_down_directories() {
        let mut app = create_test_app_with_directories(30);

        assert_eq!(app.selected_directory_index, 0);

        // Page down by 20
        app.page_down_directories(20);
        assert_eq!(app.selected_directory_index, 20);

        // Page down by 20 again - should stop at max (29)
        app.page_down_directories(20);
        assert_eq!(app.selected_directory_index, 29);

        // Should stay at max
        app.page_down_directories(20);
        assert_eq!(app.selected_directory_index, 29);
    }

    #[test]
    fn test_page_up_directories() {
        let mut app = create_test_app_with_directories(30);

        // Start at the end
        app.selected_directory_index = 29;

        // Page up by 20
        app.page_up_directories(20);
        assert_eq!(app.selected_directory_index, 9);

        // Page up by 20 again - should stop at 0
        app.page_up_directories(20);
        assert_eq!(app.selected_directory_index, 0);

        // Should stay at 0
        app.page_up_directories(20);
        assert_eq!(app.selected_directory_index, 0);
    }

    #[test]
    fn test_navigation_with_empty_directories() {
        let mut app = App::new(Theme::default(), false);
        assert_eq!(app.selected_directory_index, 0);

        // Should not crash with empty directories
        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 0);

        app.select_previous_directory();
        assert_eq!(app.selected_directory_index, 0);

        app.page_down_directories(20);
        assert_eq!(app.selected_directory_index, 0);

        app.page_up_directories(20);
        assert_eq!(app.selected_directory_index, 0);
    }

    #[test]
    fn test_navigation_with_expanded_directories() {
        let mut app = create_test_app_with_directories(2);

        // Add subdirectories to first directory
        app.library.directories[0].subdirectories = vec![
            create_test_directory_info("/test/dir0/subdir1"),
            create_test_directory_info("/test/dir0/subdir2"),
        ];
        app.library.directories[0].children_loaded = true;

        // Initially collapsed - tree has 2 items
        assert_eq!(app.get_directory_tree_items().len(), 2);
        assert_eq!(app.selected_directory_index, 0);

        // Expand first directory
        app.toggle_directory_expansion();

        // Now tree has 4 items: dir0, subdir1, subdir2, dir1
        let tree_items = app.get_directory_tree_items();
        assert_eq!(tree_items.len(), 4);

        // Navigate through expanded tree
        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 1); // subdir1

        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 2); // subdir2

        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 3); // dir1

        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 0); // wrap to dir0
    }

    #[test]
    fn test_get_directory_tree_items() {
        let mut app = create_test_app_with_directories(1);

        // Add subdirectories
        app.library.directories[0].subdirectories = vec![
            create_test_directory_info("/test/dir0/subdir1"),
            create_test_directory_info("/test/dir0/subdir2"),
        ];
        app.library.directories[0].children_loaded = true;

        // Collapsed - should only show root
        let tree_items = app.get_directory_tree_items();
        assert_eq!(tree_items.len(), 1);
        assert_eq!(tree_items[0].0, PathBuf::from("/test/dir0"));
        assert_eq!(tree_items[0].1, 0); // level
        assert!(!tree_items[0].2); // not expanded

        // Expand
        app.toggle_directory_expansion();

        // Should show root + 2 subdirectories
        let tree_items = app.get_directory_tree_items();
        assert_eq!(tree_items.len(), 3);
        assert_eq!(tree_items[0].0, PathBuf::from("/test/dir0"));
        assert_eq!(tree_items[0].1, 0); // level
        assert!(tree_items[0].2); // expanded

        assert_eq!(tree_items[1].0, PathBuf::from("/test/dir0/subdir1"));
        assert_eq!(tree_items[1].1, 1); // level

        assert_eq!(tree_items[2].0, PathBuf::from("/test/dir0/subdir2"));
        assert_eq!(tree_items[2].1, 1); // level
    }

    fn create_test_album(artist: &str, title: &str, base_path: &str, track_count: usize) -> Album {
        let mut tracks = Vec::new();
        for i in 0..track_count {
            tracks.push(Track {
                path: PathBuf::from(format!("{}/track{}.flac", base_path, i)),
                title: None,
                artist: Some(artist.to_string()),
                track_number: Some(i as u32),
                duration_secs: None,
                channels: None,
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: None,
                composer: None,
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
                edition: None,
                is_favorite: false,
                play_count: 0,
                bit_depth: None,
                sample_rate: None,
                source: None,
                uuid: None,
            });
        }
        Album {
            id: None,
            title: title.to_string(),
            year: None,
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
    fn test_next_track_removes_finished_album_and_advances() {
        let mut app = App::new(Theme::default(), false);

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);

        app.queue = vec![
            QueueEntry::new(QueueItem::new(album1)),
            QueueEntry::new(QueueItem::new(album2)),
        ];
        app.current_queue_index = Some(0);
        app.is_playing = true;

        let first_path = app.current_track_path().unwrap();
        assert!(first_path.to_string_lossy().contains("track0.flac"));

        let second_path = app.next_track().unwrap();
        assert!(second_path.as_path().unwrap().to_string_lossy().contains("track1.flac"));
        assert_eq!(app.queue.len(), 2);
        assert_eq!(app.current_queue_index, Some(0));

        let third_path = app.next_track().unwrap();
        assert!(third_path.as_path().unwrap().to_string_lossy().contains("album2/track0.flac"));
        assert_eq!(app.queue.len(), 1);
        assert_eq!(app.current_queue_index, Some(0));

        let fourth_path = app.next_track().unwrap();
        assert!(fourth_path.as_path().unwrap().to_string_lossy().contains("album2/track1.flac"));

        let none = app.next_track();
        assert!(none.is_none());
        assert!(app.queue.is_empty());
        assert!(app.current_queue_index.is_none());
        assert!(!app.is_playing);
    }

    #[test]
    fn test_adjust_eq_parameters() {
        let mut app = App::new(Theme::default(), false);
        let plugin_idx = app.plugin_chain.add_plugin(&PluginType::EQ);
        app.editing_plugin_index = Some(plugin_idx);

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
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
        app.plugin_param_selection = 1; // Index 0 is now 'Max Filters'
        assert!(app.adjust_selected_param(1.0));
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let filters = match &plugin.settings {
            PluginSettings::EQ { filters, .. } => filters,
            _ => panic!("Expected EQ plugin"),
        };
        assert_ne!(filters[0].frequency, orig_freq);

        // Q
        app.plugin_param_selection = 2;
        assert!(app.adjust_selected_param(1.0));
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let filters = match &plugin.settings {
            PluginSettings::EQ { filters, .. } => filters,
            _ => panic!("Expected EQ plugin"),
        };
        assert_ne!(filters[0].q, orig_q);

        // Gain
        app.plugin_param_selection = 3;
        assert!(app.adjust_selected_param(1.0));
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let filters = match &plugin.settings {
            PluginSettings::EQ { filters, .. } => filters,
            _ => panic!("Expected EQ plugin"),
        };
        assert_ne!(filters[0].gain_db, orig_gain);

        // Type
        app.plugin_param_selection = 4;
        assert!(app.adjust_selected_param(1.0));
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let filters = match &plugin.settings {
            PluginSettings::EQ { filters, .. } => filters,
            _ => panic!("Expected EQ plugin"),
        };
        assert_ne!(filters[0].filter_type, orig_type);
    }

    #[test]
    fn test_adjust_upmixer_parameters() {
        let mut app = App::new(Theme::default(), false);
        let plugin_idx = app.plugin_chain.add_plugin(&PluginType::Upmixer);
        app.editing_plugin_index = Some(plugin_idx);

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
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
                gain_front_direct,
                gain_front_ambient,
                gain_rear_ambient,
                height_gain,
                lfe_gain,
                lfe_cutoff_hz,
                stereo_width,
                center_spread,
                bandpass_hz,
                enable_subharmonic_synth,
                subharmonic_gain,
                subharmonic_freq_hz,
                subharmonic_attack_ms,
                subharmonic_release_ms,
                enable_hr_direct,
                hr_sharpen,
                safety_cap_db,
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
            app.plugin_param_selection = idx;
            assert!(app.adjust_selected_param(1.0));
        }

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        if let PluginSettings::Upmixer {
            speaker_config,
            gain_front_direct,
            gain_front_ambient,
            gain_rear_ambient,
            height_gain,
            lfe_gain,
            lfe_cutoff_hz,
            stereo_width,
            center_spread,
            bandpass_hz,
            enable_subharmonic_synth,
            subharmonic_gain,
            subharmonic_freq_hz,
            subharmonic_attack_ms,
            subharmonic_release_ms,
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
        let plugin_idx = app.plugin_chain.add_plugin(&PluginType::Compressor);
        app.editing_plugin_index = Some(plugin_idx);

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
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
            app.plugin_param_selection = idx;
            assert!(app.adjust_selected_param(-1.0));
        }

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
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
        let plugin_idx = app.plugin_chain.add_plugin(&PluginType::Limiter);
        app.editing_plugin_index = Some(plugin_idx);
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
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
        // Limiter params: 0=threshold, 1=release, 2=lookahead, 3=soft, 4=true_peak, 5=dual_release, 6=mix
        for idx in [0, 1, 2, 3, 6] {
            app.plugin_param_selection = idx;
            assert!(app.adjust_selected_param(-1.0));
        }
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
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
        let plugin_idx = app.plugin_chain.add_plugin(&PluginType::Gate);
        app.editing_plugin_index = Some(plugin_idx);
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
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
            app.plugin_param_selection = idx;
            let delta = if idx == 5 { -1.0 } else { 1.0 }; // mix starts at max, decrease it
            assert!(app.adjust_selected_param(delta));
        }
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
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
            .plugin_chain
            .add_plugin(&PluginType::LoudnessCompensation);
        app.editing_plugin_index = Some(plugin_idx);
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let (orig_low_freq, orig_low_gain, orig_high_freq, orig_high_gain) = match &plugin.settings
        {
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
            app.plugin_param_selection = idx;
            assert!(app.adjust_selected_param(1.0));
        }
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
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
        let plugin_idx = app.plugin_chain.add_plugin(&PluginType::BinauralDecoder);
        app.editing_plugin_index = Some(plugin_idx);
        app.selected_plugin_index = plugin_idx;

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let (orig_sofa, orig_channels, orig_opt, orig_ext, orig_near) = match &plugin.settings {
            PluginSettings::BinauralDecoder {
                sofa_file,
                input_channels,
                enable_optimization,
                externalization,
                near_field_strength,
                ..
            } => (
                sofa_file.clone(),
                *input_channels,
                *enable_optimization,
                *externalization,
                *near_field_strength,
            ),
            _ => panic!("Expected BinauralDecoder plugin"),
        };

        // Adjust numeric / boolean parameters via adjust_selected_param
        for idx in 1..5 {
            app.plugin_param_selection = idx;
            assert!(app.adjust_selected_param(1.0));
        }

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        if let PluginSettings::BinauralDecoder {
            sofa_file,
            input_channels,
            enable_optimization,
            externalization,
            near_field_strength,
            ..
        } = &plugin.settings
        {
            assert_eq!(*sofa_file, orig_sofa); // unchanged by adjust_selected_param
            assert_ne!(*input_channels, orig_channels);
            assert_ne!(*enable_optimization, orig_opt);
            assert_ne!(*externalization, orig_ext);
            assert_ne!(*near_field_strength, orig_near);
        } else {
            panic!("Expected BinauralDecoder plugin");
        }

        // Now set SOFA file via load_sofa_file path
        app.sofa_file_input = "/tmp/test.sofa".to_string();
        app.load_sofa_file().unwrap();

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        if let PluginSettings::BinauralDecoder { sofa_file, .. } = &plugin.settings {
            assert_eq!(sofa_file, "/tmp/test.sofa");
        } else {
            panic!("Expected BinauralDecoder plugin");
        }
    }

    // ============================================================================
    // QueueItem Unit Tests
    // ============================================================================

    #[test]
    fn test_queue_item_new() {
        let album = create_test_album("Artist", "Album", "/music/album", 3);
        let queue_item = QueueItem::new(album);

        assert_eq!(queue_item.current_track_index, 0);
        assert_eq!(queue_item.album.title, "Album");
        assert_eq!(queue_item.album.tracks.len(), 3);
    }

    #[test]
    fn test_queue_item_current_track() {
        let album = create_test_album("Artist", "Album", "/music/album", 3);
        let queue_item = QueueItem::new(album);

        let track = queue_item.current_track().unwrap();
        assert!(track.path.to_string_lossy().contains("track0.flac"));
    }

    #[test]
    fn test_queue_item_next_track() {
        let album = create_test_album("Artist", "Album", "/music/album", 3);
        let mut queue_item = QueueItem::new(album);

        assert_eq!(queue_item.current_track_index, 0);

        // Advance to next track
        let track = queue_item.next_track().unwrap();
        assert!(track.path.to_string_lossy().contains("track1.flac"));
        assert_eq!(queue_item.current_track_index, 1);

        // Advance again
        let track = queue_item.next_track().unwrap();
        assert!(track.path.to_string_lossy().contains("track2.flac"));
        assert_eq!(queue_item.current_track_index, 2);

        // No more tracks
        assert!(queue_item.next_track().is_none());
        assert_eq!(queue_item.current_track_index, 2); // Index unchanged
    }

    #[test]
    fn test_queue_item_previous_track() {
        let album = create_test_album("Artist", "Album", "/music/album", 3);
        let mut queue_item = QueueItem::new(album);

        // Start at last track
        queue_item.current_track_index = 2;

        // Go back
        let track = queue_item.previous_track().unwrap();
        assert!(track.path.to_string_lossy().contains("track1.flac"));
        assert_eq!(queue_item.current_track_index, 1);

        // Go back again
        let track = queue_item.previous_track().unwrap();
        assert!(track.path.to_string_lossy().contains("track0.flac"));
        assert_eq!(queue_item.current_track_index, 0);

        // Can't go back further
        assert!(queue_item.previous_track().is_none());
        assert_eq!(queue_item.current_track_index, 0); // Index unchanged
    }

    #[test]
    fn test_queue_item_empty_album() {
        let album = create_test_album("Artist", "Empty Album", "/music/empty", 0);
        let mut queue_item = QueueItem::new(album);

        assert!(queue_item.current_track().is_none());
        assert!(queue_item.next_track().is_none());
        assert!(queue_item.previous_track().is_none());
    }

    // ============================================================================
    // Volume Control Tests
    // ============================================================================

    #[test]
    fn test_increase_volume() {
        let mut app = App::new(Theme::default(), false);
        app.volume = 0.5;

        app.increase_volume();
        assert!((app.volume - 0.55).abs() < 0.001);

        // Keep increasing
        for _ in 0..20 {
            app.increase_volume();
        }
        // Should clamp at 1.0
        assert!((app.volume - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_decrease_volume() {
        let mut app = App::new(Theme::default(), false);
        app.volume = 0.5;

        app.decrease_volume();
        assert!((app.volume - 0.45).abs() < 0.001);

        // Keep decreasing
        for _ in 0..20 {
            app.decrease_volume();
        }
        // Should clamp at 0.0
        assert!((app.volume - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_volume_boundary_values() {
        let mut app = App::new(Theme::default(), false);

        // Start at 0
        app.volume = 0.0;
        app.decrease_volume();
        assert_eq!(app.volume, 0.0);

        // Start at 1
        app.volume = 1.0;
        app.increase_volume();
        assert_eq!(app.volume, 1.0);
    }

    // ============================================================================
    // Queue Management Tests
    // ============================================================================

    #[test]
    fn test_clear_queue() {
        let mut app = App::new(Theme::default(), false);

        // Add some items to queue
        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);
        app.queue = vec![
            QueueEntry::new(QueueItem::new(album1)),
            QueueEntry::new(QueueItem::new(album2)),
        ];
        app.current_queue_index = Some(1);
        app.is_playing = true;

        app.clear_queue();

        assert!(app.queue.is_empty());
        assert!(app.current_queue_index.is_none());
        assert!(!app.is_playing);
    }

    #[test]
    fn test_remove_from_queue_first_item() {
        let mut app = App::new(Theme::default(), false);

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);
        let album3 = create_test_album("Artist", "Album3", "/music/album3", 2);
        app.queue = vec![
            QueueEntry::new(QueueItem::new(album1)),
            QueueEntry::new(QueueItem::new(album2)),
            QueueEntry::new(QueueItem::new(album3)),
        ];
        app.current_queue_index = Some(1);

        // Remove first item
        app.remove_from_queue(0);

        assert_eq!(app.queue.len(), 2);
        assert_eq!(app.queue[0].item.album.title, "Album2");
        assert_eq!(app.current_queue_index, Some(0)); // Adjusted
    }

    #[test]
    fn test_remove_from_queue_current_playing() {
        let mut app = App::new(Theme::default(), false);

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);
        app.queue = vec![
            QueueEntry::new(QueueItem::new(album1)),
            QueueEntry::new(QueueItem::new(album2)),
        ];
        app.current_queue_index = Some(0);
        app.is_playing = true;

        // Remove currently playing item
        app.remove_from_queue(0);

        assert_eq!(app.queue.len(), 1);
        assert_eq!(app.queue[0].item.album.title, "Album2");
        // Current queue index should remain at 0 (now pointing to Album2)
        assert_eq!(app.current_queue_index, Some(0));
    }

    #[test]
    fn test_remove_from_queue_last_item() {
        let mut app = App::new(Theme::default(), false);

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        app.queue = vec![QueueEntry::new(QueueItem::new(album1))];
        app.current_queue_index = Some(0);
        app.is_playing = true;

        // Remove last item
        app.remove_from_queue(0);

        assert!(app.queue.is_empty());
        assert!(app.current_queue_index.is_none());
        assert!(!app.is_playing);
    }

    #[test]
    fn test_toggle_queue_item_expansion() {
        let mut app = App::new(Theme::default(), false);

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);
        app.queue = vec![
            QueueEntry::new(QueueItem::new(album1)),
            QueueEntry::new(QueueItem::new(album2)),
        ];
        app.selected_queue_index = 1;

        // Toggle expansion
        app.toggle_queue_item_expansion();
        assert!(!app.queue[0].expanded);
        assert!(app.queue[1].expanded);

        // Toggle again
        app.toggle_queue_item_expansion();
        assert!(!app.queue[1].expanded);
    }

    #[test]
    fn test_select_next_queue_item() {
        let mut app = App::new(Theme::default(), false);

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);
        app.queue = vec![
            QueueEntry::new(QueueItem::new(album1)),
            QueueEntry::new(QueueItem::new(album2)),
        ];
        app.selected_queue_index = 0;

        app.select_next_queue_item();
        assert_eq!(app.selected_queue_index, 1);

        // Wrap around
        app.select_next_queue_item();
        assert_eq!(app.selected_queue_index, 0);
    }

    #[test]
    fn test_select_previous_queue_item() {
        let mut app = App::new(Theme::default(), false);

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);
        app.queue = vec![
            QueueEntry::new(QueueItem::new(album1)),
            QueueEntry::new(QueueItem::new(album2)),
        ];
        app.selected_queue_index = 0;

        // Wrap around to last
        app.select_previous_queue_item();
        assert_eq!(app.selected_queue_index, 1);

        app.select_previous_queue_item();
        assert_eq!(app.selected_queue_index, 0);
    }

    #[test]
    fn test_queue_navigation_empty() {
        let mut app = App::new(Theme::default(), false);
        app.selected_queue_index = 0;

        app.select_next_queue_item();
        assert_eq!(app.selected_queue_index, 0);

        app.select_previous_queue_item();
        assert_eq!(app.selected_queue_index, 0);
    }

    #[test]
    fn test_queue_navigation_into_expanded_tracks() {
        let mut app = App::new(Theme::default(), false);

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 3);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);
        app.queue = vec![
            QueueEntry::new(QueueItem::new(album1)),
            QueueEntry::new(QueueItem::new(album2)),
        ];
        app.selected_queue_index = 0;
        app.queue[0].expanded = true;

        // Start on album header
        assert_eq!(app.selected_queue_track_index, None);

        // Down → first track
        app.select_next_queue_item();
        assert_eq!(app.selected_queue_index, 0);
        assert_eq!(app.selected_queue_track_index, Some(0));

        // Down → second track
        app.select_next_queue_item();
        assert_eq!(app.selected_queue_index, 0);
        assert_eq!(app.selected_queue_track_index, Some(1));

        // Down → third track
        app.select_next_queue_item();
        assert_eq!(app.selected_queue_index, 0);
        assert_eq!(app.selected_queue_track_index, Some(2));

        // Down → next album header
        app.select_next_queue_item();
        assert_eq!(app.selected_queue_index, 1);
        assert_eq!(app.selected_queue_track_index, None);
    }

    #[test]
    fn test_queue_navigation_previous_into_expanded_tracks() {
        let mut app = App::new(Theme::default(), false);

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);
        app.queue = vec![
            QueueEntry::new(QueueItem::new(album1)),
            QueueEntry::new(QueueItem::new(album2)),
        ];
        app.queue[0].expanded = true;
        app.selected_queue_index = 1;

        // Up from album2 header → last track of expanded album1
        app.select_previous_queue_item();
        assert_eq!(app.selected_queue_index, 0);
        assert_eq!(app.selected_queue_track_index, Some(1));

        // Up → first track
        app.select_previous_queue_item();
        assert_eq!(app.selected_queue_index, 0);
        assert_eq!(app.selected_queue_track_index, Some(0));

        // Up → album1 header
        app.select_previous_queue_item();
        assert_eq!(app.selected_queue_index, 0);
        assert_eq!(app.selected_queue_track_index, None);
    }

    #[test]
    fn test_collapse_resets_track_selection() {
        let mut app = App::new(Theme::default(), false);

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 3);
        app.queue = vec![QueueEntry::new(QueueItem::new(album1))];
        app.queue[0].expanded = true;
        app.selected_queue_index = 0;
        app.selected_queue_track_index = Some(1);

        // Left on a track → moves to album header
        app.collapse_queue_item();
        assert!(app.queue[0].expanded); // still expanded
        assert_eq!(app.selected_queue_track_index, None);

        // Left on album header → collapses
        app.collapse_queue_item();
        assert!(!app.queue[0].expanded);
    }

    #[test]
    fn test_jump_to_selected_track() {
        let mut app = App::new(Theme::default(), false);

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 3);
        app.queue = vec![QueueEntry::new(QueueItem::new(album1))];
        app.selected_queue_index = 0;
        app.selected_queue_track_index = Some(2);

        app.jump_to_selected_album();
        assert_eq!(app.queue[0].item.current_track_index, 2);
    }

    // ============================================================================
    // Album Navigation Tests
    // ============================================================================

    fn create_test_app_with_albums(num_albums: usize) -> App {
        let mut app = App::new(Theme::default(), false);
        for i in 0..num_albums {
            let album = create_test_album(
                &format!("Artist{}", i),
                &format!("Album{}", i),
                &format!("/music/album{}", i),
                3,
            );
            app.library.albums.push(album);
        }
        app
    }

    #[test]
    fn test_select_next_album() {
        let mut app = create_test_app_with_albums(5);
        app.selected_album_index = 0;

        app.select_next_album();
        assert_eq!(app.selected_album_index, 1);

        app.select_next_album();
        assert_eq!(app.selected_album_index, 2);
    }

    #[test]
    fn test_select_previous_album() {
        let mut app = create_test_app_with_albums(5);
        app.selected_album_index = 2;

        app.select_previous_album();
        assert_eq!(app.selected_album_index, 1);

        app.select_previous_album();
        assert_eq!(app.selected_album_index, 0);

        // Wraps around to last album
        app.select_previous_album();
        assert_eq!(app.selected_album_index, 4);
    }

    #[test]
    fn test_page_down_albums() {
        let mut app = create_test_app_with_albums(30);
        app.selected_album_index = 0;

        app.page_down_albums(10);
        assert_eq!(app.selected_album_index, 10);

        app.page_down_albums(10);
        assert_eq!(app.selected_album_index, 20);

        // Should stop at max (29)
        app.page_down_albums(20);
        assert_eq!(app.selected_album_index, 29);
    }

    #[test]
    fn test_page_up_albums() {
        let mut app = create_test_app_with_albums(30);
        app.selected_album_index = 25;

        app.page_up_albums(10);
        assert_eq!(app.selected_album_index, 15);

        app.page_up_albums(10);
        assert_eq!(app.selected_album_index, 5);

        // Should stop at 0
        app.page_up_albums(10);
        assert_eq!(app.selected_album_index, 0);
    }

    #[test]
    fn test_album_navigation_empty_library() {
        let mut app = App::new(Theme::default(), false);
        app.selected_album_index = 0;

        app.select_next_album();
        assert_eq!(app.selected_album_index, 0);

        app.page_down_albums(10);
        assert_eq!(app.selected_album_index, 0);
    }

    // ============================================================================
    // Plugin Management Tests
    // ============================================================================

    #[test]
    fn test_add_plugin() {
        let mut app = App::new(Theme::default(), false);
        // App starts with default permanent plugins (LoudnessMonitor, Matrix, etc.)
        let initial_count = app.plugin_chain.len();
        assert!(initial_count >= 2, "App should start with default plugins");

        app.add_plugin(&PluginType::Gain);
        assert_eq!(app.plugin_chain.len(), initial_count + 1);
        assert!(app.needs_plugin_update);

        app.add_plugin(&PluginType::EQ);
        assert_eq!(app.plugin_chain.len(), initial_count + 2);
    }

    #[test]
    fn test_remove_plugin() {
        let mut app = App::new(Theme::default(), false);
        let initial_count = app.plugin_chain.len();

        app.add_plugin(&PluginType::Gain);
        app.add_plugin(&PluginType::EQ);
        app.add_plugin(&PluginType::Limiter);

        assert_eq!(app.plugin_chain.len(), initial_count + 3);

        // Remove one of our added plugins (index after the defaults)
        app.remove_plugin(initial_count);
        assert_eq!(app.plugin_chain.len(), initial_count + 2);
        assert!(app.needs_plugin_update);
    }

    #[test]
    fn test_toggle_plugin() {
        let mut app = App::new(Theme::default(), false);
        app.add_plugin(&PluginType::Gain);

        // Check initial state (enabled)
        let plugin = app.plugin_chain.get_plugin(0).unwrap();
        assert!(plugin.enabled);

        // Toggle off
        app.toggle_plugin(0);
        let plugin = app.plugin_chain.get_plugin(0).unwrap();
        assert!(!plugin.enabled);

        // Toggle on
        app.toggle_plugin(0);
        let plugin = app.plugin_chain.get_plugin(0).unwrap();
        assert!(plugin.enabled);
    }

    #[test]
    fn test_move_plugin_up() {
        let mut app = App::new(Theme::default(), false);
        let base_idx = app.plugin_chain.user_plugin_insert_index();
        app.add_plugin(&PluginType::Gain);
        app.add_plugin(&PluginType::EQ);
        app.add_plugin(&PluginType::Limiter);

        // Move limiter up (from base_idx + 2 to base_idx + 1)
        app.move_plugin_up(base_idx + 2);

        // Limiter should now be at base_idx + 1
        let plugin = app.plugin_chain.get_plugin(base_idx + 1).unwrap();
        assert!(matches!(plugin.plugin_type(), PluginType::Limiter));
    }

    #[test]
    fn test_move_plugin_down() {
        let mut app = App::new(Theme::default(), false);
        let base_idx = app.plugin_chain.user_plugin_insert_index();
        app.add_plugin(&PluginType::Gain);
        app.add_plugin(&PluginType::EQ);
        app.add_plugin(&PluginType::Limiter);

        // Move gain down (from base_idx to base_idx + 1)
        app.move_plugin_down(base_idx);

        // Gain should now be at base_idx + 1
        let plugin = app.plugin_chain.get_plugin(base_idx + 1).unwrap();
        assert!(matches!(plugin.plugin_type(), PluginType::Gain));
    }

    #[test]
    fn test_move_plugin_boundary() {
        let mut app = App::new(Theme::default(), false);
        app.add_plugin(&PluginType::Gain);
        app.add_plugin(&PluginType::EQ);

        // Try to move first plugin (index 0) up - should do nothing
        let first_plugin_type = app.plugin_chain.get_plugin(0).unwrap().plugin_type();
        app.move_plugin_up(0);
        let plugin = app.plugin_chain.get_plugin(0).unwrap();
        assert_eq!(plugin.plugin_type(), first_plugin_type);

        // Try to move last plugin down (should do nothing)
        let last_idx = app.plugin_chain.len() - 1;
        let last_plugin_type = app.plugin_chain.get_plugin(last_idx).unwrap().plugin_type();
        app.move_plugin_down(last_idx);
        let plugin = app.plugin_chain.get_plugin(last_idx).unwrap();
        assert_eq!(plugin.plugin_type(), last_plugin_type);
    }

    #[test]
    fn test_select_next_plugin() {
        let mut app = App::new(Theme::default(), false);
        app.add_plugin(&PluginType::Gain);
        app.add_plugin(&PluginType::EQ);

        let total_plugins = app.plugin_chain.len();
        app.selected_plugin_index = 0;

        // Navigate through all plugins
        for i in 1..total_plugins {
            app.select_next_plugin();
            assert_eq!(app.selected_plugin_index, i);
        }

        // Wrap around to 0
        app.select_next_plugin();
        assert_eq!(app.selected_plugin_index, 0);
    }

    #[test]
    fn test_select_previous_plugin() {
        let mut app = App::new(Theme::default(), false);
        app.add_plugin(&PluginType::Gain);

        let total_plugins = app.plugin_chain.len();
        app.selected_plugin_index = 0;

        // Wrap to last
        app.select_previous_plugin();
        assert_eq!(app.selected_plugin_index, total_plugins - 1);

        // Navigate back to 0
        for _ in 1..total_plugins {
            app.select_previous_plugin();
        }
        assert_eq!(app.selected_plugin_index, 0);
    }

    #[test]
    fn test_enter_exit_plugin_edit_mode() {
        let mut app = App::new(Theme::default(), false);
        app.add_plugin(&PluginType::EQ);
        app.selected_plugin_index = 0;

        assert!(app.editing_plugin_index.is_none());

        app.enter_plugin_edit_mode();
        assert_eq!(app.editing_plugin_index, Some(0));
        assert_eq!(app.plugin_param_selection, 0);

        app.exit_plugin_edit_mode();
        assert!(app.editing_plugin_index.is_none());
    }

    // ============================================================================
    // Library View Mode Tests
    // ============================================================================

    #[test]
    fn test_toggle_library_view_mode() {
        let mut app = App::new(Theme::default(), false);
        assert_eq!(app.library_view_mode, LibraryViewMode::Flat);

        app.toggle_library_view_mode();
        assert_eq!(app.library_view_mode, LibraryViewMode::TreeView);

        app.toggle_library_view_mode();
        assert_eq!(app.library_view_mode, LibraryViewMode::Flat);
    }

    #[test]
    fn test_set_library_sort_order() {
        let mut app = App::new(Theme::default(), false);

        app.set_library_sort_order(LibrarySortOrder::Artist);
        assert_eq!(app.library_sort_order, LibrarySortOrder::Artist);

        app.set_library_sort_order(LibrarySortOrder::Album);
        assert_eq!(app.library_sort_order, LibrarySortOrder::Album);

        app.set_library_sort_order(LibrarySortOrder::Year);
        assert_eq!(app.library_sort_order, LibrarySortOrder::Year);
    }

    #[test]
    fn test_set_channel_filter() {
        let mut app = App::new(Theme::default(), false);

        app.set_channel_filter(ChannelFilter::All);
        assert_eq!(app.channel_filter, ChannelFilter::All);

        app.set_channel_filter(ChannelFilter::Stereo);
        assert_eq!(app.channel_filter, ChannelFilter::Stereo);

        app.set_channel_filter(ChannelFilter::Surround);
        assert_eq!(app.channel_filter, ChannelFilter::Surround);
    }

    #[test]
    fn test_cycle_channel_filter() {
        let mut app = App::new(Theme::default(), false);
        app.channel_filter = ChannelFilter::All;

        // Cycling depends on available channel counts, so test basic cycling
        // When library is empty, cycling should still work
        let initial = app.channel_filter;
        app.cycle_channel_filter();
        // After cycling, filter may or may not change depending on library
        // At minimum, it shouldn't panic
        let _ = app.channel_filter;

        // Reset
        app.channel_filter = initial;
    }

    // ============================================================================
    // Tree View Tests
    // ============================================================================

    #[test]
    fn test_rebuild_artist_tree() {
        let mut app = App::new(Theme::default(), false);

        // Add albums with different artists
        let album1 = create_test_album("Artist A", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist A", "Album2", "/music/album2", 2);
        let album3 = create_test_album("Artist B", "Album3", "/music/album3", 2);
        app.library.albums.push(album1);
        app.library.albums.push(album2);
        app.library.albums.push(album3);

        app.rebuild_artist_tree();

        // Should have 2 artists
        assert_eq!(app.artist_tree.len(), 2);

        // Find Artist A node - should have 2 albums
        let artist_a = app
            .artist_tree
            .iter()
            .find(|n| n.artist == "Artist A")
            .unwrap();
        assert_eq!(artist_a.album_indices.len(), 2);

        // Find Artist B node - should have 1 album
        let artist_b = app
            .artist_tree
            .iter()
            .find(|n| n.artist == "Artist B")
            .unwrap();
        assert_eq!(artist_b.album_indices.len(), 1);
    }

    #[test]
    fn test_toggle_artist_expansion() {
        let mut app = App::new(Theme::default(), false);

        // Each artist needs ≥2 albums to avoid single-album flattening in get_tree_items()
        let album1 = create_test_album("Artist A", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist A", "Album2", "/music/album2", 2);
        let album3 = create_test_album("Artist B", "Album3", "/music/album3", 2);
        let album4 = create_test_album("Artist B", "Album4", "/music/album4", 2);
        app.library.albums.push(album1);
        app.library.albums.push(album2);
        app.library.albums.push(album3);
        app.library.albums.push(album4);
        app.rebuild_artist_tree();

        app.library_view_mode = LibraryViewMode::TreeView;
        app.selected_tree_index = 0;

        // Initially collapsed
        assert!(!app.artist_tree[0].expanded);

        // Toggle expansion
        app.toggle_artist_expansion();
        assert!(app.artist_tree[0].expanded);

        // Toggle again
        app.toggle_artist_expansion();
        assert!(!app.artist_tree[0].expanded);
    }

    #[test]
    fn test_get_tree_items_collapsed() {
        let mut app = App::new(Theme::default(), false);

        let album1 = create_test_album("Artist A", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist A", "Album2", "/music/album2", 2);
        let album3 = create_test_album("Artist B", "Album3", "/music/album3", 2);
        let album4 = create_test_album("Artist B", "Album4", "/music/album4", 2);
        app.library.albums.push(album1);
        app.library.albums.push(album2);
        app.library.albums.push(album3);
        app.library.albums.push(album4);
        app.rebuild_artist_tree();

        // All collapsed - should only show artists
        let items = app.get_tree_items();
        assert_eq!(items.len(), 2);

        // Both should be Artist items
        for item in &items {
            assert!(matches!(item, TreeItem::Artist { .. }));
        }
    }

    #[test]
    fn test_get_tree_items_expanded() {
        let mut app = App::new(Theme::default(), false);

        let album1 = create_test_album("Artist A", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist A", "Album2", "/music/album2", 2);
        let album3 = create_test_album("Artist B", "Album3", "/music/album3", 2);
        let album4 = create_test_album("Artist B", "Album4", "/music/album4", 2);
        app.library.albums.push(album1);
        app.library.albums.push(album2);
        app.library.albums.push(album3);
        app.library.albums.push(album4);
        app.rebuild_artist_tree();

        // Expand first artist
        app.artist_tree[0].expanded = true;

        let items = app.get_tree_items();
        // Artist A (expanded) + 2 albums + Artist B (collapsed) = 4 items
        assert_eq!(items.len(), 4);

        // First should be Artist A (expanded)
        assert!(
            matches!(&items[0], TreeItem::Artist { name, expanded } if name == "Artist A" && *expanded)
        );

        // Next two should be albums
        assert!(matches!(&items[1], TreeItem::Album { .. }));
        assert!(matches!(&items[2], TreeItem::Album { .. }));

        // Last should be Artist B (collapsed)
        assert!(
            matches!(&items[3], TreeItem::Artist { name, expanded } if name == "Artist B" && !*expanded)
        );
    }

    #[test]
    fn test_select_next_tree_item() {
        let mut app = App::new(Theme::default(), false);

        let album1 = create_test_album("Artist A", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist B", "Album2", "/music/album2", 2);
        app.library.albums.push(album1);
        app.library.albums.push(album2);
        app.rebuild_artist_tree();

        app.library_view_mode = LibraryViewMode::TreeView;
        app.selected_tree_index = 0;

        app.select_next_tree_item();
        assert_eq!(app.selected_tree_index, 1);

        // Should wrap
        app.select_next_tree_item();
        assert_eq!(app.selected_tree_index, 0);
    }

    #[test]
    fn test_select_previous_tree_item() {
        let mut app = App::new(Theme::default(), false);

        let album1 = create_test_album("Artist A", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist B", "Album2", "/music/album2", 2);
        app.library.albums.push(album1);
        app.library.albums.push(album2);
        app.rebuild_artist_tree();

        app.library_view_mode = LibraryViewMode::TreeView;
        app.selected_tree_index = 0;

        // Wrap to last
        app.select_previous_tree_item();
        assert_eq!(app.selected_tree_index, 1);

        app.select_previous_tree_item();
        assert_eq!(app.selected_tree_index, 0);
    }

    // ============================================================================
    // Output Device Tests
    // ============================================================================

    fn create_test_audio_device(name: &str, is_default: bool) -> AudioDevice {
        AudioDevice {
            device_id: Some(format!("test-device-{}", name)),
            name: name.to_string(),
            display_info: None,
            is_input: false,
            is_default,
            supported_configs: vec![],
            default_config: None,
            available_sample_rates: vec![44100, 48000, 96000],
        }
    }

    #[test]
    fn test_select_next_output_device() {
        let mut app = App::new(Theme::default(), false);

        // Simulate having some devices
        app.output_devices = vec![
            create_test_audio_device("Device 1", true),
            create_test_audio_device("Device 2", false),
        ];
        app.selected_output_device_index = 0;

        app.select_next_output_device();
        assert_eq!(app.selected_output_device_index, 1);

        // Wrap
        app.select_next_output_device();
        assert_eq!(app.selected_output_device_index, 0);
    }

    #[test]
    fn test_select_previous_output_device() {
        let mut app = App::new(Theme::default(), false);

        app.output_devices = vec![
            create_test_audio_device("Device 1", true),
            create_test_audio_device("Device 2", false),
        ];
        app.selected_output_device_index = 0;

        // Wrap to last
        app.select_previous_output_device();
        assert_eq!(app.selected_output_device_index, 1);

        app.select_previous_output_device();
        assert_eq!(app.selected_output_device_index, 0);
    }

    #[test]
    fn test_get_selected_output_device() {
        let mut app = App::new(Theme::default(), false);

        // Empty devices
        assert!(app.get_selected_output_device().is_none());

        app.output_devices = vec![create_test_audio_device("Test Device", false)];
        app.selected_output_device_index = 0;

        let device = app.get_selected_output_device().unwrap();
        assert_eq!(device.name, "Test Device");
    }

    #[test]
    fn test_output_device_navigation_empty() {
        let mut app = App::new(Theme::default(), false);
        app.selected_output_device_index = 0;

        // Should not panic with empty devices
        app.select_next_output_device();
        assert_eq!(app.selected_output_device_index, 0);

        app.select_previous_output_device();
        assert_eq!(app.selected_output_device_index, 0);
    }

    // ============================================================================
    // Screen and Mode Tests
    // ============================================================================

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

    // ============================================================================
    // Playback State Tests
    // ============================================================================

    #[test]
    fn test_start_queue() {
        let mut app = App::new(Theme::default(), false);

        // Empty queue
        assert!(app.start_queue().is_none());
        assert!(app.current_queue_index.is_none());
        assert!(!app.is_playing);

        // Add items to queue
        let album = create_test_album("Artist", "Album", "/music/album", 3);
        app.queue.push(QueueEntry::new(QueueItem::new(album)));

        let path = app.start_queue();
        assert!(path.is_some());
        assert_eq!(app.current_queue_index, Some(0));
        assert!(app.is_playing);
    }

    #[test]
    fn test_previous_track_within_album() {
        let mut app = App::new(Theme::default(), false);

        let album = create_test_album("Artist", "Album", "/music/album", 3);
        app.queue.push(QueueEntry::new(QueueItem::new(album)));
        app.current_queue_index = Some(0);
        app.is_playing = true;

        // Move to track 2
        app.queue[0].item.current_track_index = 2;

        // Go back
        let path = app.previous_track();
        assert!(path.is_some());
        assert!(path.unwrap().as_path().unwrap().to_string_lossy().contains("track1.flac"));
    }

    #[test]
    fn test_previous_track_to_previous_album() {
        let mut app = App::new(Theme::default(), false);

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);
        app.queue.push(QueueEntry::new(QueueItem::new(album1)));
        app.queue.push(QueueEntry::new(QueueItem::new(album2)));
        app.current_queue_index = Some(1);
        app.is_playing = true;

        // At first track of second album
        app.queue[1].item.current_track_index = 0;

        // Go back should go to last track of first album
        let path = app.previous_track();
        assert!(path.is_some());
        assert!(
            path.unwrap()
                .as_path()
                .unwrap()
                .to_string_lossy()
                .contains("album1/track1.flac")
        );
        assert_eq!(app.current_queue_index, Some(0));
    }

    #[test]
    fn test_apply_spinorama_to_plugin_chain_adds_eq_when_missing() {
        use sotf_audio_player::spinorama_eq_types::SpinoramaBiquad;
        let mut app = App::new(Theme::default(), false);
        app.spinorama_eq.selected_speaker = Some("Test Speaker".to_string());
        app.spinorama_eq.filters = vec![
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

        let result = app.apply_spinorama_to_plugin_chain();
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);

        // An EQ plugin should now exist in the chain
        let has_eq = (0..app.plugin_chain.len()).any(|i| {
            app.plugin_chain
                .get_plugin(i)
                .map(|p| !p.is_permanent() && matches!(p.settings, PluginSettings::EQ { .. }))
                .unwrap_or(false)
        });
        assert!(has_eq, "Expected an EQ plugin to be present");
    }

    #[test]
    fn test_apply_spinorama_to_plugin_chain_updates_last_eq() {
        use sotf_audio_player::spinorama_eq_types::SpinoramaBiquad;
        let mut app = App::new(Theme::default(), false);
        // Add two EQ plugins — spinorama should target the last one
        app.add_plugin(&PluginType::EQ);
        app.add_plugin(&PluginType::EQ);

        // Record indices of both EQ plugins
        let eq_indices: Vec<usize> = (0..app.plugin_chain.len())
            .filter(|&i| {
                app.plugin_chain
                    .get_plugin(i)
                    .map(|p| !p.is_permanent() && matches!(p.settings, PluginSettings::EQ { .. }))
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(eq_indices.len(), 2, "Expected two EQ plugins");
        let last_eq_idx = eq_indices[1];

        app.spinorama_eq.selected_speaker = Some("Test Speaker".to_string());
        app.spinorama_eq.filters = vec![SpinoramaBiquad {
            filter_type: "Peak".to_string(),
            freq: 500.0,
            q: 2.0,
            db_gain: 1.5,
        }];

        let result = app.apply_spinorama_to_plugin_chain();
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);

        // Verify the LAST EQ plugin was updated (not the first)
        let plugin = app.plugin_chain.get_plugin(last_eq_idx).unwrap();
        if let PluginSettings::EQ { filters, .. } = &plugin.settings {
            assert_eq!(filters.len(), 1);
            assert!((filters[0].frequency - 500.0).abs() < 0.01);
        } else {
            panic!("Expected EQ plugin settings");
        }

        // First EQ should still have default filters (unchanged)
        let first_plugin = app.plugin_chain.get_plugin(eq_indices[0]).unwrap();
        if let PluginSettings::EQ { filters, .. } = &first_plugin.settings {
            // Default EQ has no filters with freq 500
            assert!(
                filters.is_empty() || filters.iter().all(|f| (f.frequency - 500.0).abs() > 0.01),
                "First EQ should not have been modified"
            );
        }
    }

    #[test]
    fn test_apply_spinorama_to_plugin_chain_empty_filters_returns_error() {
        let mut app = App::new(Theme::default(), false);
        app.spinorama_eq.filters = vec![];
        let result = app.apply_spinorama_to_plugin_chain();
        assert!(result.is_err());
    }
}

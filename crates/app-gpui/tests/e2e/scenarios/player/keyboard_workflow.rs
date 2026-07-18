use crate::driver::AppDriver;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player::{Album, PluginSettings, PluginType, Track};
use sotf_audio_player_gpui::app::types::{
    HeadphoneEqStep, RecordingStep, RoomEqStep, SpinoramaStep,
};
use sotf_audio_player_gpui::app::{InputMode, Screen, SettingsTab};
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;
use std::path::{Path, PathBuf};

struct KeyboardWorkflowScenario;

struct KeyboardLibraryQueueScenario;

struct KeyboardWizardScenario;

struct KeyboardPluginRackScenario;

fn bind_default_keys(cx: &mut TestAppContext) {
    gpui_ui_kit::clear_all_input_states();
    cx.update(|cx| {
        cx.bind_keys(sotf_audio_player_gpui::app::keybindings::get_keybindings(
            sotf_audio_player_gpui::app::KeymapPreset::Default,
        ));
    });
}

fn keyboard_album(id: i64, title: &str, path: &Path) -> Album {
    Album {
        id: Some(id),
        title: title.to_string(),
        year: Some(2026),
        tracks: vec![Track {
            path: PathBuf::from(path),
            title: Some(format!("{title} Track")),
            artist: Some("Keyboard QA".to_string()),
            channels: Some(2),
            sample_rate: Some(48_000),
            bit_depth: Some(24),
            ..Default::default()
        }],
        ..Default::default()
    }
}

impl TestScenario for KeyboardWorkflowScenario {
    fn name(&self) -> &'static str {
        "Whole-app keyboard navigation and overlay workflow"
    }

    fn setup(&mut self, cx: &mut TestAppContext) -> Result<(), Box<dyn Error>> {
        bind_default_keys(cx);
        Ok(())
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        for (key, screen) in [
            ("shift-l", Screen::Library),
            ("shift-q", Screen::Queue),
            ("shift-p", Screen::Studio),
        ] {
            driver.simulate_keystrokes(key);
            driver.run_until_parked();
            assert_eq!(
                driver.read_app(|app| app.ui_state.current_screen),
                screen,
                "{key} did not open {screen:?}"
            );
        }

        driver.simulate_keystrokes("shift-o");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| (
                app.ui_state.current_screen,
                app.ui_state.active_settings_tab
            )),
            (Screen::Settings, SettingsTab::AudioDevice)
        );

        driver.simulate_keystrokes("shift-d");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| (
                app.ui_state.current_screen,
                app.ui_state.active_settings_tab
            )),
            (Screen::Settings, SettingsTab::Library)
        );

        driver.navigate_to(Screen::Library);
        driver.simulate_keystrokes("/");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.ui_state.input_mode),
            InputMode::Search
        );
        driver.simulate_keystrokes("escape");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.ui_state.input_mode),
            InputMode::Normal
        );

        driver.simulate_keystrokes("?");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.ui_state.input_mode),
            InputMode::Help
        );
        driver.simulate_keystrokes("escape");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.ui_state.input_mode),
            InputMode::Normal
        );

        driver.simulate_keystrokes("f1");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.ui_state.input_mode),
            InputMode::ScreenGuide
        );
        driver.simulate_keystrokes("escape");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.ui_state.input_mode),
            InputMode::Normal
        );

        Ok(())
    }
}

impl TestScenario for KeyboardLibraryQueueScenario {
    fn name(&self) -> &'static str {
        "Keyboard-only data-backed Library and Queue workflow"
    }

    fn setup(&mut self, cx: &mut TestAppContext) -> Result<(), Box<dyn Error>> {
        bind_default_keys(cx);
        Ok(())
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);
        let temp_dir = tempfile::tempdir()?;
        let first_path = temp_dir.path().join("first.flac");
        let second_path = temp_dir.path().join("second.flac");
        std::fs::write(&first_path, b"fake audio")?;
        std::fs::write(&second_path, b"fake audio")?;

        driver.update_app(|app, _| {
            app.library_state.library.albums = vec![
                keyboard_album(1, "Keyboard First", &first_path),
                keyboard_album(2, "Keyboard Second", &second_path),
            ];
            app.library_state.invalidate_cache();
            app.library_state.ensure_cache_valid();
            app.invalidate_library_stats();
            app.library_state.selected_index = 0;
            app.queue_state.clear();
            app.playback.current_queue_index = None;
            app.playback.is_playing = false;
        });

        driver.navigate_to(Screen::Library);
        driver.simulate_keystrokes("a");
        driver.run_until_parked();
        assert_eq!(driver.read_app(|app| app.queue_state.len()), 1);

        driver.simulate_keystrokes("right");
        driver.run_until_parked();
        assert_eq!(driver.read_app(|app| app.library_state.selected_index), 1);
        driver.simulate_keystrokes("a");
        driver.run_until_parked();
        assert_eq!(driver.read_app(|app| app.queue_state.len()), 2);

        driver.simulate_keystrokes("shift-q");
        driver.run_until_parked();
        driver.simulate_keystrokes("down");
        driver.run_until_parked();
        assert_eq!(driver.read_app(|app| app.queue_state.selected_index), 1);

        driver.simulate_keystrokes("enter");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.playback.current_queue_index),
            Some(1)
        );

        driver.simulate_keystrokes("d");
        driver.run_until_parked();
        assert_eq!(driver.read_app(|app| app.queue_state.len()), 1);
        assert_eq!(driver.read_app(|app| app.queue_state.selected_index), 0);
        assert_eq!(
            driver.read_app(|app| app.queue_state[0].album.title.clone()),
            "Keyboard First"
        );

        Ok(())
    }
}

impl TestScenario for KeyboardWizardScenario {
    fn name(&self) -> &'static str {
        "Keyboard-only validated domain wizard completion"
    }

    fn setup(&mut self, cx: &mut TestAppContext) -> Result<(), Box<dyn Error>> {
        bind_default_keys(cx);
        Ok(())
    }

    fn teardown(&mut self, _cx: &mut TestAppContext) -> Result<(), Box<dyn Error>> {
        gpui_ui_kit::clear_all_input_states();
        Ok(())
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        driver.update_app(|app, _| {
            app.ui_state.last_screen = Screen::Library;
            app.ui_state.current_screen = Screen::Recording;
            app.measurement_state.recording_state.step = RecordingStep::Saving;
        });
        driver.simulate_keystrokes("alt-right");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.ui_state.current_screen),
            Screen::Library
        );

        driver.update_app(|app, _| {
            app.ui_state.last_screen = Screen::Library;
            app.ui_state.current_screen = Screen::RoomEq;
            app.measurement_state.room_eq_state.step = RoomEqStep::Export;
        });
        driver.simulate_keystrokes("alt-right");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.ui_state.current_screen),
            Screen::Library
        );

        driver.update_app(|app, _| {
            app.ui_state.last_screen = Screen::Library;
            app.ui_state.current_screen = Screen::HeadphoneEq;
            app.measurement_state.headphone_eq_state.model.step = HeadphoneEqStep::Export;
        });
        driver.simulate_keystrokes("alt-right");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.ui_state.current_screen),
            Screen::Library
        );

        driver.update_app(|app, _| {
            app.ui_state.last_screen = Screen::Library;
            app.ui_state.current_screen = Screen::Spinorama;
            app.measurement_state.spinorama_eq_state.model.step = SpinoramaStep::Export;
        });
        driver.simulate_keystrokes("alt-right");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.ui_state.current_screen),
            Screen::Library
        );

        Ok(())
    }
}

impl TestScenario for KeyboardPluginRackScenario {
    fn name(&self) -> &'static str {
        "Keyboard-only plugin rack editing"
    }

    fn setup(&mut self, cx: &mut TestAppContext) -> Result<(), Box<dyn Error>> {
        bind_default_keys(cx);
        Ok(())
    }

    fn teardown(&mut self, _cx: &mut TestAppContext) -> Result<(), Box<dyn Error>> {
        gpui_ui_kit::clear_all_input_states();
        Ok(())
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);
        driver.navigate_to(Screen::Studio);
        driver.run_until_parked();

        let initial_count = driver.read_app(|app| app.plugin_state.graph.len());
        driver.simulate_keystrokes("!");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.plugin_state.graph.len()),
            initial_count + 1,
            "the documented quick-add shortcut did not add EQ"
        );
        assert_eq!(
            driver.read_app(|app| {
                app.plugin_state
                    .graph
                    .get_plugin(app.plugin_state.selected_plugin_index)
                    .map(|plugin| plugin.plugin_type())
            }),
            Some(PluginType::EQ)
        );

        driver.simulate_keystrokes("delete");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.plugin_state.graph.len()),
            initial_count,
            "Delete did not remove the selected rack plugin"
        );

        driver.update_app(|app, _| {
            app.plugin_state.add_plugin(&PluginType::Gain);
            app.plugin_state.editing_plugin_index = None;
            app.plugin_state.plugin_param_selection = 0;
        });
        driver.run_until_parked();

        let gain_index = driver.read_app(|app| app.plugin_state.selected_plugin_index);
        let gain_before = driver.read_app(move |app| {
            match &app
                .plugin_state
                .graph
                .get_plugin(gain_index)
                .expect("Gain should be present")
                .settings
            {
                PluginSettings::Gain { gain_db, .. } => *gain_db,
                other => panic!("expected Gain, found {other:?}"),
            }
        });

        driver.simulate_keystrokes("=");
        driver.run_until_parked();
        let (gain_after, editing_index) = driver.read_app(move |app| {
            let gain_db = match &app
                .plugin_state
                .graph
                .get_plugin(gain_index)
                .expect("Gain should still be present")
                .settings
            {
                PluginSettings::Gain { gain_db, .. } => *gain_db,
                other => panic!("expected Gain, found {other:?}"),
            };
            (gain_db, app.plugin_state.editing_plugin_index)
        });
        assert!(gain_after > gain_before, "= did not increase Gain");
        assert_eq!(editing_index, Some(gain_index));

        driver.simulate_keystrokes("right");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.plugin_state.plugin_param_selection),
            1,
            "Right did not select the next parameter"
        );
        driver.simulate_keystrokes("left");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.plugin_state.plugin_param_selection),
            0,
            "Left did not select the previous parameter"
        );

        let enabled_before = driver.read_app(move |app| {
            app.plugin_state
                .graph
                .get_plugin(gain_index)
                .expect("Gain should still be present")
                .enabled
        });
        driver.simulate_keystrokes("enter");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(move |app| {
                app.plugin_state
                    .graph
                    .get_plugin(gain_index)
                    .expect("Gain should still be present")
                    .enabled
            }),
            !enabled_before,
            "Enter did not toggle the selected plugin"
        );

        driver.simulate_keystrokes("up");
        driver.run_until_parked();
        assert_ne!(
            driver.read_app(|app| app.plugin_state.selected_plugin_index),
            gain_index,
            "Up did not select the previous plugin"
        );
        driver.simulate_keystrokes("down");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.plugin_state.selected_plugin_index),
            gain_index,
            "Down did not restore the next plugin selection"
        );

        driver.update_app(|app, _| {
            app.plugin_state.add_plugin(&PluginType::Gain);
        });
        driver.run_until_parked();
        let reorder_index = driver.read_app(|app| app.plugin_state.selected_plugin_index);
        assert!(
            reorder_index > gain_index,
            "the reorder fixture needs two adjacent user plugins"
        );

        driver.simulate_keystrokes("secondary-up");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.plugin_state.selected_plugin_index),
            reorder_index - 1,
            "Secondary+Up did not reorder the selected plugin"
        );
        driver.simulate_keystrokes("secondary-down");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.plugin_state.selected_plugin_index),
            reorder_index,
            "Secondary+Down did not restore the plugin order"
        );

        driver.simulate_keystrokes("delete");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.plugin_state.graph.len()),
            initial_count + 1,
            "Delete did not remove the reordered Gain plugin"
        );
        driver.simulate_keystrokes("up");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.plugin_state.selected_plugin_index),
            gain_index,
            "Up did not return to the remaining Gain plugin"
        );
        driver.simulate_keystrokes("delete");
        driver.run_until_parked();
        assert_eq!(
            driver.read_app(|app| app.plugin_state.graph.len()),
            initial_count,
            "Delete did not remove the keyboard-edited Gain plugin"
        );

        Ok(())
    }
}

#[gpui::test]
async fn whole_app_keyboard_navigation_and_overlays_work(cx: &mut TestAppContext) {
    E2ERunner::new(KeyboardWorkflowScenario)
        .run(cx)
        .await
        .unwrap();
}

#[gpui::test]
async fn keyboard_library_and_queue_operations_use_real_app_state(cx: &mut TestAppContext) {
    E2ERunner::new(KeyboardLibraryQueueScenario)
        .run(cx)
        .await
        .unwrap();
}

#[gpui::test]
async fn keyboard_completes_validated_domain_wizards(cx: &mut TestAppContext) {
    E2ERunner::new(KeyboardWizardScenario)
        .run(cx)
        .await
        .unwrap();
}

#[gpui::test]
async fn keyboard_completes_plugin_rack_editing(cx: &mut TestAppContext) {
    E2ERunner::new(KeyboardPluginRackScenario)
        .run(cx)
        .await
        .unwrap();
}

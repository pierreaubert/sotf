use crate::{driver::AppDriver, runner::TestScenario};
use gpui::{VisualTestContext, WindowHandle};
use sotf_audio::plugins::PluginSettings;
use sotf_audio::plugins::PluginType;
use sotf_audio_player_gpui::app::types::PluginUpdateType;
use sotf_audio_player_gpui::components::plugins::editing::PluginEditingManager;
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

pub struct FletcherMunsonScenario;

impl TestScenario for FletcherMunsonScenario {
    fn name(&self) -> &'static str {
        "fletcher_munson_param_verification"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // 1. Add Fletcher-Munson plugin (now merged into LoudnessCompensation with mode=2)
        driver.update_app(|app, _cx| {
            println!("Adding Fletcher-Munson plugin...");
            app.add_plugin(&PluginType::FletcherMunson);
            app.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
        });

        // 2. Modify Reference Level (Index 13 in LoudnessCompensation PARAMS)
        driver.update_app(|app, _cx| {
            println!("Modifying Reference Level...");
            // FletcherMunson is now LoudnessCompensation with mode=2
            let plugin_idx = app
                .plugin_state
                .graph
                .plugins()
                .iter()
                .position(|p: &&sotf_audio::plugins::Plugin| {
                    matches!(p.plugin_type(), PluginType::LoudnessCompensation)
                })
                .expect("LoudnessCompensation plugin not found after addition");

            println!("LoudnessCompensation (FM) found at index {}", plugin_idx);

            // Set Reference Level to 70.0 dB SPL (index 13 in LC PARAMS, range [60, 100])
            app.set_plugin_param(plugin_idx, 13, 70.0);
        });

        // 3. Verify Reference Level Update
        let _ = driver.read_app(|app| {
            println!("Verifying Reference Level...");
            let plugin_idx = app
                .plugin_state
                .graph
                .plugins()
                .iter()
                .position(|p: &&sotf_audio::plugins::Plugin| {
                    matches!(p.plugin_type(), PluginType::LoudnessCompensation)
                })
                .expect("LoudnessCompensation plugin not found for verification");

            let plugin = app
                .plugin_state
                .graph
                .get_plugin(plugin_idx)
                .ok_or("Plugin not found")?;

            if let PluginSettings::LoudnessCompensation {
                reference_level_db, ..
            } = plugin.settings
            {
                println!("Current Reference Level: {}", reference_level_db);
                if (reference_level_db - 70.0).abs() > 0.001 {
                    panic!(
                        "Reference level mismatch: expected 70.0, got {}",
                        reference_level_db
                    );
                }
            } else {
                panic!(
                    "Expected LoudnessCompensation plugin, found {:?}",
                    plugin.plugin_type()
                );
            }
            Ok::<(), Box<dyn Error>>(())
        });

        // 4. Modify Low Frequency (Index 0 in LoudnessCompensation PARAMS)
        driver.update_app(|app, _cx| {
            println!("Modifying Low Frequency...");
            let plugin_idx = app
                .plugin_state
                .graph
                .plugins()
                .iter()
                .position(|p: &&sotf_audio::plugins::Plugin| {
                    matches!(p.plugin_type(), PluginType::LoudnessCompensation)
                })
                .expect("LoudnessCompensation plugin not found");

            // Set Low Freq to 80 Hz (index 0 in LC PARAMS)
            app.set_plugin_param(plugin_idx, 0, 80.0);
        });

        // 5. Verify Low Frequency Update
        driver.read_app(|app| {
            println!("Verifying Low Frequency...");
            let plugin_idx = app
                .plugin_state
                .graph
                .plugins()
                .iter()
                .position(|p: &&sotf_audio::plugins::Plugin| {
                    matches!(p.plugin_type(), PluginType::LoudnessCompensation)
                })
                .expect("LoudnessCompensation plugin not found");

            let plugin = app
                .plugin_state
                .graph
                .get_plugin(plugin_idx)
                .expect("Plugin not found");

            if let PluginSettings::LoudnessCompensation { low_freq, .. } = plugin.settings {
                println!("Current Low Frequency: {}", low_freq);
                if (low_freq - 80.0).abs() > 0.001 {
                    panic!("Low frequency mismatch: expected 80.0, got {}", low_freq);
                }
            } else {
                panic!("Expected LoudnessCompensation plugin");
            }
        });

        println!("Test completed successfully.");

        Ok(())
    }
}

#[gpui::test]
async fn test_fletcher_munson(cx: &mut gpui::TestAppContext) {
    use crate::runner::E2ERunner;
    let scenario = FletcherMunsonScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await.unwrap();
    assert!(result.passed, "Test failed: {:?}", result.error_message);
}

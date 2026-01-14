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

        // 1. Add Fletcher-Munson plugin
        driver.update_app(|app, _cx| {
            println!("Adding Fletcher-Munson plugin...");
            app.add_plugin(&PluginType::FletcherMunson);
            app.plugin_state.pending_plugin_update = Some(PluginUpdateType::Structural);
        });

        // 2. Modify Reference Level (Index 1)
        driver.update_app(|app, _cx| {
            println!("Modifying Reference Level...");
            // Find Fletcher-Munson plugin index
            let plugin_idx = app
                .plugin_state
                .plugin_chain
                .plugins()
                .iter()
                .position(|p| matches!(p.plugin_type(), PluginType::FletcherMunson))
                .expect("Fletcher-Munson plugin not found after addition");

            println!("Fletcher-Munson found at index {}", plugin_idx);

            // Set Reference Level to -20.0 dB
            app.set_plugin_param(plugin_idx, 1, -20.0);
        });

        // 3. Verify Reference Level Update
        driver.read_app(|app| {
            println!("Verifying Reference Level...");
            let plugin_idx = app
                .plugin_state
                .plugin_chain
                .plugins()
                .iter()
                .position(|p| matches!(p.plugin_type(), PluginType::FletcherMunson))
                .expect("Fletcher-Munson plugin not found for verification");

            let plugin = app
                .plugin_state
                .plugin_chain
                .get_plugin(plugin_idx)
                .ok_or_else(|| "Plugin not found")?;

            if let PluginSettings::FletcherMunson {
                reference_level_db, ..
            } = plugin.settings
            {
                println!("Current Reference Level: {}", reference_level_db);
                if (reference_level_db - -20.0).abs() > 0.001 {
                    panic!(
                        "Reference level mismatch: expected -20.0, got {}",
                        reference_level_db
                    );
                }
            } else {
                panic!(
                    "Expected FletcherMunson plugin, found {:?}",
                    plugin.plugin_type()
                );
            }
            Ok::<(), Box<dyn Error>>(())
        });

        // 4. Modify Band 1 Frequency (Index 8)
        driver.update_app(|app, _cx| {
            println!("Modifying Band 1 Frequency...");
            let plugin_idx = app
                .plugin_state
                .plugin_chain
                .plugins()
                .iter()
                .position(|p| matches!(p.plugin_type(), PluginType::FletcherMunson))
                .expect("Fletcher-Munson plugin not found");

            // Set Band 1 Freq to 80 Hz
            app.set_plugin_param(plugin_idx, 8, 80.0);
        });

        // 5. Verify Band 1 Frequency Update
        driver.read_app(|app| {
            println!("Verifying Band 1 Frequency...");
            let plugin_idx = app
                .plugin_state
                .plugin_chain
                .plugins()
                .iter()
                .position(|p| matches!(p.plugin_type(), PluginType::FletcherMunson))
                .expect("Fletcher-Munson plugin not found");

            let plugin = app
                .plugin_state
                .plugin_chain
                .get_plugin(plugin_idx)
                .expect("Plugin not found");

            if let PluginSettings::FletcherMunson { band1_freq, .. } = plugin.settings {
                println!("Current Band 1 Frequency: {}", band1_freq);
                if (band1_freq - 80.0).abs() > 0.001 {
                    panic!(
                        "Band 1 frequency mismatch: expected 80.0, got {}",
                        band1_freq
                    );
                }
            } else {
                panic!("Expected FletcherMunson plugin");
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

//! E2E tests for Plugin Rack component.
//!
//! Tests for plugin chain management using real App state.

use crate::driver::AppDriver;
use crate::pages::plugin_rack::PluginRackPage;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio::plugins::PluginType;
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

// =============================================================================
// Add / Remove Plugin
// =============================================================================

struct AddRemovePluginScenario;

impl TestScenario for AddRemovePluginScenario {
    fn name(&self) -> &'static str {
        "Add Remove Plugin"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);
        let mut page = PluginRackPage::new(&mut driver);

        let initial_count = page.get_plugin_count();

        // Add EQ plugin
        let eq_id = page.add_plugin(PluginType::EQ);
        let eq_index = page
            .find_plugin_index_by_id(eq_id)
            .ok_or("EQ plugin not found after add")?;

        // Verify it was added
        if page.get_plugin_count() != initial_count + 1 {
            return Err("Plugin count should increase by 1".into());
        }

        // Verify type
        if page.get_plugin_type(eq_index) != Some(PluginType::EQ) {
            return Err("Plugin type should be EQ".into());
        }

        // Remove it
        page.remove_plugin(eq_index);

        // Verify removed
        if page.plugin_exists(eq_id) {
            return Err("Plugin should be removed".into());
        }

        if page.get_plugin_count() != initial_count {
            return Err("Plugin count should return to initial".into());
        }

        Ok(())
    }
}

// =============================================================================
// Toggle Plugin
// =============================================================================

struct TogglePluginScenario;

impl TestScenario for TogglePluginScenario {
    fn name(&self) -> &'static str {
        "Toggle Plugin"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);
        let mut page = PluginRackPage::new(&mut driver);

        // Add a plugin
        let id = page.add_plugin(PluginType::Compressor);
        let index = page
            .find_plugin_index_by_id(id)
            .ok_or("Compressor not found")?;

        // Should be enabled by default
        if !page.is_plugin_enabled(index) {
            return Err("Plugin should be enabled by default".into());
        }

        // Disable
        page.toggle_plugin(index);
        if page.is_plugin_enabled(index) {
            return Err("Plugin should be disabled after toggle".into());
        }

        // Re-enable
        page.toggle_plugin(index);
        if !page.is_plugin_enabled(index) {
            return Err("Plugin should be enabled after second toggle".into());
        }

        // Clean up
        page.remove_plugin(index);

        Ok(())
    }
}

// =============================================================================
// Multiple Plugins
// =============================================================================

struct MultiplePluginsScenario;

impl TestScenario for MultiplePluginsScenario {
    fn name(&self) -> &'static str {
        "Multiple Plugins"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);
        let mut page = PluginRackPage::new(&mut driver);

        let initial_count = page.get_plugin_count();

        // Add multiple plugins
        let eq_id = page.add_plugin(PluginType::EQ);
        let comp_id = page.add_plugin(PluginType::Compressor);
        let limiter_id = page.add_plugin(PluginType::Limiter);

        if page.get_plugin_count() != initial_count + 3 {
            return Err(format!(
                "Expected {} plugins, got {}",
                initial_count + 3,
                page.get_plugin_count()
            )
            .into());
        }

        // Disable middle plugin
        let comp_index = page
            .find_plugin_index_by_id(comp_id)
            .ok_or("Compressor not found")?;
        page.toggle_plugin(comp_index);

        let eq_index = page
            .find_plugin_index_by_id(eq_id)
            .ok_or("EQ not found")?;
        let limiter_index = page
            .find_plugin_index_by_id(limiter_id)
            .ok_or("Limiter not found")?;

        if !page.is_plugin_enabled(eq_index) {
            return Err("EQ should still be enabled".into());
        }
        if page.is_plugin_enabled(comp_index) {
            return Err("Compressor should be disabled".into());
        }
        if !page.is_plugin_enabled(limiter_index) {
            return Err("Limiter should still be enabled".into());
        }

        // Remove middle plugin
        page.remove_plugin(comp_index);

        if page.plugin_exists(comp_id) {
            return Err("Compressor should be removed".into());
        }

        // Clean up remaining — re-fetch indices since they may have shifted
        let eq_index = page
            .find_plugin_index_by_id(eq_id)
            .ok_or("EQ not found after remove")?;
        page.remove_plugin(eq_index);

        let limiter_index = page
            .find_plugin_index_by_id(limiter_id)
            .ok_or("Limiter not found after remove")?;
        page.remove_plugin(limiter_index);

        if page.get_plugin_count() != initial_count {
            return Err("Should return to initial count".into());
        }

        Ok(())
    }
}

// =============================================================================
// Permanent Plugins
// =============================================================================

struct PermanentPluginsScenario;

impl TestScenario for PermanentPluginsScenario {
    fn name(&self) -> &'static str {
        "Permanent Plugins"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);
        let mut page = PluginRackPage::new(&mut driver);

        let initial_count = page.get_plugin_count();

        // The rack should have permanent plugins from the start
        if initial_count == 0 {
            return Err("Rack should have permanent plugins (input monitor, output monitor, matrix)".into());
        }

        // Check that at least one permanent plugin exists
        let mut found_permanent = false;
        for i in 0..initial_count {
            if page.is_plugin_permanent(i) {
                found_permanent = true;
                break;
            }
        }

        if !found_permanent {
            return Err("Should have at least one permanent plugin".into());
        }

        Ok(())
    }
}

// =============================================================================
// Plugin Selection
// =============================================================================

struct PluginSelectionScenario;

impl TestScenario for PluginSelectionScenario {
    fn name(&self) -> &'static str {
        "Plugin Selection"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);
        let mut page = PluginRackPage::new(&mut driver);

        // Add two plugins so we have something to select between
        let eq_id = page.add_plugin(PluginType::EQ);
        let comp_id = page.add_plugin(PluginType::Compressor);

        let eq_index = page
            .find_plugin_index_by_id(eq_id)
            .ok_or("EQ not found")?;
        let comp_index = page
            .find_plugin_index_by_id(comp_id)
            .ok_or("Compressor not found")?;

        // Select EQ
        page.select_plugin(eq_index);

        // Select Compressor
        page.select_plugin(comp_index);

        // Clean up
        // Re-fetch indices after potential shifts
        let eq_index = page
            .find_plugin_index_by_id(eq_id)
            .ok_or("EQ not found for cleanup")?;
        page.remove_plugin(eq_index);

        let comp_index = page
            .find_plugin_index_by_id(comp_id)
            .ok_or("Compressor not found for cleanup")?;
        page.remove_plugin(comp_index);

        Ok(())
    }
}

// =============================================================================
// Output Channels
// =============================================================================

struct OutputChannelsScenario;

impl TestScenario for OutputChannelsScenario {
    fn name(&self) -> &'static str {
        "Output Channels"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);
        let mut page = PluginRackPage::new(&mut driver);

        // Default chain should have a valid output channel count
        let channels = page.get_output_channels();
        if channels == 0 {
            return Err("Output channels should be > 0".into());
        }

        Ok(())
    }
}

// =============================================================================
// Test Registration
// =============================================================================

#[gpui::test]
async fn test_add_remove_plugin(cx: &mut TestAppContext) {
    let scenario = AddRemovePluginScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Add/remove plugin test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_toggle_plugin(cx: &mut TestAppContext) {
    let scenario = TogglePluginScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Toggle plugin test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_multiple_plugins(cx: &mut TestAppContext) {
    let scenario = MultiplePluginsScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Multiple plugins test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_permanent_plugins(cx: &mut TestAppContext) {
    let scenario = PermanentPluginsScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Permanent plugins test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_plugin_selection(cx: &mut TestAppContext) {
    let scenario = PluginSelectionScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Plugin selection test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_output_channels(cx: &mut TestAppContext) {
    let scenario = OutputChannelsScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Output channels test failed: {:?}",
        result.err()
    );
}

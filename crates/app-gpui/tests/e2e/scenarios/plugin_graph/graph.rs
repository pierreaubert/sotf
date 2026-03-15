//! E2E tests for Plugin Graph component.
//!
//! Tests for the node-based plugin graph editor using real App state.

use crate::driver::AppDriver;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::Screen;
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

// =============================================================================
// Plugin Graph Screen Navigation
// =============================================================================

struct NavigateToPluginGraphScenario;

impl TestScenario for NavigateToPluginGraphScenario {
    fn name(&self) -> &'static str {
        "Navigate to Plugin Graph"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Start at Library screen
        let screen = driver.read_app(|app| app.ui_state.current_screen);
        if screen != Screen::Library {
            return Err(format!("Expected Library screen, got {:?}", screen).into());
        }

        // Navigate to PluginGraph screen
        driver.navigate_to(Screen::PluginGraph);

        // Verify navigation worked
        let screen = driver.read_app(|app| app.ui_state.current_screen);
        if screen != Screen::PluginGraph {
            return Err(format!("Expected PluginGraph screen, got {:?}", screen).into());
        }

        Ok(())
    }
}

// =============================================================================
// Default Graph Structure
// =============================================================================

struct DefaultGraphStructureScenario;

impl TestScenario for DefaultGraphStructureScenario {
    fn name(&self) -> &'static str {
        "Default Graph Structure"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Navigate to PluginGraph screen
        driver.navigate_to(Screen::PluginGraph);

        // The default graph should have a workflow canvas
        let has_canvas = driver.read_app(|app| {
            app.plugin_state.workflow_canvas.is_some()
        });

        if !has_canvas {
            return Err("Workflow canvas should be created on PluginGraph screen".into());
        }

        // Verify the default chain has plugins (at least EQ by default)
        let chain_len = driver.read_app(|app| app.plugin_state.chain.len());
        if chain_len == 0 {
            return Err("Default chain should have at least one plugin (EQ)".into());
        }

        // Verify there's at least one plugin in the chain
        let has_plugins = chain_len > 0;
        if !has_plugins {
            return Err("Should have plugins in the chain".into());
        }

        Ok(())
    }
}

// =============================================================================
// Plugin Graph Header Stats
// =============================================================================

struct GraphHeaderStatsScenario;

impl TestScenario for GraphHeaderStatsScenario {
    fn name(&self) -> &'static str {
        "Graph Header Stats"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Navigate to PluginGraph screen
        driver.navigate_to(Screen::PluginGraph);

        // The graph should have a valid plugin graph or default created
        let plugin_graph_exists = driver.read_app(|app| {
            app.plugin_state.plugin_graph.is_some()
                || !app.plugin_state.chain.is_empty()
        });

        if !plugin_graph_exists {
            return Err("Should have a plugin graph or chain".into());
        }

        // Verify output channels are set
        let output_channels = driver.read_app(|app| {
            app.audio_device_state.output_devices
                .get(app.audio_device_state.selected_output_device_index)
                .and_then(|d| d.default_config.as_ref())
                .map(|c| c.channels as usize)
                .unwrap_or(2)
        });

        if output_channels == 0 {
            return Err("Should have valid output channels".into());
        }

        Ok(())
    }
}

// =============================================================================
// Palette Elements
// =============================================================================

struct PaletteElementsScenario;

impl TestScenario for PaletteElementsScenario {
    fn name(&self) -> &'static str {
        "Palette Elements"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Navigate to PluginGraph screen
        driver.navigate_to(Screen::PluginGraph);

        // The plugin_state should have plugin types available for the palette
        // We can verify this by checking the chain contains expected plugin types
        let chain_len = driver.read_app(|app| app.plugin_state.chain.len());

        // The default setup should have at least one plugin (EQ)
        if chain_len == 0 {
            return Err("Chain should have at least one plugin".into());
        }

        // Verify first plugin is EQ (the default)
        let first_is_eq = driver.read_app(|app| {
            app.plugin_state
                .chain
                .plugins()
                .get(0)
                .map(|p| p.plugin_type() == sotf_audio::plugins::PluginType::EQ)
                .unwrap_or(false)
        });

        if !first_is_eq {
            return Err("First plugin should be EQ (default)".into());
        }

        Ok(())
    }
}

// =============================================================================
// Home Button Navigation
// =============================================================================

struct HomeButtonNavigationScenario;

impl TestScenario for HomeButtonNavigationScenario {
    fn name(&self) -> &'static str {
        "Home Button Navigation"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Navigate to PluginGraph screen
        driver.navigate_to(Screen::PluginGraph);

        // Verify we're on PluginGraph
        let screen = driver.read_app(|app| app.ui_state.current_screen);
        if screen != Screen::PluginGraph {
            return Err(format!("Expected PluginGraph screen, got {:?}", screen).into());
        }

        // Navigate back to Library via direct state change (simulating home button)
        driver.navigate_to(Screen::Library);

        // Verify we're back on Library
        let screen = driver.read_app(|app| app.ui_state.current_screen);
        if screen != Screen::Library {
            return Err(format!("Expected Library screen after home, got {:?}", screen).into());
        }

        Ok(())
    }
}

// =============================================================================
// Multiple Screen Navigation
// =============================================================================

struct MultipleScreenNavigationScenario;

impl TestScenario for MultipleScreenNavigationScenario {
    fn name(&self) -> &'static str {
        "Multiple Screen Navigation"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Start at Library
        let screen = driver.read_app(|app| app.ui_state.current_screen);
        if screen != Screen::Library {
            return Err(format!("Expected Library screen, got {:?}", screen).into());
        }

        // Navigate to PluginGraph
        driver.navigate_to(Screen::PluginGraph);
        let screen = driver.read_app(|app| app.ui_state.current_screen);
        if screen != Screen::PluginGraph {
            return Err(format!("Expected PluginGraph screen, got {:?}", screen).into());
        }

        // Navigate back to Library
        driver.navigate_to(Screen::Library);
        let screen = driver.read_app(|app| app.ui_state.current_screen);
        if screen != Screen::Library {
            return Err(format!("Expected Library screen, got {:?}", screen).into());
        }

        // Navigate to PluginGraph again
        driver.navigate_to(Screen::PluginGraph);
        let screen = driver.read_app(|app| app.ui_state.current_screen);
        if screen != Screen::PluginGraph {
            return Err(format!("Expected PluginGraph screen, got {:?}", screen).into());
        }

        Ok(())
    }
}

// =============================================================================
// Test Registration
// =============================================================================

#[gpui::test]
async fn test_navigate_to_plugin_graph(cx: &mut TestAppContext) {
    let scenario = NavigateToPluginGraphScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Navigate to plugin graph test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_default_graph_structure(cx: &mut TestAppContext) {
    let scenario = DefaultGraphStructureScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Default graph structure test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_graph_header_stats(cx: &mut TestAppContext) {
    let scenario = GraphHeaderStatsScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Graph header stats test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_palette_elements(cx: &mut TestAppContext) {
    let scenario = PaletteElementsScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Palette elements test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_home_button_navigation(cx: &mut TestAppContext) {
    let scenario = HomeButtonNavigationScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Home button navigation test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_multiple_screen_navigation(cx: &mut TestAppContext) {
    let scenario = MultipleScreenNavigationScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Multiple screen navigation test failed: {:?}",
        result.err()
    );
}

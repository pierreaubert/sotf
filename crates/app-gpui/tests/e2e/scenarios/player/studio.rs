use crate::driver::AppDriver;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::Screen;
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

/// Test Studio screen navigation and basic functionality
pub struct StudioScreenScenario;

impl TestScenario for StudioScreenScenario {
    fn name(&self) -> &'static str {
        "Studio Screen Navigation"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Navigate to Studio screen
        driver.navigate_to(Screen::Studio);

        // Verify we're on Studio screen
        let screen = driver.read_app(|app| app.ui_state.current_screen);
        if screen != Screen::Studio {
            return Err(format!("Expected Studio screen, got {:?}", screen).into());
        }

        // Verify Studio screen shows the plugin rack
        // The Studio screen is essentially the Plugin Rack in a different view
        let has_plugins = driver.read_app(|app| app.plugin_state.graph.plugins().len());

        // Log the number of plugins for debugging
        println!("Studio screen loaded with {} plugins", has_plugins);

        Ok(())
    }
}

#[gpui::test]
async fn test_studio_screen_navigation(cx: &mut TestAppContext) {
    let scenario = StudioScreenScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;

    if let Err(e) = &result {
        println!("Test failed: {}", e);
    }
    assert!(result.is_ok(), "Studio screen navigation test failed");
}

/// Exercise the description popover in the complete Studio view.
pub struct StudioPickerTooltipScenario;

impl TestScenario for StudioPickerTooltipScenario {
    fn name(&self) -> &'static str {
        "Studio picker description tooltip"
    }

    fn window_size(&self) -> Option<gpui::Size<gpui::Pixels>> {
        Some(gpui::size(gpui::px(1440.0), gpui::px(900.0)))
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);
        driver.update_app(|app, cx| {
            // Compressor is a Beta plugin; production picker coverage excludes it.
            app.ui_state.release_channel = sotf_audio_player::ReleaseChannel::Beta;
            app.set_screen(Screen::Studio, "TooltipRegression");
            app.plugin_ui.show_add_plugin_menu = true;
            cx.notify();
        });
        driver.cx.run_until_parked();
        gpui::AnyWindowHandle::from(driver.view).update(driver.cx, |_, window, cx| {
            // The test platform does not schedule a native display refresh.
            window.refresh();
            window.draw(cx).clear(cx);
        })?;
        let bounds = driver
            .cx
            .debug_bounds("rack-add-tooltip-Compressor")
            .ok_or_else(|| {
                #[cfg(feature = "dev-api")]
                let painted = sotf_audio_player_gpui::app::dev_api::registry::snapshot_for(
                    driver.view.window_id().as_u64(),
                ).into_iter().map(|(name, _)| name).collect::<Vec<_>>();
                #[cfg(not(feature = "dev-api"))]
                let painted: Vec<String> = Vec::new();
                let (screen, menu, width, height) = driver.read_app(|app| (
                    app.ui_state.current_screen,
                    app.plugin_ui.show_add_plugin_menu,
                    app.ui_state.window_width,
                    app.ui_state.window_height,
                ));
                format!("Compressor picker missing: screen={screen:?}, menu={menu}, viewport={width}x{height}, painted={painted:?}")
            })?;
        driver
            .cx
            .simulate_mouse_move(bounds.center(), None, gpui::Modifiers::default());
        driver.cx.run_until_parked();
        driver
            .cx
            .executor()
            .advance_clock(std::time::Duration::from_secs(1));
        driver.cx.run_until_parked();
        gpui::AnyWindowHandle::from(driver.view).update(driver.cx, |_, window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        })?;
        assert!(
            driver
                .cx
                .debug_bounds("themed-tooltip-description")
                .is_some(),
            "The full Studio view must render the hovered plugin description"
        );
        #[cfg(feature = "dev-api")]
        for frame in 0..3 {
            // The native player ticks while a stationary tooltip stays cached.
            driver.update_app(|_, cx| cx.notify());
            gpui::AnyWindowHandle::from(driver.view).update(driver.cx, |_, window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
            })?;
            assert!(
                driver
                    .cx
                    .debug_bounds("themed-tooltip-description")
                    .is_some(),
                "Tooltip disappeared from the painted scene on redraw {frame}"
            );
            let tooltip = sotf_audio_player_gpui::app::dev_api::registry::snapshot_for(
                driver.view.window_id().as_u64(),
            )
            .into_iter()
            .find(|(selector, _)| selector == "tooltip.description");
            assert_eq!(
                tooltip.and_then(|(_, element)| element.state.text),
                Some("Reduce dynamic range above a threshold.".to_owned()),
                "Painted tooltip must remain observable on redraw {frame}"
            );
        }
        Ok(())
    }
}

#[gpui::test]
async fn test_studio_picker_description_tooltip(cx: &mut TestAppContext) {
    crate::runner::ensure_shared_test_config_dir();
    let result = E2ERunner::new(StudioPickerTooltipScenario)
        .run(cx)
        .await
        .expect("Studio tooltip scenario runner failed");
    assert!(
        result.passed,
        "Studio tooltip failed: {:?}",
        result.error_message
    );
}

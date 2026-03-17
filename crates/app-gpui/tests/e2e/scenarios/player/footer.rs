//! E2E tests for Footer component.
//!
//! Tests for verifying footer state through real App state.

use crate::driver::AppDriver;
use crate::pages::footer::FooterPage;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::Screen;
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

// =============================================================================
// Pure utility tests (no GPUI context needed)
// =============================================================================

#[test]
fn test_time_formatting() {
    let format_time = |secs: f64| -> String {
        let mins = (secs / 60.0) as u32;
        let s = (secs % 60.0) as u32;
        format!("{:02}:{:02}", mins, s)
    };

    assert_eq!(format_time(0.0), "00:00");
    assert_eq!(format_time(59.0), "00:59");
    assert_eq!(format_time(60.0), "01:00");
    assert_eq!(format_time(90.0), "01:30");
    assert_eq!(format_time(3600.0), "60:00");
}

// =============================================================================
// Real E2E tests
// =============================================================================

struct FooterPlaybackStateScenario;

impl TestScenario for FooterPlaybackStateScenario {
    fn name(&self) -> &'static str {
        "Footer Playback State"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        {
            let mut footer = FooterPage::new(&mut driver);

            // Initially not playing
            if footer.is_playing() {
                return Err("Should not be playing initially".into());
            }

            // Initially not muted
            if footer.is_muted() {
                return Err("Should not be muted initially".into());
            }

            // Initial volume should be 0.1
            let volume = footer.get_volume();
            if (volume - 0.1).abs() > 0.001 {
                return Err(format!("Expected volume ~0.1, got {}", volume).into());
            }

            // No track should be playing
            let title = footer.get_current_track_title();
            if title.is_some() {
                return Err("Should have no current track initially".into());
            }

            // Initial playback position should be 0
            let position = footer.get_playback_position();
            if position.abs() > 0.001 {
                return Err(format!("Expected position ~0.0, got {}", position).into());
            }
        }

        Ok(())
    }
}

struct FooterVolumeScenario;

impl TestScenario for FooterVolumeScenario {
    fn name(&self) -> &'static str {
        "Footer Volume"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Set volume and verify through FooterPage
        driver.update_app(|app, _| {
            app.playback.volume = 0.75;
        });

        {
            let mut footer = FooterPage::new(&mut driver);
            let volume = footer.get_volume();
            if (volume - 0.75).abs() > 0.001 {
                return Err(format!("Expected volume 0.75, got {}", volume).into());
            }
        }

        // Mute and verify
        driver.update_app(|app, _| {
            app.playback.muted = true;
        });

        {
            let mut footer = FooterPage::new(&mut driver);
            if !footer.is_muted() {
                return Err("Should be muted".into());
            }

            // Volume should be preserved while muted
            let volume = footer.get_volume();
            if (volume - 0.75).abs() > 0.001 {
                return Err(
                    format!("Volume should be preserved while muted, got {}", volume).into(),
                );
            }
        }

        Ok(())
    }
}

struct FooterPlaybackToggleScenario;

impl TestScenario for FooterPlaybackToggleScenario {
    fn name(&self) -> &'static str {
        "Footer Playback Toggle"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Initially not playing
        {
            let mut footer = FooterPage::new(&mut driver);
            if footer.is_playing() {
                return Err("Should not be playing initially".into());
            }
        }

        // Set playing
        driver.update_app(|app, _| {
            app.playback.is_playing = true;
        });

        {
            let mut footer = FooterPage::new(&mut driver);
            if !footer.is_playing() {
                return Err("Should be playing after toggle".into());
            }
        }

        // Set paused
        driver.update_app(|app, _| {
            app.playback.is_playing = false;
        });

        {
            let mut footer = FooterPage::new(&mut driver);
            if footer.is_playing() {
                return Err("Should be paused after second toggle".into());
            }
        }

        Ok(())
    }
}

struct FooterDevicePopupScenario;

impl TestScenario for FooterDevicePopupScenario {
    fn name(&self) -> &'static str {
        "Footer Device Popup"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Device popup initially closed
        let show_popup = driver.read_app(|app| app.ui_state.show_device_popup);
        if show_popup {
            return Err("Device popup should be closed initially".into());
        }

        // Open device popup
        driver.update_app(|app, _| {
            app.ui_state.show_device_popup = true;
        });
        let show_popup = driver.read_app(|app| app.ui_state.show_device_popup);
        if !show_popup {
            return Err("Device popup should be open".into());
        }

        // Close device popup
        driver.update_app(|app, _| {
            app.ui_state.show_device_popup = false;
        });
        let show_popup = driver.read_app(|app| app.ui_state.show_device_popup);
        if show_popup {
            return Err("Device popup should be closed after toggle".into());
        }

        Ok(())
    }
}

struct FooterStudioMenuScenario;

impl TestScenario for FooterStudioMenuScenario {
    fn name(&self) -> &'static str {
        "Footer Studio Menu"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Studio menu initially closed
        let show_menu = driver.read_app(|app| app.ui_state.show_studio_menu);
        if show_menu {
            return Err("Studio menu should be closed initially".into());
        }

        // Open studio menu
        driver.update_app(|app, _| {
            app.ui_state.show_studio_menu = true;
        });
        let show_menu = driver.read_app(|app| app.ui_state.show_studio_menu);
        if !show_menu {
            return Err("Studio menu should be open".into());
        }

        // Navigate to Studio screen from studio menu
        driver.navigate_to(Screen::Studio);
        let screen = driver.read_app(|app| app.ui_state.current_screen);
        if screen != Screen::Studio {
            return Err(format!("Expected Studio screen, got {:?}", screen).into());
        }

        // Navigate to Recording screen from studio menu
        driver.navigate_to(Screen::Recording);
        let screen = driver.read_app(|app| app.ui_state.current_screen);
        if screen != Screen::Recording {
            return Err(format!("Expected Recording screen, got {:?}", screen).into());
        }

        // Navigate to PluginGraph screen from studio menu
        driver.navigate_to(Screen::PluginGraph);
        let screen = driver.read_app(|app| app.ui_state.current_screen);
        if screen != Screen::PluginGraph {
            return Err(format!("Expected PluginGraph screen, got {:?}", screen).into());
        }

        Ok(())
    }
}

struct FooterPlaybackPositionScenario;

impl TestScenario for FooterPlaybackPositionScenario {
    fn name(&self) -> &'static str {
        "Footer Playback Position"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Set position and duration
        driver.update_app(|app, _| {
            app.playback.position_secs = 90.0;
            app.playback.duration_secs = 300.0;
        });

        {
            let mut footer = FooterPage::new(&mut driver);
            let position = footer.get_playback_position();
            if (position - 90.0).abs() > 0.001 {
                return Err(format!("Expected position 90.0, got {}", position).into());
            }
        }

        // Verify duration through direct read
        let duration = driver.read_app(|app| app.playback.duration_secs);
        if (duration - 300.0).abs() > 0.001 {
            return Err(format!("Expected duration 300.0, got {}", duration).into());
        }

        // Advance position
        driver.update_app(|app, _| {
            app.playback.position_secs = 150.0;
        });

        {
            let mut footer = FooterPage::new(&mut driver);
            let position = footer.get_playback_position();
            if (position - 150.0).abs() > 0.001 {
                return Err(format!("Expected position 150.0, got {}", position).into());
            }
        }

        Ok(())
    }
}

#[gpui::test]
async fn test_footer_playback_state(cx: &mut TestAppContext) {
    let scenario = FooterPlaybackStateScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Footer playback state test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_footer_volume(cx: &mut TestAppContext) {
    let scenario = FooterVolumeScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Footer volume test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_footer_playback_toggle(cx: &mut TestAppContext) {
    let scenario = FooterPlaybackToggleScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Footer playback toggle test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_footer_device_popup(cx: &mut TestAppContext) {
    let scenario = FooterDevicePopupScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Footer device popup test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_footer_studio_menu(cx: &mut TestAppContext) {
    let scenario = FooterStudioMenuScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Footer studio menu test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_footer_playback_position(cx: &mut TestAppContext) {
    let scenario = FooterPlaybackPositionScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Footer playback position test failed: {:?}",
        result.err()
    );
}

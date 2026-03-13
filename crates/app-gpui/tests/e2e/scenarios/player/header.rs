//! E2E tests for Header Component.
//!
//! Tests for the application header using real App state.

use crate::driver::AppDriver;
use crate::pages::header::HeaderPage;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::{ActiveMenu, InputMode, Screen};
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

struct ScreenNavigationScenario;

impl TestScenario for ScreenNavigationScenario {
    fn name(&self) -> &'static str {
        "Screen Navigation"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);
        let mut header = HeaderPage::new(&mut driver);

        // Default screen is Library
        let screen = header.get_current_screen();
        if screen != Screen::Library {
            return Err(format!("Expected Library, got {:?}", screen).into());
        }

        // Navigate through screens
        let screens = [
            Screen::Queue,
            Screen::Spectrum,
            Screen::Studio,
            Screen::Settings,
            Screen::Library,
        ];

        for &target in &screens {
            header.navigate_to(target);
            let current = header.get_current_screen();
            if current != target {
                return Err(format!("Expected {:?}, got {:?}", target, current).into());
            }
        }

        Ok(())
    }
}

struct MenuStateScenario;

impl TestScenario for MenuStateScenario {
    fn name(&self) -> &'static str {
        "Menu State"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // No menu open initially
        let menu = driver.read_app(|app| app.ui_state.active_menu);
        if menu != ActiveMenu::None {
            return Err(format!("Expected no menu open, got {:?}", menu).into());
        }

        // Open file menu
        driver.update_app(|app, _| {
            app.ui_state.active_menu = ActiveMenu::File;
        });
        let menu = driver.read_app(|app| app.ui_state.active_menu);
        if menu != ActiveMenu::File {
            return Err(format!("Expected File menu, got {:?}", menu).into());
        }

        // Switch to show menu (exclusive)
        driver.update_app(|app, _| {
            app.ui_state.active_menu = ActiveMenu::Show;
        });
        let menu = driver.read_app(|app| app.ui_state.active_menu);
        if menu != ActiveMenu::Show {
            return Err(format!("Expected Show menu, got {:?}", menu).into());
        }

        // Switch to help menu
        driver.update_app(|app, _| {
            app.ui_state.active_menu = ActiveMenu::Help;
        });
        let menu = driver.read_app(|app| app.ui_state.active_menu);
        if menu != ActiveMenu::Help {
            return Err(format!("Expected Help menu, got {:?}", menu).into());
        }

        // Close menu
        driver.update_app(|app, _| {
            app.ui_state.active_menu = ActiveMenu::None;
        });
        let menu = driver.read_app(|app| app.ui_state.active_menu);
        if menu != ActiveMenu::None {
            return Err("Menu should be closed".into());
        }

        Ok(())
    }
}

struct MenuExclusivityScenario;

impl TestScenario for MenuExclusivityScenario {
    fn name(&self) -> &'static str {
        "Menu Exclusivity"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Initially no menu open
        {
            let mut header = HeaderPage::new(&mut driver);
            if header.is_menu_open() {
                return Err("No menu should be open initially".into());
            }
        }

        // Open File menu
        driver.update_app(|app, _| {
            app.ui_state.active_menu = ActiveMenu::File;
        });

        {
            let mut header = HeaderPage::new(&mut driver);
            if !header.is_menu_open() {
                return Err("Menu should be open after setting File".into());
            }
            let open_menu = header.get_open_menu();
            if open_menu != ActiveMenu::File {
                return Err(format!("Expected File, got {:?}", open_menu).into());
            }
        }

        // Switching to AddPlugin replaces File
        driver.update_app(|app, _| {
            app.ui_state.active_menu = ActiveMenu::AddPlugin;
        });

        {
            let mut header = HeaderPage::new(&mut driver);
            let open_menu = header.get_open_menu();
            if open_menu != ActiveMenu::AddPlugin {
                return Err(format!("Expected AddPlugin, got {:?}", open_menu).into());
            }
        }

        Ok(())
    }
}

struct SearchFocusScenario;

impl TestScenario for SearchFocusScenario {
    fn name(&self) -> &'static str {
        "Search Focus"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Default input mode is Normal
        let mode = driver.read_app(|app| app.ui_state.input_mode);
        if mode != InputMode::Normal {
            return Err(format!("Expected Normal input mode, got {:?}", mode).into());
        }

        // Enter search mode
        driver.update_app(|app, _| {
            app.ui_state.input_mode = InputMode::Search;
        });
        let mode = driver.read_app(|app| app.ui_state.input_mode);
        if mode != InputMode::Search {
            return Err(format!("Expected Search input mode, got {:?}", mode).into());
        }

        // Exit search mode
        driver.update_app(|app, _| {
            app.ui_state.input_mode = InputMode::Normal;
        });
        let mode = driver.read_app(|app| app.ui_state.input_mode);
        if mode != InputMode::Normal {
            return Err("Expected Normal input mode after exit".into());
        }

        Ok(())
    }
}

struct InputModeTransitionsScenario;

impl TestScenario for InputModeTransitionsScenario {
    fn name(&self) -> &'static str {
        "Input Mode Transitions"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Cycle through several input modes
        let modes = [
            InputMode::Search,
            InputMode::Help,
            InputMode::About,
            InputMode::KeyboardShortcuts,
            InputMode::Normal,
        ];

        for &target_mode in &modes {
            driver.update_app(|app, _| {
                app.ui_state.input_mode = target_mode;
            });
            let current = driver.read_app(|app| app.ui_state.input_mode);
            if current != target_mode {
                return Err(
                    format!("Expected {:?} input mode, got {:?}", target_mode, current).into(),
                );
            }
        }

        Ok(())
    }
}

#[gpui::test]
async fn test_screen_navigation(cx: &mut TestAppContext) {
    let scenario = ScreenNavigationScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Screen navigation failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_menu_state(cx: &mut TestAppContext) {
    let scenario = MenuStateScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(result.is_ok(), "Menu state test failed: {:?}", result.err());
}

#[gpui::test]
async fn test_menu_exclusivity(cx: &mut TestAppContext) {
    let scenario = MenuExclusivityScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Menu exclusivity test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_search_focus(cx: &mut TestAppContext) {
    let scenario = SearchFocusScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Search focus test failed: {:?}",
        result.err()
    );
}

#[gpui::test]
async fn test_input_mode_transitions(cx: &mut TestAppContext) {
    let scenario = InputModeTransitionsScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Input mode transitions test failed: {:?}",
        result.err()
    );
}

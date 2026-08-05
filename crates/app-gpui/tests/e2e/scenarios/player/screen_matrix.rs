use crate::driver::AppDriver;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{
    Modifiers, MouseButton, Pixels, Size, TestAppContext, VisualTestContext, WindowHandle, px, size,
};
use sotf_audio_player_gpui::Screen;
use sotf_audio_player_gpui::app::state::plugin::EarTrainingSurface;
use sotf_audio_player_gpui::i18n::Language;
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

struct ScreenMatrixScenario {
    window_size: Size<Pixels>,
}

impl ScreenMatrixScenario {
    fn new(width: f32, height: f32) -> Self {
        Self {
            window_size: size(px(width), px(height)),
        }
    }
}

impl TestScenario for ScreenMatrixScenario {
    fn name(&self) -> &'static str {
        "All GPUI screens responsive render matrix"
    }

    fn window_size(&self) -> Option<Size<Pixels>> {
        Some(self.window_size)
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        for language in Language::all() {
            driver.update_app(|app, _| app.set_language(*language));
            driver.run_until_parked();

            for screen in Screen::all() {
                driver.navigate_to(*screen);
                driver.run_until_parked();
                assert_eq!(
                    driver.read_app(|app| app.ui_state.current_screen),
                    *screen,
                    "{screen:?} did not remain active after rendering in {}",
                    language.code()
                );
            }
        }

        Ok(())
    }
}

#[gpui::test]
async fn every_screen_renders_at_normal_compact_and_wide_boundaries(cx: &mut TestAppContext) {
    for (width, height) in [(1200.0, 900.0), (700.0, 900.0), (1600.0, 1000.0)] {
        let runner = E2ERunner::new(ScreenMatrixScenario::new(width, height));
        runner.run(cx).await.unwrap();
    }
}

struct ListeningToolsSidebarScenario {
    window_size: Size<Pixels>,
}

impl ListeningToolsSidebarScenario {
    fn new(width: f32, height: f32) -> Self {
        Self {
            window_size: size(px(width), px(height)),
        }
    }
}

impl TestScenario for ListeningToolsSidebarScenario {
    fn name(&self) -> &'static str {
        "Learning and A/B tools route to distinct listening surfaces"
    }

    fn window_size(&self) -> Option<Size<Pixels>> {
        Some(self.window_size)
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        click_debug_element(&mut driver, "nav-learning")?;
        assert_eq!(
            driver.read_app(|app| app.ui_state.current_screen),
            Screen::ListeningTest
        );
        assert_eq!(
            driver.read_app(|app| app.plugin_state.listening_test_state.surface),
            EarTrainingSurface::EqBands
        );
        assert!(
            driver
                .cx
                .debug_bounds("ear-training-learning-navigation")
                .is_some()
        );

        click_debug_element(&mut driver, "ear-training-courses")?;
        assert_eq!(
            driver.read_app(|app| app.plugin_state.listening_test_state.surface),
            EarTrainingSurface::Courses
        );

        click_debug_element(&mut driver, "nav-ab-compare")?;
        assert_eq!(
            driver.read_app(|app| app.plugin_state.listening_test_state.surface),
            EarTrainingSurface::BlindComparison
        );
        assert!(driver.cx.debug_bounds("listening-setup-steps").is_some());
        assert!(
            driver
                .cx
                .debug_bounds("ear-training-learning-navigation")
                .is_none()
        );

        click_debug_element(&mut driver, "nav-learning")?;
        assert_eq!(
            driver.read_app(|app| app.plugin_state.listening_test_state.surface),
            EarTrainingSurface::EqBands
        );

        Ok(())
    }
}

fn click_debug_element(
    driver: &mut AppDriver<'_>,
    selector: &'static str,
) -> Result<(), Box<dyn Error>> {
    let bounds = driver
        .cx
        .debug_bounds(selector)
        .ok_or_else(|| format!("{selector} bounds should be available"))?;
    let center = bounds.center();
    driver
        .cx
        .simulate_mouse_down(center, MouseButton::Left, Modifiers::default());
    driver
        .cx
        .simulate_mouse_up(center, MouseButton::Left, Modifiers::default());
    driver.run_until_parked();
    Ok(())
}

#[gpui::test]
async fn learning_and_ab_tools_open_the_right_experience(cx: &mut TestAppContext) {
    for (width, height) in [(1200.0, 900.0), (700.0, 800.0)] {
        E2ERunner::new(ListeningToolsSidebarScenario::new(width, height))
            .run(cx)
            .await
            .unwrap();
    }
}

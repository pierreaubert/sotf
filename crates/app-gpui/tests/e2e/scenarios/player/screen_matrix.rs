use crate::driver::AppDriver;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{Pixels, Size, TestAppContext, VisualTestContext, WindowHandle, px, size};
use sotf_audio_player_gpui::Screen;
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

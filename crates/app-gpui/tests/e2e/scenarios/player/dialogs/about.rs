//! E2E tests for About Dialog.

use crate::driver::AppDriver;
use crate::pages::dialogs::DialogsPage;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::InputMode;
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

struct AboutDialogScenario;

impl TestScenario for AboutDialogScenario {
    fn name(&self) -> &'static str {
        "About Dialog"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Dialog should not be open initially
        {
            let mut dialogs = DialogsPage::new(&mut driver);
            if dialogs.is_about_dialog_open() {
                return Err("About dialog should not be open initially".into());
            }
        }

        // Open about dialog
        driver.update_app(|app, _| {
            app.ui_state.input_mode = InputMode::About;
        });
        driver.run_until_parked();

        {
            let mut dialogs = DialogsPage::new(&mut driver);
            if !dialogs.is_about_dialog_open() {
                return Err("About dialog should be open".into());
            }
        }

        // Close about dialog
        driver.update_app(|app, _| {
            app.ui_state.input_mode = InputMode::Normal;
        });
        driver.run_until_parked();

        {
            let mut dialogs = DialogsPage::new(&mut driver);
            if dialogs.is_about_dialog_open() {
                return Err("About dialog should be closed".into());
            }
        }

        Ok(())
    }
}

#[gpui::test]
async fn test_about_dialog(cx: &mut TestAppContext) {
    let scenario = AboutDialogScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "About dialog test failed: {:?}",
        result.err()
    );
}

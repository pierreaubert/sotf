//! E2E tests for Help Dialog.

use crate::driver::AppDriver;
use crate::pages::dialogs::DialogsPage;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::InputMode;
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

struct HelpDialogScenario;

impl TestScenario for HelpDialogScenario {
    fn name(&self) -> &'static str {
        "Help Dialog"
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
            if dialogs.is_shortcuts_dialog_open() {
                return Err("Help dialog should not be open initially".into());
            }
        }

        // Open help dialog
        driver.update_app(|app, _| {
            app.ui_state.input_mode = InputMode::KeyboardShortcuts;
        });
        driver.run_until_parked();

        {
            let mut dialogs = DialogsPage::new(&mut driver);
            if !dialogs.is_shortcuts_dialog_open() {
                return Err("Help dialog should be open".into());
            }
        }

        // Close via close_dialogs helper
        {
            let mut dialogs = DialogsPage::new(&mut driver);
            dialogs.close_dialogs();
        }

        {
            let mut dialogs = DialogsPage::new(&mut driver);
            if dialogs.is_shortcuts_dialog_open() {
                return Err("Help dialog should be closed after close_dialogs".into());
            }
        }

        Ok(())
    }
}

#[gpui::test]
async fn test_help_dialog(cx: &mut TestAppContext) {
    let scenario = HelpDialogScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    assert!(
        result.is_ok(),
        "Help dialog test failed: {:?}",
        result.err()
    );
}

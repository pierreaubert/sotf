use crate::driver::AppDriver;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::{InputMode, Screen};
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

/// Test that the spinorama speaker search mode correctly:
/// 1. Marks itself as a text input mode (disabling keybindings)
/// 2. Updates speaker_search state when text is typed
/// 3. Exits search on Enter (via handle_enter action)
/// 4. Exits search on Escape (via Cancel action)
///
/// This is a regression test for a bug where letters bound to keybindings
/// (j, k, h, l, n, b, s, c, etc.) could not be typed in the search box.
pub struct SpinoramaSearchScenario;

impl TestScenario for SpinoramaSearchScenario {
    fn name(&self) -> &'static str {
        "Spinorama Speaker Search Input"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Navigate to Spinorama screen
        driver.navigate_to(Screen::Spinorama);

        // === Test 1: SpinoramaSpeakerSearch is a text input mode ===
        // This is the key invariant: when in search mode, key_context must be
        // "TextInput" to prevent single-letter keybindings from firing.
        if !InputMode::SpinoramaSpeakerSearch.is_text_input() {
            return Err(
                "SpinoramaSpeakerSearch must be a text input mode to block keybindings".into(),
            );
        }

        let mut page = driver.spinorama();

        // Enter search mode
        page.enter_search_mode();

        // Verify we're in search mode
        if !page.is_search_mode() {
            return Err("Should be in SpinoramaSpeakerSearch mode".into());
        }

        // === Test 2: Text input updates speaker_search state ===
        // The Input widget's on_text_change callback pushes text to speaker_search.
        // We simulate this directly since the test can't click/focus the Input widget.
        page.type_search_direct("kef");
        let query = page.get_search_query();
        if query != "kef" {
            return Err(format!("Expected 'kef', got '{}'", query).into());
        }

        // === Test 3: More text appends correctly ===
        page.type_search_direct(" r3");
        let query = page.get_search_query();
        if query != "kef r3" {
            return Err(format!("Expected 'kef r3', got '{}'", query).into());
        }

        // === Test 4: Enter exits search mode ===
        page.enter_search_mode();
        page.type_search_direct("genelec");
        // Simulate Enter keystroke - dispatches Enter action which exits SpinoramaSpeakerSearch
        driver.simulate_keystrokes("enter");
        driver.run_until_parked();
        let mode = driver.read_app(|app| app.ui_state.input_mode);
        if mode != InputMode::Normal {
            return Err(format!(
                "Enter should exit search mode, but mode is {:?}",
                mode
            )
            .into());
        }
        // Search query should be preserved after Enter
        let query = driver.read_app(|app| {
            app.measurement_state
                .spinorama_eq_state
                .speaker_search
                .clone()
        });
        if query != "genelec" {
            return Err(format!(
                "Search query should be preserved after Enter, got '{}'",
                query
            )
            .into());
        }

        // === Test 5: Escape exits search mode ===
        {
            let mut page = driver.spinorama();
            page.enter_search_mode();
            page.type_search_direct("jbl");
        }
        // Simulate Escape keystroke - dispatches Cancel action
        driver.simulate_keystrokes("escape");
        driver.run_until_parked();
        let mode = driver.read_app(|app| app.ui_state.input_mode);
        if mode != InputMode::Normal {
            return Err(format!(
                "Escape should exit search mode, but mode is {:?}",
                mode
            )
            .into());
        }

        Ok(())
    }
}

#[gpui::test]
async fn spinorama_speaker_search(cx: &mut TestAppContext) {
    let scenario = SpinoramaSearchScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await.unwrap();
    assert!(
        result.passed,
        "Spinorama search test failed: {}",
        result.error_message.unwrap_or_default()
    );
}

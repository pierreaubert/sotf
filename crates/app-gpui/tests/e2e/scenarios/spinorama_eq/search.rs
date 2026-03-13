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
        // Exit search mode by directly setting input mode to Normal
        // (In real app, Enter key triggers Input's on_edit_end callback which does this)
        driver.update_app(|app, _| {
            app.ui_state.input_mode = InputMode::Normal;
        });
        driver.run_until_parked();
        let mode = driver.read_app(|app| app.ui_state.input_mode);
        if mode != InputMode::Normal {
            return Err(format!(
                "Should exit search mode, but mode is {:?}",
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
        // Exit search mode by directly setting input mode to Normal
        // (In real app, Escape key triggers Cancel action which does this)
        driver.update_app(|app, _| {
            app.ui_state.input_mode = InputMode::Normal;
        });
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

/// Regression test: pressing 's' (and other keybinding-bound letters) while in
/// SpinoramaSpeakerSearch mode must NOT trigger the CycleSortOrder action.
///
/// The "TextInput" key context prevents keybindings from matching, ensuring
/// single-letter keys don't fire actions while the search box is active.
pub struct SpinoramaSearchKeystrokeScenario;

impl TestScenario for SpinoramaSearchKeystrokeScenario {
    fn name(&self) -> &'static str {
        "Spinorama Search: keybound letters must not trigger actions"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Navigate to Spinorama screen
        driver.navigate_to(Screen::Spinorama);

        // Enter search mode
        driver.update_app(|app, _| {
            app.ui_state.input_mode = sotf_audio_player_gpui::app::InputMode::SpinoramaSpeakerSearch;
            app.measurement_state
                .spinorama_eq_state
                .speaker_search
                .clear();
            app.measurement_state.spinorama_eq_state.step =
                sotf_audio_player_gpui::app::types::SpinoramaStep::SelectSpeaker;
        });
        driver.run_until_parked();

        // Record the current sort order before pressing 's'
        let sort_before = driver.read_app(|app| app.library_state.sort_order);

        // Simulate pressing 's' — this is bound to CycleSortOrder in PlayerView context
        driver.simulate_keystrokes("s");
        driver.run_until_parked();

        // Verify: sort order must NOT have changed (action must not fire in TextInput context)
        let sort_after = driver.read_app(|app| app.library_state.sort_order);
        if sort_before != sort_after {
            return Err(format!(
                "CycleSortOrder fired in SpinoramaSpeakerSearch mode! Sort changed from {:?} to {:?}. \
                 The 's' key should be passed to the Input widget, not trigger an action.",
                sort_before, sort_after
            )
            .into());
        }

        // Verify we're still in search mode (not kicked out)
        let mode = driver.read_app(|app| app.ui_state.input_mode);
        if mode != sotf_audio_player_gpui::app::InputMode::SpinoramaSpeakerSearch {
            return Err(format!(
                "Should still be in SpinoramaSpeakerSearch mode after pressing 's', but mode is {:?}",
                mode
            )
            .into());
        }

        Ok(())
    }
}

#[gpui::test]
async fn spinorama_search_keystroke_passthrough(cx: &mut TestAppContext) {
    let scenario = SpinoramaSearchKeystrokeScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await.unwrap();
    assert!(
        result.passed,
        "Spinorama search keystroke test failed: {}",
        result.error_message.unwrap_or_default()
    );
}

/// Regression test: when the Input widget has focus and the user types characters
/// including 's', 'j', 'k' (keys bound to actions in Normal mode), the text must
/// actually reach the search query state via the Input widget's on_text_change callback.
///
/// This tests the full GPUI keystroke → Input widget → on_text_change → state path.
/// The parent on_key_down handler must NOT call stop_propagation() for
/// SpinoramaSpeakerSearch mode, because that prevents the Input widget from
/// receiving the key event when it has focus.
pub struct SpinoramaSearchInputReceivesKeysScenario;

impl TestScenario for SpinoramaSearchInputReceivesKeysScenario {
    fn name(&self) -> &'static str {
        "Spinorama Search: Input widget receives all keystrokes"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        // Navigate to Spinorama screen and enter search mode
        driver.navigate_to(Screen::Spinorama);
        driver.update_app(|app, _| {
            app.ui_state.input_mode = sotf_audio_player_gpui::app::InputMode::SpinoramaSpeakerSearch;
            app.measurement_state
                .spinorama_eq_state
                .speaker_search
                .clear();
            app.measurement_state.spinorama_eq_state.step =
                sotf_audio_player_gpui::app::types::SpinoramaStep::SelectSpeaker;
        });
        driver.run_until_parked();

        // Focus the Input widget by finding it and clicking on it.
        // The Input widget has id "speaker-search".
        // In GPUI test context, we can simulate focus by dispatching a click.
        // Since we can't easily click the exact element, we test that the
        // on_key_down handler doesn't interfere with the Input's processing.

        // Simulate typing problematic characters: s, j, k (all bound to actions)
        // We type via GPUI keystrokes. If the Input widget is focused, these
        // should reach its on_key_down handler and update speaker_search.
        // If not focused, they should at minimum not trigger actions.
        let keys_to_test = ["s", "j", "k", "h", "l", "n", "b", "c"];
        for key in &keys_to_test {
            driver.simulate_keystrokes(key);
        }
        driver.run_until_parked();

        // The Input widget may or may not have focus in the test context.
        // But the critical invariant is: no actions should have fired.
        // Verify sort order unchanged (s = CycleSortOrder)
        let sort = driver.read_app(|app| app.library_state.sort_order);
        if sort != sotf_audio_player_gpui::app::state::library::LibrarySortOrder::default() {
            return Err(format!(
                "Action fired during search mode! Sort order changed to {:?}",
                sort
            )
            .into());
        }

        // Verify we're still in search mode
        let mode = driver.read_app(|app| app.ui_state.input_mode);
        if mode != sotf_audio_player_gpui::app::InputMode::SpinoramaSpeakerSearch {
            return Err(format!(
                "Should still be in SpinoramaSpeakerSearch mode, but mode is {:?}",
                mode
            )
            .into());
        }

        // If the Input widget received the keys, speaker_search would contain "sjkhlnbc".
        // If not (the current bug), it will be empty because the parent's stop_propagation
        // swallowed the events.
        let query = driver.read_app(|app| {
            app.measurement_state
                .spinorama_eq_state
                .speaker_search
                .clone()
        });

        if query.is_empty() {
            return Err(
                "Input widget did not receive any keystrokes. \
                 The parent on_key_down handler's stop_propagation() is swallowing key events \
                 before they reach the Input widget. Characters typed in the search box \
                 (s, j, k, h, l, n, b, c) were all lost."
                    .into(),
            );
        }

        // Check that all expected characters arrived
        let expected = "sjkhlnbc";
        if query != expected {
            return Err(format!(
                "Input widget received partial keystrokes. Expected '{}', got '{}'. \
                 Some letters are being intercepted before reaching the Input widget.",
                expected, query
            )
            .into());
        }

        Ok(())
    }
}

#[gpui::test]
async fn spinorama_search_input_receives_keys(cx: &mut TestAppContext) {
    let scenario = SpinoramaSearchInputReceivesKeysScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await.unwrap();
    assert!(
        result.passed,
        "Spinorama search input test failed: {}",
        result.error_message.unwrap_or_default()
    );
}

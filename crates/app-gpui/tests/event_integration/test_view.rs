//! Minimal test view that mimics the app's key handling behavior.
//!
//! This view captures the essential input handling patterns from PlayerView
//! without requiring the full 300+ field App struct.

use gpui::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// =============================================================================
// Actions (minimal subset for testing)
// =============================================================================

actions!(
    test_player,
    [
        TestPlayPause,
        TestVolumeUp,
        TestVolumeDown,
        TestToggleSearch,
        TestNextTrack,
        TestPrevTrack,
        TestSetFilterAll,
        TestSetFilter1,
        TestSetFilter2,
        TestSetFilter3,
        TestSetFilter4,
        TestSetFilter5,
    ]
);

// =============================================================================
// Test State
// =============================================================================

/// Input modes matching the real app
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestInputMode {
    Normal,
    Search,
    TextEntry,
}

/// Tracked state for test verification
#[derive(Debug, Default)]
pub struct TrackedState {
    /// Actions that were triggered
    pub actions: Vec<String>,
    /// Current input mode
    pub input_mode: TestInputMode,
    /// Search query content
    pub search_query: String,
    /// Simulated volume level
    pub volume: f32,
    /// Whether playback is active
    pub is_playing: bool,
    /// Current filter setting
    pub filter_rating: Option<u8>,
}

impl Default for TestInputMode {
    fn default() -> Self {
        TestInputMode::Normal
    }
}

impl TrackedState {
    pub fn new() -> Self {
        Self {
            volume: 1.0,
            ..Default::default()
        }
    }

    pub fn record_action(&mut self, action: &str) {
        self.actions.push(action.to_string());
    }

    pub fn clear_actions(&mut self) {
        self.actions.clear();
    }
}

/// Shared state wrapper for test view
pub type SharedTestState = Rc<RefCell<TrackedState>>;

// =============================================================================
// Test View
// =============================================================================

/// Minimal test view that implements the same input handling patterns as PlayerView
pub struct InputTestView {
    pub state: SharedTestState,
    focus_handle: FocusHandle,
    render_count: Arc<AtomicUsize>,
}

impl InputTestView {
    pub fn new(state: SharedTestState, cx: &mut Context<Self>) -> Self {
        Self {
            state,
            focus_handle: cx.focus_handle(),
            render_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Check if current mode is a text input mode
    fn is_text_input_mode(&self) -> bool {
        let state = self.state.borrow();
        matches!(state.input_mode, TestInputMode::Search | TestInputMode::TextEntry)
    }

    // =========================================================================
    // Action Handlers
    // =========================================================================

    fn toggle_playback(&mut self, _: &TestPlayPause, _window: &mut Window, _cx: &mut Context<Self>) {
        let mut state = self.state.borrow_mut();
        state.record_action("PlayPause");
        state.is_playing = !state.is_playing;
    }

    fn volume_up(&mut self, _: &TestVolumeUp, _window: &mut Window, _cx: &mut Context<Self>) {
        let mut state = self.state.borrow_mut();
        state.record_action("VolumeUp");
        state.volume = (state.volume + 0.1).min(1.0);
    }

    fn volume_down(&mut self, _: &TestVolumeDown, _window: &mut Window, _cx: &mut Context<Self>) {
        let mut state = self.state.borrow_mut();
        state.record_action("VolumeDown");
        state.volume = (state.volume - 0.1).max(0.0);
    }

    fn toggle_search(&mut self, _: &TestToggleSearch, _window: &mut Window, _cx: &mut Context<Self>) {
        let mut state = self.state.borrow_mut();
        state.record_action("ToggleSearch");
        state.input_mode = match state.input_mode {
            TestInputMode::Normal => TestInputMode::Search,
            TestInputMode::Search => TestInputMode::Normal,
            TestInputMode::TextEntry => TestInputMode::TextEntry,
        };
    }

    fn next_track(&mut self, _: &TestNextTrack, _window: &mut Window, _cx: &mut Context<Self>) {
        self.state.borrow_mut().record_action("NextTrack");
    }

    fn prev_track(&mut self, _: &TestPrevTrack, _window: &mut Window, _cx: &mut Context<Self>) {
        self.state.borrow_mut().record_action("PrevTrack");
    }

    fn set_filter_all(&mut self, _: &TestSetFilterAll, _window: &mut Window, _cx: &mut Context<Self>) {
        let mut state = self.state.borrow_mut();
        state.record_action("SetFilterAll");
        state.filter_rating = None;
    }

    fn set_filter_1(&mut self, _: &TestSetFilter1, _window: &mut Window, _cx: &mut Context<Self>) {
        let mut state = self.state.borrow_mut();
        state.record_action("SetFilter1");
        state.filter_rating = Some(1);
    }

    fn set_filter_2(&mut self, _: &TestSetFilter2, _window: &mut Window, _cx: &mut Context<Self>) {
        let mut state = self.state.borrow_mut();
        state.record_action("SetFilter2");
        state.filter_rating = Some(2);
    }

    fn set_filter_3(&mut self, _: &TestSetFilter3, _window: &mut Window, _cx: &mut Context<Self>) {
        let mut state = self.state.borrow_mut();
        state.record_action("SetFilter3");
        state.filter_rating = Some(3);
    }

    fn set_filter_4(&mut self, _: &TestSetFilter4, _window: &mut Window, _cx: &mut Context<Self>) {
        let mut state = self.state.borrow_mut();
        state.record_action("SetFilter4");
        state.filter_rating = Some(4);
    }

    fn set_filter_5(&mut self, _: &TestSetFilter5, _window: &mut Window, _cx: &mut Context<Self>) {
        let mut state = self.state.borrow_mut();
        state.record_action("SetFilter5");
        state.filter_rating = Some(5);
    }

    // =========================================================================
    // Key Event Handler (mimics PlayerView behavior)
    // =========================================================================

    fn handle_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // In text input mode, handle keys directly (don't let keybindings fire)
        if self.is_text_input_mode() {
            self.handle_text_input(event, window, cx);
        }
        // In normal mode, keybindings are handled by GPUI action system
    }

    fn handle_text_input(&mut self, event: &KeyDownEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        let mut state = self.state.borrow_mut();

        // Handle escape to exit text mode
        if event.keystroke.key == "escape" {
            state.input_mode = TestInputMode::Normal;
            state.search_query.clear();
            return;
        }

        // Handle backspace
        if event.keystroke.key == "backspace" {
            state.search_query.pop();
            return;
        }

        // Add character to search query (for printable characters)
        if let Some(ime_key) = &event.keystroke.ime_key {
            state.search_query.push_str(ime_key);
        } else if event.keystroke.key.len() == 1 {
            // Single character key
            state.search_query.push_str(&event.keystroke.key);
        }
    }
}

impl Render for InputTestView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_count.fetch_add(1, Ordering::SeqCst);

        // Determine key context based on input mode
        let key_context = if self.is_text_input_mode() {
            "TextInput"
        } else {
            "TestPlayer"
        };

        div()
            .id("test-view-root")
            .key_context(key_context)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key_down))
            // Register action handlers (only active in TestPlayer context)
            .on_action(cx.listener(Self::toggle_playback))
            .on_action(cx.listener(Self::volume_up))
            .on_action(cx.listener(Self::volume_down))
            .on_action(cx.listener(Self::toggle_search))
            .on_action(cx.listener(Self::next_track))
            .on_action(cx.listener(Self::prev_track))
            .on_action(cx.listener(Self::set_filter_all))
            .on_action(cx.listener(Self::set_filter_1))
            .on_action(cx.listener(Self::set_filter_2))
            .on_action(cx.listener(Self::set_filter_3))
            .on_action(cx.listener(Self::set_filter_4))
            .on_action(cx.listener(Self::set_filter_5))
            .size_full()
            .child(format!("Mode: {:?}", self.state.borrow().input_mode))
    }
}

// =============================================================================
// Event Simulator
// =============================================================================

/// High-level simulator for injecting events and verifying state
pub struct EventSimulator<'a> {
    cx: &'a mut VisualTestContext,
    state: SharedTestState,
}

impl<'a> EventSimulator<'a> {
    pub fn new(cx: &'a mut VisualTestContext, state: SharedTestState) -> Self {
        Self { cx, state }
    }

    /// Simulate a keystroke and run until parked
    pub fn keystroke(&mut self, key: &str) {
        self.cx.simulate_keystrokes(key);
        self.cx.run_until_parked();
    }

    /// Simulate multiple keystrokes
    pub fn keystrokes(&mut self, keys: &[&str]) {
        for key in keys {
            self.keystroke(key);
        }
    }

    /// Simulate typing text (character by character)
    pub fn type_text(&mut self, text: &str) {
        for c in text.chars() {
            self.cx.simulate_keystrokes(&c.to_string());
            self.cx.run_until_parked();
        }
    }

    /// Get the current state
    pub fn state(&self) -> std::cell::Ref<'_, TrackedState> {
        self.state.borrow()
    }

    /// Get mutable state
    pub fn state_mut(&self) -> std::cell::RefMut<'_, TrackedState> {
        self.state.borrow_mut()
    }

    /// Check if an action was triggered
    pub fn action_triggered(&self, action: &str) -> bool {
        self.state.borrow().actions.contains(&action.to_string())
    }

    /// Count how many times an action was triggered
    pub fn action_count(&self, action: &str) -> usize {
        self.state.borrow().actions.iter().filter(|a| *a == action).count()
    }

    /// Clear recorded actions
    pub fn clear_actions(&self) {
        self.state.borrow_mut().clear_actions();
    }

    /// Get all triggered actions
    pub fn actions(&self) -> Vec<String> {
        self.state.borrow().actions.clone()
    }

    /// Set input mode directly (for test setup)
    pub fn set_input_mode(&self, mode: TestInputMode) {
        self.state.borrow_mut().input_mode = mode;
    }

    /// Get current input mode
    pub fn input_mode(&self) -> TestInputMode {
        self.state.borrow().input_mode
    }

    /// Get search query
    pub fn search_query(&self) -> String {
        self.state.borrow().search_query.clone()
    }

    /// Get volume
    pub fn volume(&self) -> f32 {
        self.state.borrow().volume
    }

    /// Check if playing
    pub fn is_playing(&self) -> bool {
        self.state.borrow().is_playing
    }
}

// =============================================================================
// Test Helpers
// =============================================================================

/// Create a test window with the InputTestView
pub fn create_test_window(cx: &mut TestAppContext) -> (Entity<InputTestView>, SharedTestState) {
    let state = Rc::new(RefCell::new(TrackedState::new()));
    let state_clone = state.clone();

    let window = cx.add_window(move |_window, cx| InputTestView::new(state_clone, cx));

    (window, state)
}

/// Register keybindings for the test view
pub fn register_test_keybindings(cx: &mut TestAppContext) {
    cx.bind_keys([
        // TestPlayer context bindings (active in normal mode)
        KeyBinding::new("space", TestPlayPause, Some("TestPlayer")),
        KeyBinding::new("+", TestVolumeUp, Some("TestPlayer")),
        KeyBinding::new("=", TestVolumeUp, Some("TestPlayer")), // = without shift
        KeyBinding::new("-", TestVolumeDown, Some("TestPlayer")),
        KeyBinding::new("/", TestToggleSearch, Some("TestPlayer")),
        KeyBinding::new("n", TestNextTrack, Some("TestPlayer")),
        KeyBinding::new("p", TestPrevTrack, Some("TestPlayer")),
        KeyBinding::new("0", TestSetFilterAll, Some("TestPlayer")),
        KeyBinding::new("1", TestSetFilter1, Some("TestPlayer")),
        KeyBinding::new("2", TestSetFilter2, Some("TestPlayer")),
        KeyBinding::new("3", TestSetFilter3, Some("TestPlayer")),
        KeyBinding::new("4", TestSetFilter4, Some("TestPlayer")),
        KeyBinding::new("5", TestSetFilter5, Some("TestPlayer")),
    ]);
}

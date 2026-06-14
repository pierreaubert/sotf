/// Action category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum ActionCategory {
    #[default]
    Playback,
    Navigation,
    Library,
    Volume,
    Plugins,
    General,
}

/// Key modifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Modifier {
    Cmd,
    Shift,
    Alt,
    Ctrl,
}

/// Key binding
#[derive(Debug, Clone, Default)]
pub(super) struct KeyBinding {
    pub(super) action_id: String,
    pub(super) action_name: String,
    pub(super) category: ActionCategory,
    pub(super) key: String,
    pub(super) modifiers: Vec<Modifier>,
    pub(super) is_customized: bool,
    pub(super) is_default: bool,
}

/// Binding conflict
#[derive(Debug, Clone)]
pub(super) struct BindingConflict {
    pub(super) new_action: String,
    pub(super) existing_action: String,
    pub(super) key_combo: String,
}

/// Keybindings state
#[derive(Default)]
pub(super) struct KeybindingsState {
    pub(super) bindings: Vec<KeyBinding>,
    pub(super) filtered_bindings: Vec<KeyBinding>,
    pub(super) selected_category: Option<ActionCategory>,
    pub(super) search_query: String,
    pub(super) editing_binding: Option<String>, // action_id of binding being edited
    pub(super) pending_key: Option<(String, Vec<Modifier>)>,
    pub(super) conflict: Option<BindingConflict>,
    pub(super) show_customized_only: bool,
}

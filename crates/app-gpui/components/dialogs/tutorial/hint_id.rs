/// Unique identifiers for contextual hints shown once per feature encounter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HintId {
    /// First time opening the Studio screen
    StudioFirstVisit,
    /// First plugin added to the rack
    FirstPluginAdded,
    /// First time on the Room EQ screen
    RoomEqFirstVisit,
    /// Empty queue shown in library
    EmptyQueue,
}

impl HintId {
    pub fn as_str(&self) -> &'static str {
        match self {
            HintId::StudioFirstVisit => "studio_first_visit",
            HintId::FirstPluginAdded => "first_plugin_added",
            HintId::RoomEqFirstVisit => "roomeq_first_visit",
            HintId::EmptyQueue => "empty_queue",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            HintId::StudioFirstVisit => "Plugin Rack",
            HintId::FirstPluginAdded => "Plugin Added",
            HintId::RoomEqFirstVisit => "Room EQ",
            HintId::EmptyQueue => "Build Your Queue",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            HintId::StudioFirstVisit => {
                "Rack shortcuts: arrow keys select · Cmd/Ctrl+↑/↓ reorder · Enter toggles · Delete removes · +/- adjusts · Shift+1…0 quick-adds."
            }
            HintId::FirstPluginAdded => {
                "Click a plugin card to edit its parameters. Use = / - keys to adjust values."
            }
            HintId::RoomEqFirstVisit => {
                "Start by loading measurement data, then configure and run the optimizer."
            }
            HintId::EmptyQueue => "Click an album in the library to add it to your playback queue.",
        }
    }
}

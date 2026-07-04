use crate::app::Screen;

pub(super) fn get_keybindings_for_screen(screen: Screen) -> Vec<(&'static str, &'static str)> {
    match screen {
        Screen::Home | Screen::HomeShelf => vec![
            ("/", "Search albums"),
            ("Enter", "Open album"),
            ("Space", "Play/Pause"),
        ],
        Screen::NowPlaying => vec![
            ("Space", "Play/Pause"),
            ("N", "Next track"),
            ("P", "Previous track"),
            ("J/K", "Move through queue"),
        ],
        Screen::Library => vec![
            ("↑/↓ or K/J", "Navigate albums/artists"),
            ("PageUp/PageDown", "Jump by page"),
            ("/", "Search albums"),
            ("T", "Toggle tree view / flat view"),
            ("H/L or ←/→", "Collapse/expand artists in tree view"),
            ("S or 1/2/3/4", "Sort by Artist/Album/Title/Year"),
            ("C or 5/6/7/8/9", "Filter: All/Mono/Stereo/Multi/Mixed"),
            ("A or Enter", "Add album to queue"),
            ("Shift-Q", "Go to queue screen"),
        ],
        Screen::Streams => vec![
            ("Tab", "Move between stream fields"),
            ("Enter", "Play stream"),
            ("Space", "Play/Pause"),
        ],
        Screen::Queue => vec![
            ("↑/↓ or K/J", "Navigate queue items"),
            ("Enter", "Play selected album from start"),
            ("H/L or ←/→", "Expand/collapse album tracks"),
            ("Space", "Play/Pause"),
            ("N or >", "Next track"),
            ("B or <", "Previous track"),
            ("D/Delete", "Remove from queue"),
            ("C", "Clear entire queue"),
        ],
        Screen::Spectrum => vec![("Space", "Play/Pause"), ("N", "Next track")],
        Screen::Settings => vec![("T", "Cycle theme"), ("Alt-L", "Cycle language")],
        Screen::SettingsDetail => vec![("Esc", "Back to settings")],
        Screen::StudioHub => vec![
            ("Enter", "Open selected studio tool"),
            ("Space", "Play/Pause"),
        ],
        Screen::EqCurve => vec![("Esc", "Back to Studio"), ("Enter", "Edit selected EQ")],
        Screen::Studio => vec![
            ("E/U/G/L/O/B", "Add plugins"),
            ("Enter/e", "Edit plugin"),
            ("D/Delete", "Delete plugin"),
            ("Space", "Toggle on/off"),
            ("Shift-U/N", "Move up/down"),
            ("Shift-S/l", "Save/Load preset"),
        ],
        Screen::Recording => vec![
            ("Back/Close", "Navigate between steps"),
            ("Next/Finish", "Proceed to next step or finish"),
        ],
        Screen::RoomEq => vec![
            ("Back/Close", "Navigate between steps"),
            ("Next/Finish", "Proceed to next step or finish"),
        ],
        Screen::HeadphoneEq => vec![
            ("Back/Close", "Navigate between steps"),
            ("Next/Finish", "Proceed to next step or finish"),
        ],
        Screen::Spinorama => vec![
            ("Back/Close", "Navigate between steps"),
            ("Next/Finish", "Proceed to next step or finish"),
        ],
        Screen::PluginGraph => vec![
            ("Click+Drag", "Move nodes"),
            ("Drag port", "Create connection"),
            ("Delete", "Remove selected"),
            ("Space", "Toggle selected plugin"),
        ],
        Screen::Playlists => vec![
            ("↑/↓ or K/J", "Navigate playlists"),
            ("Enter", "Open playlist"),
            ("D/Delete", "Remove playlist"),
        ],
    }
}

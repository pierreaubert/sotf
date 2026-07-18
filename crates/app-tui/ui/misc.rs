pub(crate) use crate::app::Screen;
pub(crate) use ratatui::{Frame, layout::Rect};

/// Returns the screen area below the title bar (rows 3+), used for modals.
pub(crate) fn below_title_bar(f: &Frame) -> Rect {
    let area = f.area();
    let title_height = 3u16;
    Rect {
        x: area.x,
        y: area.y + title_height,
        width: area.width,
        height: area.height.saturating_sub(title_height),
    }
}

pub(crate) const DUAL_VIEW_HEIGHT_THRESHOLD: u16 = 40;

pub(crate) fn centered_modal_rect(
    area: Rect,
    width_percent: u16,
    height_percent: u16,
    min_width: u16,
    min_height: u16,
) -> Rect {
    let percent_width = area.width.saturating_mul(width_percent.min(100)) / 100;
    let percent_height = area.height.saturating_mul(height_percent.min(100)) / 100;
    let width = percent_width.max(min_width.min(area.width)).min(area.width);
    let height = percent_height
        .max(min_height.min(area.height))
        .min(area.height);

    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

pub(super) fn get_detailed_keybindings_for_screen(
    screen: Screen,
    language: crate::i18n::Language,
) -> Vec<(&'static str, &'static str)> {
    let bindings = match screen {
        Screen::Loading => vec![],
        Screen::Library => vec![
            ("↑/↓ or k/j", "Navigate albums/artists"),
            ("PageUp/PageDown", "Jump by page"),
            ("/", "Search albums"),
            ("t", "Toggle tree view / flat view"),
            ("h/l or ←/→", "Collapse/expand artists in tree view"),
            ("s or 1/2/3/4", "Sort by Artist/Album/Title/Year"),
            ("c or 5/6/7/8/9", "Filter: All/Mono/Stereo/Multi/Mixed"),
            ("a or Enter", "Add album to queue"),
            ("q", "Go to queue screen"),
        ],
        Screen::Configure => vec![
            ("1", "Directories sub-screen"),
            ("2", "Recording sub-screen"),
            ("3", "Room EQ sub-screen"),
            ("4", "Headphone EQ sub-screen"),
            ("5", "Spinorama EQ sub-screen"),
            ("6", "Federation Sources sub-screen"),
            ("7", "Servers sub-screen"),
            ("8", "Metadata Services sub-screen"),
            ("", ""),
            ("DIRECTORIES:", "(when on Directories sub-screen)"),
            ("↑/↓ or k/j", "Navigate directories"),
            ("a", "Add directory"),
            ("d/Delete", "Remove selected directory"),
            ("s", "Scan library (incremental)"),
            ("R", "Force rescan ALL files (preserves ReplayGain)"),
            ("m", "Database maintenance (clean missing files)"),
            ("r", "Analyze ReplayGain for all tracks"),
        ],
        Screen::Queue => vec![
            ("↑/↓ or k/j", "Navigate queue items"),
            ("Enter", "Play selected album from start"),
            ("h/l or ←/→", "Expand/collapse album tracks"),
            ("p", "Play/resume from current position"),
            ("Space", "Pause/resume"),
            ("n or >", "Next track"),
            ("b or <", "Previous track"),
            ("d/Delete", "Remove from queue"),
            ("c", "Clear entire queue"),
            ("A", "Add album (or selected track) to active playlist"),
        ],
        Screen::Plugins => vec![
            ("↑/↓ or k/j", "Navigate plugin chain"),
            ("a", "Add plugin (opens selection dialog)"),
            ("e or Enter", "Edit selected plugin"),
            ("t", "Toggle plugin enabled/disabled"),
            ("d/Delete", "Remove plugin"),
            ("u/U or Shift+↑", "Move plugin up in chain"),
            ("w/W or Shift+↓", "Move plugin down in chain"),
            ("s", "Save plugin chain to file"),
            ("l", "Load plugin chain from file"),
            ("", ""),
            ("ADD PLUGIN:", "(↑/↓ navigate, Enter select, Esc cancel)"),
            ("", ""),
            ("EDIT MODE:", "(when editing a plugin)"),
            ("↑/↓ or k/j", "Navigate parameters"),
            ("←/→ or h/l", "Adjust parameter value (small)"),
            ("[/]", "Adjust parameter value (large)"),
            ("a", "Load APO file (EQ plugins only)"),
            ("o", "Load SOFA file (Binaural only)"),
            ("ESC", "Exit edit mode"),
        ],
        Screen::Playlists => vec![
            ("↑/↓ or k/j", "Navigate playlists/tracks"),
            ("Enter or l", "Open playlist"),
            ("Esc or h", "Close playlist (back to list)"),
            ("n", "Create new playlist"),
            ("r", "Rename selected playlist"),
            ("d", "Delete selected playlist"),
            ("p", "Play all tracks"),
            ("x", "Remove track (in tracks view)"),
            ("K/J", "Move track up/down"),
            ("i", "Import M3U playlist"),
            ("e", "Export playlist to M3U"),
        ],
        Screen::Devices => vec![
            ("↑/↓ or k/j", "Navigate output devices"),
            ("Enter/Space", "Select output device"),
            ("r/R", "Rescan audio and cast devices"),
        ],
    };
    let text = crate::i18n::TuiTranslations::for_language(language);
    bindings
        .into_iter()
        .map(|(key, action)| (key, text.action_description(action)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{centered_modal_rect, get_detailed_keybindings_for_screen};
    use crate::app::Screen;
    use ratatui::layout::Rect;

    #[test]
    fn detailed_help_resolves_for_every_required_locale() {
        for language in crate::i18n::Language::ALL {
            for screen in [
                Screen::Loading,
                Screen::Library,
                Screen::Queue,
                Screen::Playlists,
                Screen::Plugins,
                Screen::Devices,
                Screen::Configure,
            ] {
                let _ = get_detailed_keybindings_for_screen(screen, language);
            }
        }
    }

    #[test]
    fn centered_modal_rect_clamps_to_tiny_terminal() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 8,
        };
        let rect = centered_modal_rect(area, 82, 82, 60, 20);
        assert!(rect.width <= area.width);
        assert!(rect.height <= area.height);
        assert_eq!(rect.x, 0);
        assert_eq!(rect.y, 0);
    }
}

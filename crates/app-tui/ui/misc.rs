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
    let text = crate::i18n::TuiTranslations::for_language(language);
    super::keybinding_catalog::keybindings_for_screen(screen)
        .iter()
        .map(|binding| (binding.key, text.action_description(binding.description)))
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

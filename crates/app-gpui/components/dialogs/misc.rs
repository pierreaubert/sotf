use crate::app::Screen;
use crate::app::i18n::{KeybindingTranslations, Language};
use crate::app::keybindings::{KeymapPreset, get_documented_keybindings_for_screen};

pub fn get_keybindings_for_screen(
    screen: Screen,
    language: Language,
    preset: KeymapPreset,
) -> Vec<(String, String)> {
    let text = KeybindingTranslations::for_language(language);
    get_documented_keybindings_for_screen(screen, preset)
        .into_iter()
        .map(|binding| {
            (
                binding.key,
                text.action_description(binding.description).to_string(),
            )
        })
        .collect()
}

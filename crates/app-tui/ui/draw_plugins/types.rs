use super::super::*;

/// Entry in the plugin parameter display list.
/// Can be a selectable parameter or a non-selectable section separator.
pub(super) enum ParamDisplayEntry {
    /// A selectable parameter with name and formatted value
    Param(String, String),
    /// A section separator line (not selectable)
    Separator(String),
}

/// Get the parameters for a plugin as display entries.
/// Returns a mix of selectable parameters and non-selectable separators.
/// Get the parameters for a plugin as display entries.
/// Returns a mix of selectable parameters and non-selectable separators.
pub(super) fn get_plugin_parameters(
    settings: &PluginSettings,
    _selected: usize,
) -> Vec<ParamDisplayEntry> {
    use crate::app::TuiEditablePlugin;
    use ParamDisplayEntry::{Param, Separator};

    let descriptors = settings.get_descriptors();
    let mut entries = Vec::with_capacity(descriptors.len() + 5);
    let mut last_group = String::new();

    for (i, desc) in descriptors.iter().enumerate() {
        // Add separator if group changes
        if !desc.group.is_empty() && desc.group != last_group {
            entries.push(Separator(desc.group.clone()));
            last_group = desc.group.clone();
        }

        let value = settings.get_value_as_string(i);
        let display_value = if desc.unit.is_empty() {
            value
        } else {
            format!("{} {}", value, desc.unit)
        };

        entries.push(Param(desc.name.clone(), display_value));
    }

    entries
}

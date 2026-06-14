use crate::app::{App, FilePickerMode, FilePickerOrigin};
use sotf_audio_player::PluginSettings;

/// Open the file explorer for a FilePath parameter identified by its engine key.
pub(super) fn open_file_path_param(app: &mut App, engine_key: &str) {
    match engine_key {
        "path_a_config" => {
            let start = sotf_audio_player::config::get_plugin_presets_dir()
                .map(|d| d.to_string_lossy().to_string());
            app.open_file_explorer(
                FilePickerOrigin::ABConfigA,
                FilePickerMode::File,
                "Select Path A Config (JSON)",
                start.as_deref(),
                Some("json"),
            );
        }
        "path_b_config" => {
            let start = sotf_audio_player::config::get_plugin_presets_dir()
                .map(|d| d.to_string_lossy().to_string());
            app.open_file_explorer(
                FilePickerOrigin::ABConfigB,
                FilePickerMode::File,
                "Select Path B Config (JSON)",
                start.as_deref(),
                Some("json"),
            );
        }
        "ir_file" => {
            if let Some(plugin) = app.plugin_graph.get_plugin(app.selected_plugin_index)
                && let PluginSettings::Convolution { ref ir_file, .. } = plugin.settings
            {
                let current_path = ir_file.clone();
                app.open_file_explorer(
                    FilePickerOrigin::IrFile,
                    FilePickerMode::File,
                    "Select Impulse Response (WAV)",
                    Some(&current_path),
                    Some("wav"),
                );
            }
        }
        "sofa_file" => {
            app.open_file_explorer(
                FilePickerOrigin::SofaFile,
                FilePickerMode::File,
                "Select SOFA File",
                Some(&app.sofa_file_input.clone()),
                Some("sofa"),
            );
        }
        _ => {}
    }
}

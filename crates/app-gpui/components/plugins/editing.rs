//! Plugin management and editing methods.
//!
//! Thin wrapper over `PluginController` from sotf-player. Each method delegates
//! to the controller and handles the returned `PluginUpdateEffect` by setting
//! `pending_plugin_update` on the GPUI-specific `PluginState`.

use sotf_plugins::{SpectralTiltCorrection, TiltReferenceFreq};

use crate::app::types::PluginUpdateType;
use crate::app::{App, ToastMessage};

pub trait PluginEditingManager {
    fn sync_spectrum_visible(&mut self);
    fn add_plugin(&mut self, plugin_type: &sotf_audio_player::PluginType);
    fn toggle_plugin(&mut self, index: usize);
    fn move_plugin_up(&mut self, index: usize);
    fn move_plugin_down(&mut self, index: usize);
    fn select_next_plugin(&mut self);
    fn select_previous_plugin(&mut self);
    fn remove_plugin(&mut self, index: usize);
    fn get_editing_plugin(&self) -> Option<&sotf_audio_player::Plugin>;
    fn get_editing_plugin_mut(&mut self) -> Option<&mut sotf_audio_player::Plugin>;
    fn select_next_param(&mut self);
    fn select_previous_param(&mut self);
    fn adjust_selected_param(&mut self, delta: f64) -> bool;

    // Additional methods
    fn set_plugin_param(&mut self, plugin_idx: usize, param_idx: usize, value: f64);
    fn set_plugin_param_string(&mut self, plugin_idx: usize, param_idx: usize, value: String);
    fn set_spectrum_tilt_correction(
        &mut self,
        plugin_idx: usize,
        correction: SpectralTiltCorrection,
    );
    fn set_spectrum_tilt_reference(&mut self, plugin_idx: usize, reference: TiltReferenceFreq);
    fn reset_plugin_param(&mut self, plugin_idx: usize, param_idx: usize);
    fn load_apo_file(&mut self) -> Result<(), String>;
    fn load_sofa_file(&mut self) -> Result<(), String>;
    fn add_eq_band(&mut self) -> Result<(), String>;
    fn remove_eq_band(&mut self, band_idx: usize) -> Result<(), String>;
    fn toggle_eq_band_mute(&mut self, band_idx: usize) -> Result<(), String>;
    fn toggle_eq_band_solo(&mut self, band_idx: usize) -> Result<(), String>;
    fn set_eq_per_channel_mode(&mut self, plugin_idx: usize, per_channel: bool);
    fn refresh_plugin_presets(&mut self);
    fn save_plugin_chain(&mut self);
    fn save_selected_preset(&mut self);
    fn load_plugin_chain(&mut self);
    fn load_selected_preset(&mut self);
    fn select_next_preset(&mut self);
    fn select_previous_preset(&mut self);
}

/// Convert a `PluginUpdateEffect` from the controller to GPUI's `PluginUpdateType`.
fn effect_to_update_type(
    effect: sotf_audio_player::PluginUpdateEffect,
) -> Option<PluginUpdateType> {
    match effect {
        sotf_audio_player::PluginUpdateEffect::None => None,
        sotf_audio_player::PluginUpdateEffect::Structural => Some(PluginUpdateType::Structural),
        sotf_audio_player::PluginUpdateEffect::Parameter {
            plugin_index,
            param_index,
        } => Some(PluginUpdateType::Parameter {
            plugin_index,
            param_index,
        }),
    }
}

impl PluginEditingManager for App {
    fn sync_spectrum_visible(&mut self) {
        self.spectrum_visible = self.plugin_state.has_enabled_spectrum_analyzer();
    }

    fn add_plugin(&mut self, plugin_type: &sotf_audio_player::PluginType) {
        let effect = self.plugin_state.add_plugin(plugin_type);
        self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
        self.sync_spectrum_visible();
    }

    fn toggle_plugin(&mut self, index: usize) {
        let effect = self.plugin_state.toggle_plugin(index);
        self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
        self.sync_spectrum_visible();
    }

    fn move_plugin_up(&mut self, index: usize) {
        let effect = self.plugin_state.move_plugin_up(index);
        self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
    }

    fn move_plugin_down(&mut self, index: usize) {
        let effect = self.plugin_state.move_plugin_down(index);
        self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
    }

    fn select_next_plugin(&mut self) {
        self.plugin_state.select_next_plugin();
    }

    fn select_previous_plugin(&mut self) {
        self.plugin_state.select_previous_plugin();
    }

    fn remove_plugin(&mut self, index: usize) {
        let effect = self.plugin_state.remove_plugin(index);
        self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
        self.sync_spectrum_visible();
    }

    fn get_editing_plugin(&self) -> Option<&sotf_audio_player::Plugin> {
        self.plugin_state.get_editing_plugin()
    }

    fn get_editing_plugin_mut(&mut self) -> Option<&mut sotf_audio_player::Plugin> {
        self.plugin_state.get_editing_plugin_mut()
    }

    fn select_next_param(&mut self) {
        self.plugin_state.select_next_param();
    }

    fn select_previous_param(&mut self) {
        self.plugin_state.select_previous_param();
    }

    fn adjust_selected_param(&mut self, delta: f64) -> bool {
        let (adjusted, effect) = self.plugin_state.adjust_selected_param(delta);
        if adjusted {
            self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
        }
        adjusted
    }

    fn set_plugin_param(&mut self, plugin_idx: usize, param_idx: usize, value: f64) {
        let effect = self
            .plugin_state
            .set_plugin_param(plugin_idx, param_idx, value);
        self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
    }

    fn set_plugin_param_string(&mut self, plugin_idx: usize, param_idx: usize, value: String) {
        let effect = self
            .plugin_state
            .set_plugin_param_string(plugin_idx, param_idx, value);
        self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
    }

    fn set_spectrum_tilt_correction(
        &mut self,
        plugin_idx: usize,
        correction: SpectralTiltCorrection,
    ) {
        let effect = self
            .plugin_state
            .set_spectrum_tilt_correction(plugin_idx, correction);
        self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
    }

    fn set_spectrum_tilt_reference(&mut self, plugin_idx: usize, reference: TiltReferenceFreq) {
        let effect = self
            .plugin_state
            .set_spectrum_tilt_reference(plugin_idx, reference);
        self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
    }

    fn reset_plugin_param(&mut self, plugin_idx: usize, param_idx: usize) {
        let effect = self.plugin_state.reset_plugin_param(plugin_idx, param_idx);
        self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
    }

    fn load_apo_file(&mut self) -> Result<(), String> {
        let path = std::path::Path::new(&self.input_state.apo_file_input);
        let effect = self.plugin_state.load_apo_filters(path)?;
        self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
        Ok(())
    }

    fn load_sofa_file(&mut self) -> Result<(), String> {
        let sofa_file_path = self.input_state.sofa_file_input.clone();
        let effect = self.plugin_state.load_sofa_path(sofa_file_path)?;
        self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
        Ok(())
    }

    fn add_eq_band(&mut self) -> Result<(), String> {
        let effect = self.plugin_state.add_eq_band()?;
        self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
        Ok(())
    }

    fn remove_eq_band(&mut self, band_idx: usize) -> Result<(), String> {
        let effect = self.plugin_state.remove_eq_band(band_idx)?;
        self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
        Ok(())
    }

    fn toggle_eq_band_mute(&mut self, band_idx: usize) -> Result<(), String> {
        let effect = self.plugin_state.toggle_eq_band_mute(band_idx)?;
        self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
        Ok(())
    }

    fn toggle_eq_band_solo(&mut self, band_idx: usize) -> Result<(), String> {
        let effect = self.plugin_state.toggle_eq_band_solo(band_idx)?;
        self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
        Ok(())
    }

    fn set_eq_per_channel_mode(&mut self, plugin_idx: usize, per_channel: bool) {
        let effect = self
            .plugin_state
            .set_eq_per_channel_mode(plugin_idx, per_channel);
        self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
    }

    fn refresh_plugin_presets(&mut self) {
        self.plugin_state.refresh_presets();
    }

    fn save_plugin_chain(&mut self) {
        if self.input_state.plugin_file_input.is_empty() {
            self.ui_state.toast_message =
                Some(ToastMessage::error("No filename specified".to_string()));
            return;
        }

        let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
            self.ui_state.toast_message = Some(ToastMessage::error(
                "Could not find presets directory".to_string(),
            ));
            return;
        };

        match self
            .plugin_state
            .save_to_file(&presets_dir, &self.input_state.plugin_file_input)
        {
            Ok(filename) => {
                self.ui_state.toast_message =
                    Some(ToastMessage::success(format!("Saved preset: {}", filename)));
            }
            Err(e) => {
                self.ui_state.toast_message = Some(ToastMessage::error(e.clone()));
                log::error!("Failed to save plugin chain: {}", e);
            }
        }
    }

    fn save_selected_preset(&mut self) {
        let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
            self.ui_state.toast_message = Some(ToastMessage::error(
                "Could not find presets directory".to_string(),
            ));
            return;
        };

        match self.plugin_state.save_selected_preset(&presets_dir) {
            Ok(filename) => {
                self.ui_state.toast_message = Some(ToastMessage::success(format!(
                    "Overwritten preset: {}",
                    filename
                )));
            }
            Err(e) => {
                self.ui_state.toast_message = Some(ToastMessage::error(e.clone()));
                log::error!("Failed to save plugin chain: {}", e);
            }
        }
    }

    fn load_plugin_chain(&mut self) {
        if self.input_state.plugin_file_input.is_empty() {
            self.ui_state.toast_message =
                Some(ToastMessage::error("No filename specified".to_string()));
            return;
        }

        let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
            self.ui_state.toast_message = Some(ToastMessage::error(
                "Could not find presets directory".to_string(),
            ));
            return;
        };

        match self
            .plugin_state
            .load_from_file(&presets_dir, &self.input_state.plugin_file_input)
        {
            Ok((effect, filename)) => {
                self.ui_state.toast_message = Some(ToastMessage::success(format!(
                    "Loaded preset: {}",
                    filename
                )));
                self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
                self.sync_spectrum_visible();
            }
            Err(e) => {
                self.ui_state.toast_message = Some(ToastMessage::error(e.clone()));
                log::error!("Failed to load plugin chain: {}", e);
            }
        }
    }

    fn load_selected_preset(&mut self) {
        let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
            self.ui_state.toast_message = Some(ToastMessage::error(
                "Could not find presets directory".to_string(),
            ));
            return;
        };

        match self.plugin_state.load_selected_preset(&presets_dir) {
            Ok((effect, filename, plugin_count)) => {
                self.ui_state.toast_message = Some(ToastMessage::success(format!(
                    "Loaded preset: {} ({} plugins)",
                    filename, plugin_count
                )));
                self.plugin_state.pending_plugin_update = effect_to_update_type(effect);
                self.sync_spectrum_visible();
            }
            Err(e) => {
                self.ui_state.toast_message = Some(ToastMessage::error(e.clone()));
                log::error!("Failed to load plugin chain: {}", e);
            }
        }
    }

    fn select_next_preset(&mut self) {
        self.plugin_state.select_next_preset();
    }

    fn select_previous_preset(&mut self) {
        self.plugin_state.select_previous_preset();
    }
}

// Re-export get_param_count from the controller for use by UI components
pub use sotf_audio_player::get_param_count;

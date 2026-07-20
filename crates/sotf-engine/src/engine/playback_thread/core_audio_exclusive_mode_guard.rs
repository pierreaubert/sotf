#[cfg(target_os = "macos")]
use super::coreaudio_mod::coreaudio_output_device_id;
use crate::{OutputAccessMode, OutputAccessStatus};

#[cfg(target_os = "macos")]
#[derive(Default)]
pub(super) struct CoreAudioExclusiveModeGuard {
    pub(super) device_id: Option<u32>,
    pub(super) device_name: String,
    pub(super) acquired_by_guard: bool,
}

#[cfg(target_os = "macos")]
impl CoreAudioExclusiveModeGuard {
    pub(super) fn inactive() -> Self {
        Self::default()
    }

    pub(super) fn activate_for_device(
        &mut self,
        device_name: &str,
        mode: OutputAccessMode,
    ) -> Result<OutputAccessStatus, String> {
        if !mode.prefers_exclusive() {
            self.release();
            return Ok(OutputAccessStatus::Shared);
        }

        let Some(device_id) = coreaudio_output_device_id(device_name) else {
            return self.unavailable_for_mode(
                device_name,
                mode,
                "CoreAudio device id could not be resolved".to_string(),
            );
        };

        let current_pid = std::process::id() as i32;
        if self.acquired_by_guard && self.device_id == Some(device_id) {
            match coreaudio::audio_unit::macos_helpers::get_hogging_pid(device_id) {
                Ok(owner) if owner == current_pid => {
                    return Ok(OutputAccessStatus::ExclusiveActive);
                }
                Ok(owner) => {
                    log::warn!(
                        "[Playback Thread] CoreAudio exclusive ownership for '{}' moved to pid {}; reacquiring",
                        self.device_name,
                        owner
                    );
                    self.device_id = None;
                    self.device_name.clear();
                    self.acquired_by_guard = false;
                }
                Err(e) => {
                    return self.unavailable_for_mode(
                        device_name,
                        mode,
                        format!("CoreAudio hog-mode owner query failed: {}", e),
                    );
                }
            }
        }

        self.release();

        let owner = match coreaudio::audio_unit::macos_helpers::get_hogging_pid(device_id) {
            Ok(owner) => owner,
            Err(e) => {
                return self.unavailable_for_mode(
                    device_name,
                    mode,
                    format!("CoreAudio hog-mode owner query failed: {}", e),
                );
            }
        };

        if owner == current_pid {
            self.device_id = Some(device_id);
            self.device_name = device_name.to_string();
            self.acquired_by_guard = false;
            return Ok(OutputAccessStatus::ExclusiveActive);
        }

        if owner != -1 {
            return self.unavailable_for_mode(
                device_name,
                mode,
                format!("device is already hogged by pid {}", owner),
            );
        }

        let new_owner = match coreaudio::audio_unit::macos_helpers::toggle_hog_mode(device_id) {
            Ok(owner) => owner,
            Err(e) => {
                return self.unavailable_for_mode(
                    device_name,
                    mode,
                    format!("CoreAudio hog-mode acquisition failed: {}", e),
                );
            }
        };

        if new_owner == current_pid {
            self.device_id = Some(device_id);
            self.device_name = device_name.to_string();
            self.acquired_by_guard = true;
            Ok(OutputAccessStatus::ExclusiveActive)
        } else {
            self.unavailable_for_mode(
                device_name,
                mode,
                format!("CoreAudio returned hog owner pid {}", new_owner),
            )
        }
    }

    pub(super) fn unavailable_for_mode(
        &mut self,
        device_name: &str,
        mode: OutputAccessMode,
        reason: String,
    ) -> Result<OutputAccessStatus, String> {
        self.release();
        if mode.requires_exclusive() {
            Err(format!(
                "Exclusive output is required, but CoreAudio exclusive mode could not be acquired for '{}': {}",
                device_name, reason
            ))
        } else {
            log::warn!(
                "[Playback Thread] CoreAudio exclusive output unavailable for '{}': {}; falling back to shared output",
                device_name,
                reason
            );
            Ok(OutputAccessStatus::FallbackShared)
        }
    }

    pub(super) fn release(&mut self) {
        if self.acquired_by_guard
            && let Some(device_id) = self.device_id
        {
            let current_pid = std::process::id() as i32;
            match coreaudio::audio_unit::macos_helpers::get_hogging_pid(device_id) {
                Ok(owner) if owner == current_pid => {
                    match coreaudio::audio_unit::macos_helpers::toggle_hog_mode(device_id) {
                        Ok(-1) => {
                            log::info!(
                                "[Playback Thread] Released CoreAudio exclusive mode for '{}'",
                                self.device_name
                            );
                        }
                        Ok(owner) => {
                            log::warn!(
                                "[Playback Thread] CoreAudio exclusive release for '{}' left owner pid {}",
                                self.device_name,
                                owner
                            );
                        }
                        Err(e) => {
                            log::warn!(
                                "[Playback Thread] Failed to release CoreAudio exclusive mode for '{}': {}",
                                self.device_name,
                                e
                            );
                        }
                    }
                }
                Ok(owner) => {
                    log::debug!(
                        "[Playback Thread] CoreAudio exclusive mode for '{}' is now owned by pid {}; not releasing",
                        self.device_name,
                        owner
                    );
                }
                Err(e) => {
                    log::warn!(
                        "[Playback Thread] Failed to query CoreAudio exclusive owner during release for '{}': {}",
                        self.device_name,
                        e
                    );
                }
            }
        }

        self.device_id = None;
        self.device_name.clear();
        self.acquired_by_guard = false;
    }
}

#[cfg(target_os = "macos")]
impl Drop for CoreAudioExclusiveModeGuard {
    fn drop(&mut self) {
        self.release();
    }
}

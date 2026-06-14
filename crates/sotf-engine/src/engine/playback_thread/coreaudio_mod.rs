#[cfg(target_os = "macos")]
pub(super) fn coreaudio_output_device_id(name: &str) -> Option<u32> {
    coreaudio::audio_unit::macos_helpers::get_device_id_from_name(name, false)
}

#[cfg(not(target_os = "macos"))]
pub(super) fn coreaudio_output_device_id(_name: &str) -> Option<u32> {
    None
}

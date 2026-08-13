use super::native_backend::NativeExternalPluginBackend;
use super::plugin_descriptor::PluginDescriptor;

#[cfg(feature = "external-plugin-clap")]
pub(super) fn load_clap_backend(
    descriptor: &PluginDescriptor,
    sample_rate: u32,
    max_block_frames: usize,
) -> Result<Box<dyn NativeExternalPluginBackend>, String> {
    Ok(Box::new(super::clap_backend::ClapBackend::load(
        descriptor,
        sample_rate,
        max_block_frames,
    )?))
}

#[cfg(not(feature = "external-plugin-clap"))]
pub(super) fn load_clap_backend(
    _descriptor: &PluginDescriptor,
    _sample_rate: u32,
    _max_block_frames: usize,
) -> Result<Box<dyn NativeExternalPluginBackend>, String> {
    Err("CLAP backend feature is disabled".to_string())
}

#[cfg(feature = "external-plugin-vst3")]
pub(super) fn load_vst3_backend(
    descriptor: &PluginDescriptor,
    sample_rate: u32,
    max_block_frames: usize,
) -> Result<Box<dyn NativeExternalPluginBackend>, String> {
    Ok(Box::new(super::vst3_backend::Vst3Backend::load(
        descriptor,
        sample_rate,
        max_block_frames,
    )?))
}

#[cfg(not(feature = "external-plugin-vst3"))]
pub(super) fn load_vst3_backend(
    _descriptor: &PluginDescriptor,
    _sample_rate: u32,
    _max_block_frames: usize,
) -> Result<Box<dyn NativeExternalPluginBackend>, String> {
    Err("VST3 backend feature is disabled".to_string())
}

#[cfg(all(feature = "external-plugin-au", target_os = "macos"))]
pub(super) fn load_audio_unit_backend(
    descriptor: &PluginDescriptor,
    sample_rate: u32,
    max_block_frames: usize,
) -> Result<Box<dyn NativeExternalPluginBackend>, String> {
    Ok(Box::new(super::au_backend::AudioUnitBackend::load(
        descriptor,
        sample_rate,
        max_block_frames,
    )?))
}

#[cfg(any(
    not(feature = "external-plugin-au"),
    all(feature = "external-plugin-au", not(target_os = "macos"))
))]
pub(super) fn load_audio_unit_backend(
    _descriptor: &PluginDescriptor,
    _sample_rate: u32,
    _max_block_frames: usize,
) -> Result<Box<dyn NativeExternalPluginBackend>, String> {
    Err("AudioUnit backend is available only on macOS builds with external-plugin-au".to_string())
}

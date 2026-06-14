pub(super) const SOTF_PLUGIN_FFI_ABI_VERSION: u32 = 3;

pub(super) const MAX_FFI_MIDI_EVENTS_PER_BLOCK: usize = 256;

pub(super) const MAX_FFI_OUTPUT_EVENTS_PER_BLOCK: usize = 256;

pub(super) const MAX_PRESET_JSON_IMPORT_BYTES: usize = 16 * 1024 * 1024;

pub(super) const MAX_PRESET_STATE_BYTES: usize = 4 * 1024 * 1024;

pub(super) const PRESET_UT_TYPE: &[u8] = b"org.spinorama.sotf.plugin-preset\0";

pub(super) const PRESET_FILE_EXTENSION: &[u8] = b"sotfpreset\0";

pub(super) const PRESET_MIME_TYPE: &[u8] = b"application/vnd.spinorama.sotf.plugin-preset+json\0";

pub(super) const VST3_COMPONENT_NAME: &[u8] = b"SOTF Plugin FFI Host\0";

pub(super) const VST3_VENDOR: &[u8] = b"Spinorama\0";

pub(super) const VST3_SDK_VERSION: &[u8] = b"VST 3.7 compatible C ABI\0";

pub(super) const VST3_ENTRYPOINT: &[u8] = b"plugin_create\0";

pub(super) const SWIFT_PACKAGE_NAME: &[u8] = b"SOTFPluginFFI\0";

pub(super) const SWIFT_PRODUCT_NAME: &[u8] = b"SOTFPluginFFI\0";

pub(super) const SWIFT_TARGET_NAME: &[u8] = b"SOTFPluginFFI\0";

pub(super) const SWIFT_LIBRARY_NAME: &[u8] = b"sotf_audio_plugins_ffi\0";

pub(super) const SWIFT_HEADER_NAME: &[u8] = b"SOTFPluginFFI.h\0";

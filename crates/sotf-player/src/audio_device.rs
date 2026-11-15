//! Audio device selection logic.
//!
//! Shared between all app frontends (GPUI, TUI, etc.)

use sotf_audio::devices::AudioDevice;

/// Core audio output device state: devices list, selected index, and current device name.
///
/// Frontends may embed this struct and add their own UI-specific fields
/// (HAL config, dropdown open states, input devices, etc.).
#[derive(Debug, Clone, Default)]
pub struct AudioOutputDeviceState {
    /// Available output devices
    pub devices: Vec<AudioDevice>,
    /// Currently selected output device index
    pub selected_index: usize,
    /// Name of the currently active output device
    pub current_device_name: Option<String>,
}

impl AudioOutputDeviceState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the currently selected output device.
    pub fn selected_device(&self) -> Option<&AudioDevice> {
        self.devices.get(self.selected_index)
    }

    /// Select output device by index. Returns true if selection changed.
    pub fn select(&mut self, index: usize) -> bool {
        if index < self.devices.len() && index != self.selected_index {
            self.selected_index = index;
            true
        } else {
            false
        }
    }

    /// Select next output device (wraps around).
    pub fn select_next(&mut self) {
        if !self.devices.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.devices.len();
        }
    }

    /// Select previous output device (wraps around).
    pub fn select_previous(&mut self) {
        if !self.devices.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.devices.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    /// Update the list of available output devices. Clamps selection to valid range.
    pub fn set_devices(&mut self, devices: Vec<AudioDevice>) {
        self.devices = devices;
        if self.selected_index >= self.devices.len() {
            self.selected_index = 0;
        }
    }

    /// Update output devices and select the best default (prefers non-virtual, non-HAL).
    pub fn set_devices_with_smart_default(&mut self, devices: Vec<AudioDevice>) {
        self.devices = devices;
        self.selected_index = self.find_best_default_index();
        if let Some(device) = self.devices.get(self.selected_index) {
            self.current_device_name = Some(device.name.clone());
        }
    }

    /// Get the max channels supported by the selected device.
    pub fn max_channels(&self) -> Option<usize> {
        self.selected_device()
            .and_then(|device| device.default_config.as_ref())
            .map(|config| config.channels as usize)
    }

    /// Get the current device sample rate, or 48 kHz as fallback.
    pub fn current_sample_rate(&self) -> f64 {
        self.selected_device()
            .and_then(|device| device.default_config.as_ref())
            .map(|config| config.sample_rate as f64)
            .unwrap_or(48000.0)
    }

    /// Find the best default output device index.
    /// Priority: system default non-virtual > any non-virtual > system default > first.
    pub fn find_best_default_index(&self) -> usize {
        // Prefer system default that's not virtual
        if let Some((idx, _)) = self
            .devices
            .iter()
            .enumerate()
            .find(|(_, d)| d.is_default && !is_virtual_device(&d.name))
        {
            return idx;
        }

        // Any non-virtual device
        if let Some((idx, _)) = self
            .devices
            .iter()
            .enumerate()
            .find(|(_, d)| !is_virtual_device(&d.name))
        {
            return idx;
        }

        // System default (even if virtual)
        self.devices
            .iter()
            .enumerate()
            .find(|(_, d)| d.is_default)
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    }
}

/// Check if a device name is a virtual/passthrough device that shouldn't be default.
/// Whether `name` is a virtual/null endpoint rather than a physical
/// output. Delegates to the shared endpoint classifier in
/// sotf-capture so player, engine and daemon agree — including Linux
/// names (monitor sources, PipeWire loopbacks, the ALSA sample
/// discarder). The old hand-rolled list also matched the bare
/// substring "hal", which false-positives on real hardware names
/// (e.g. "Wharfedale"); the SotF HAL device is covered by the "sotf"
/// and "virtual" markers instead.
pub fn is_virtual_device(name: &str) -> bool {
    !sotf_audio::devices::classify_device(name, None).is_physical_output_candidate()
        || sotf_audio::devices::is_null_device(name)
}

/// ALSA's fixed channel order, truncated by stream width. Measured
/// empirically through the ALSA->PipeWire path (oracle matrix in
/// sotf-capture's pipewire_oracle test): an N-channel stream gets
/// the first N of these positions, whatever the caller intended.
const ALSA_POSITION_ORDER: [&str; 8] = ["FL", "FR", "RL", "RR", "FC", "LFE", "SL", "SR"];

/// Standard stream widths whose ALSA position maps are well-defined.
const STANDARD_STREAM_WIDTHS: [u16; 4] = [2, 4, 6, 8];

/// Canonical speaker position for a speaker/group name, or `None`
/// when the name carries no recognizable position. Accepts the short
/// codes the speaker configs use plus common spellings.
fn speaker_position(name: &str) -> Option<&'static str> {
    let normalized: String = name
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect();
    // Strip a trailing instance number ("SL1", "SUB2") for matching.
    let stem = normalized.trim_end_matches(|ch: char| ch.is_ascii_digit());
    match stem {
        "L" | "FL" | "LEFT" | "FRONTLEFT" => Some("FL"),
        "R" | "FR" | "RIGHT" | "FRONTRIGHT" => Some("FR"),
        "C" | "FC" | "CENTER" | "CENTRE" | "FRONTCENTER" | "FRONTCENTRE" => Some("FC"),
        "LFE" | "SUB" | "SW" | "SUBWOOFER" => Some("LFE"),
        "SL" | "LS" | "LEFTSURROUND" | "SURROUNDLEFT" | "SIDELEFT" => Some("SL"),
        "SR" | "RS" | "RIGHTSURROUND" | "SURROUNDRIGHT" | "SIDERIGHT" => Some("SR"),
        "BL" | "RL" | "BACKLEFT" | "REARLEFT" => Some("RL"),
        "BR" | "RR" | "BACKRIGHT" | "REARRIGHT" => Some("RR"),
        _ => None,
    }
}

/// How to address one speaker on the measurement output stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeasurementRoute {
    /// Index to write the stimulus to in the opened stream.
    pub stream_index: u16,
    /// Width the stream must be opened at for that index to mean
    /// what the caller intends. `None` = backend default.
    pub stream_width: Option<u16>,
}

/// Compute the measurement output route for one speaker.
///
/// Physical devices keep the user's interface-channel patch verbatim.
/// Server/virtual sinks (PipeWire/Pulse through ALSA) reassign
/// channel *positions by stream width* in ALSA's fixed order, so a
/// raw interface index does not name a speaker there: translate the
/// speaker's position (from its name) into that order at the
/// smallest standard width containing every mapped speaker's
/// position. Unrecognized speaker names fall back to the physical
/// patch on every device class.
#[must_use]
pub fn measurement_output_route(
    speaker_name: &str,
    interface_channel: u16,
    all_speaker_names: &[String],
    total_interface_channels: u16,
    output_device_name: &str,
) -> MeasurementRoute {
    let physical = MeasurementRoute {
        stream_index: interface_channel,
        stream_width: Some(total_interface_channels.max(interface_channel + 1)),
    };
    let class = sotf_audio::devices::classify_device(output_device_name, None);
    if class == sotf_audio::devices::DeviceClass::Physical {
        return physical;
    }
    let Some(position) = speaker_position(speaker_name) else {
        return physical;
    };
    // The stream must be wide enough that *every* mapped speaker's
    // position exists in the ALSA map for that width, so all takes
    // in the session share one consistent width.
    let mut needed: Vec<&'static str> = all_speaker_names
        .iter()
        .filter_map(|name| speaker_position(name))
        .collect();
    if needed.is_empty() {
        needed.push(position);
    }
    let Some(width) = STANDARD_STREAM_WIDTHS.into_iter().find(|&width| {
        needed
            .iter()
            .all(|role| ALSA_POSITION_ORDER[..usize::from(width)].contains(role))
    }) else {
        return physical;
    };
    let Some(index) = ALSA_POSITION_ORDER[..usize::from(width)]
        .iter()
        .position(|&role| role == position)
    else {
        return physical;
    };
    MeasurementRoute {
        stream_index: u16::try_from(index).unwrap_or(interface_channel),
        stream_width: Some(width),
    }
}

/// Translate a whole speaker mapping set at once: `(group_name,
/// interface_channel)` pairs in, `(stream indices, shared stream
/// width)` out. All routes share one width by construction, so a
/// multi-speaker measurement session opens every take at the same
/// width. See [`measurement_output_route`].
#[must_use]
pub fn measurement_output_routes(
    mappings: &[(String, u16)],
    output_device_name: &str,
) -> (Vec<u16>, Option<u16>) {
    let total_interface_channels = mappings
        .iter()
        .map(|(_, channel)| *channel)
        .max()
        .map_or(2, |max| max + 1);
    let names: Vec<String> = mappings.iter().map(|(name, _)| name.clone()).collect();
    let routes: Vec<MeasurementRoute> = mappings
        .iter()
        .map(|(name, channel)| {
            measurement_output_route(
                name,
                *channel,
                &names,
                total_interface_channels,
                output_device_name,
            )
        })
        .collect();
    let width = routes.first().and_then(|route| route.stream_width);
    (
        routes.iter().map(|route| route.stream_index).collect(),
        width,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(name: &str, is_default: bool) -> AudioDevice {
        AudioDevice {
            device_id: Some(name.to_string()),
            name: name.to_string(),
            display_info: None,
            is_input: false,
            is_default,
            supported_configs: Vec::new(),
            default_config: None,
            available_sample_rates: Vec::new(),
        }
    }

    #[test]
    fn virtual_device_detector_matches_systemwide_virtual_output() {
        assert!(is_virtual_device("SotF Virtual Device"));
        assert!(is_virtual_device("SotF Virtual Output"));
        assert!(is_virtual_device("BlackHole 2ch"));
        assert!(is_virtual_device("Loopback Audio"));
        assert!(is_virtual_device("Generic Virtual Device"));
    }

    #[test]
    fn measurement_route_translates_roles_for_pipewire_sinks() {
        // The #258 setup: 5.1 speakers auto-numbered 0..5 into a
        // PipeWire sink. ALSA assigns positions [FL FR RL RR FC LFE
        // SL SR] truncated by width, so SL/SR force width 8 and the
        // indices below; anything narrower misroutes FC/LFE/SL.
        let names: Vec<String> = ["FL", "FR", "C", "LFE", "SL", "SR"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let route = |speaker: &str, iface: u16| {
            measurement_output_route(speaker, iface, &names, 6, "PipeWire Sound Server")
        };
        assert_eq!(
            route("FL", 0),
            MeasurementRoute {
                stream_index: 0,
                stream_width: Some(8)
            }
        );
        assert_eq!(route("C", 2).stream_index, 4);
        assert_eq!(route("LFE", 3).stream_index, 5);
        assert_eq!(route("SL", 4).stream_index, 6);
        assert_eq!(route("SR", 5).stream_index, 7);
        assert_eq!(route("SR", 5).stream_width, Some(8));
    }

    #[test]
    fn measurement_route_keeps_physical_patch_on_hardware() {
        // MOTU-style patch: speakers wired to interface channels 2..7.
        let names: Vec<String> = ["FL", "FR", "C", "LFE", "SL", "SR"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let route = measurement_output_route("SL", 6, &names, 8, "MOTU UltraLite mk5");
        assert_eq!(route.stream_index, 6);
        assert_eq!(route.stream_width, Some(8));
    }

    #[test]
    fn measurement_route_stereo_stays_stereo() {
        let names: Vec<String> = vec!["FL".to_string(), "FR".to_string()];
        let route = measurement_output_route("FR", 1, &names, 2, "PipeWire Sound Server");
        assert_eq!(
            route,
            MeasurementRoute {
                stream_index: 1,
                stream_width: Some(2)
            }
        );
    }

    #[test]
    fn smart_default_skips_virtual_system_default() {
        let mut state = AudioOutputDeviceState::new();
        state.set_devices(vec![
            device("SotF Virtual Device", true),
            device("Built-in Output", false),
        ]);

        assert_eq!(state.find_best_default_index(), 1);
    }
}

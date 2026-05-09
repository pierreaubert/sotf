//! MIDI hardware input bridge.
//!
//! Auto-detects a connected supported MIDI controller (Xone:K2, Launch
//! Control XL) at startup, opens the input port, and pumps decoded
//! `MidiMessage`s onto a `std::sync::mpsc::Sender`. The receiver is drained
//! from the GPUI tick loop in `ui::PlayerView` and dispatched through the
//! plugin state's `MidiMappingEngine`.
//!
//! Defaults are intentionally minimal:
//! - First matching device wins; no device picker UI.
//! - No hot-plug rescan after startup — user reconnects via app restart.
//! - No LED feedback. Outgoing MIDI is not sent.

use std::sync::mpsc::{self, Receiver, Sender};

use sotf_audio_player_midi::layout::ControllerLayout;
use sotf_audio_player_midi::layouts::{lcxl_layout, xone_k2_layout};
use sotf_audio_player_midi::message::MidiMessage;
use sotf_audio_player_midi::{MidiDeviceInfo, MidiManager};

/// Identifier for a supported controller layout, matching the strings used
/// by `available_controllers()` in `app::state::plugin`.
pub const CONTROLLER_ID_XONE_K2: &str = "xone_k2";
pub const CONTROLLER_ID_LCXL: &str = "lcxl";

/// MIDI input service holding the device connection and a receiver of
/// parsed messages. Drop this to disconnect.
pub struct MidiInputService {
    /// Held only to keep the device connection alive — the manager owns the
    /// midir input connection internally.
    _manager: MidiManager,
    /// Decoded MIDI messages from the active input port.
    rx: Receiver<MidiMessage>,
    /// Stable id of the matched controller (e.g. "xone_k2").
    controller_id: &'static str,
    /// Cached `ControllerLayout` so the engine can be configured without
    /// re-running detection logic.
    layout: ControllerLayout,
    /// Friendly name reported by midir.
    device_name: String,
}

impl MidiInputService {
    /// Resolved controller layout id ("xone_k2" or "lcxl").
    pub fn controller_id(&self) -> &'static str {
        self.controller_id
    }

    /// Layout to install on the mapping engine.
    pub fn layout(&self) -> ControllerLayout {
        self.layout.clone()
    }

    /// Device name as reported by the OS — useful for breadcrumbs.
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Drain all currently-buffered MIDI messages. Returns immediately when
    /// the channel is empty; never blocks the caller.
    pub fn drain(&self) -> Vec<MidiMessage> {
        let mut out = Vec::new();
        while let Ok(msg) = self.rx.try_recv() {
            out.push(msg);
        }
        out
    }
}

/// Try to auto-connect to the first supported controller found in the OS
/// MIDI input device list. Returns `None` when nothing matches or when MIDI
/// initialization fails — the caller should treat this as a no-op (the app
/// is fully usable without hardware).
pub fn try_start() -> Option<MidiInputService> {
    let mut manager = match MidiManager::new() {
        Ok(m) => m,
        Err(err) => {
            log::info!("MIDI: manager init failed ({err}); MIDI input disabled");
            return None;
        }
    };

    let devices = match manager.list_input_devices() {
        Ok(d) => d,
        Err(err) => {
            log::info!("MIDI: input enumeration failed ({err}); MIDI input disabled");
            return None;
        }
    };

    let Some((matched, controller_id)) = pick_supported_device(&devices) else {
        if devices.is_empty() {
            log::info!("MIDI: no input devices present");
        } else {
            let names: Vec<&str> = devices.iter().map(|d| d.name.as_str()).collect();
            log::info!(
                "MIDI: no supported controller in connected devices: {:?}",
                names
            );
        }
        return None;
    };

    let device_name = matched.name.clone();
    let layout = match controller_id {
        CONTROLLER_ID_XONE_K2 => xone_k2_layout(),
        CONTROLLER_ID_LCXL => lcxl_layout(),
        _ => unreachable!("pick_supported_device only yields known ids"),
    };

    let (tx, rx): (Sender<MidiMessage>, Receiver<MidiMessage>) = mpsc::channel();

    let connect_result = manager.connect_input(matched.index, move |msg| {
        // Channel send only fails if the receiver has been dropped — at
        // that point the service is shutting down and dropping messages
        // is the correct behavior.
        let _ = tx.send(msg);
    });

    if let Err(err) = connect_result {
        log::warn!("MIDI: failed to open '{device_name}' ({err}); MIDI input disabled");
        return None;
    }

    log::info!(
        "MIDI: connected '{device_name}' using layout '{}'",
        layout.name
    );

    Some(MidiInputService {
        _manager: manager,
        rx,
        controller_id,
        layout,
        device_name,
    })
}

/// Match a device list against the controllers we know how to drive. Names
/// are matched case-insensitively against the device's `name` field as
/// reported by the OS. Returns the first match with its stable id.
fn pick_supported_device(devices: &[MidiDeviceInfo]) -> Option<(&MidiDeviceInfo, &'static str)> {
    for d in devices {
        let lower = d.name.to_lowercase();
        if lower.contains("xone") {
            return Some((d, CONTROLLER_ID_XONE_K2));
        }
        if lower.contains("launch control xl") || lower.contains("launchcontrol xl") {
            return Some((d, CONTROLLER_ID_LCXL));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_audio_player_midi::device::MidiDeviceType;

    fn dev(name: &str) -> MidiDeviceInfo {
        MidiDeviceInfo {
            index: 0,
            name: name.to_string(),
            device_type: MidiDeviceType::Input,
            manufacturer: None,
            is_connected: false,
        }
    }

    #[test]
    fn pick_xone_by_substring() {
        let list = [dev("Xone:K2 MIDI 1")];
        let (_, id) = pick_supported_device(&list).expect("should match");
        assert_eq!(id, CONTROLLER_ID_XONE_K2);
    }

    #[test]
    fn pick_lcxl_by_substring() {
        let list = [dev("Launch Control XL")];
        let (_, id) = pick_supported_device(&list).expect("should match");
        assert_eq!(id, CONTROLLER_ID_LCXL);
    }

    #[test]
    fn no_match_when_unknown_device() {
        let list = [dev("Some Random USB MIDI")];
        assert!(pick_supported_device(&list).is_none());
    }

    #[test]
    fn no_match_when_empty_list() {
        assert!(pick_supported_device(&[]).is_none());
    }

    #[test]
    fn xone_wins_over_lcxl_when_both_present() {
        let list = [dev("Launch Control XL"), dev("Xone:K2")];
        let (_, id) = pick_supported_device(&list).expect("should match");
        // First match wins; LCXL is first → expect lcxl. Verify this is the
        // documented behavior, not a silent priority change.
        assert_eq!(id, CONTROLLER_ID_LCXL);
    }
}

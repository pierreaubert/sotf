# sotf-midi Release Notes

## Supported MIDI behavior for release

This crate provides host-independent MIDI device management and control for
SOTF audio players. The following behavior is supported and tested for release:

### Device discovery and hot-plug

- `MidiManager::list_input_devices` / `list_output_devices` enumerate available
  OS MIDI ports without requiring a connection.
- `MidiManager::device_snapshot` and `poll_device_changes` provide non-mutating
  hot-plug detection based on device name and direction (input vs. output).
- Reconnecting a device with the same name and direction is treated as the same
  logical device; index changes do not trigger false disconnect/connect events.
- Devices with identical names but opposite directions are kept distinct.

### Mapping persistence

- `MidiMapping`, `ControllerLayout`, `MappingTemplate`, `MidiDeviceInfo`, and
  `MidiDeviceSnapshot` all serialize to JSON via serde and round-trip exactly.
- `MidiConfig` save/load preserves profiles, device names, init messages,
  custom settings, active profile, default devices, and channel filter.
- Manual overrides, multi-page bindings, and per-binding value scaling survive
  persistence.

### Missing-device safety

- `MidiManager::with_config` constructs successfully even when the configured
  default input/output devices are not present.
- `connect_input_by_name` / `connect_output_by_name` return
  `MidiError::ConnectionError` for absent devices instead of panicking.
- A persisted mapping for an absent controller loads normally; the mapping
  engine simply reports `Unmapped` for incoming messages until a matching
  layout is provided.

### Invalid input events

- The MIDI parser rejects malformed bytes: empty messages, data-only bytes
  without running status, truncated channel messages, and data bytes with the
  high bit set.
- Parsed messages that do not correspond to any control in the active
  `ControllerLayout`, or that arrive on the wrong channel, produce
  `MappingAction::Unmapped` and do not modify plugin parameters.
- Out-of-range parameter indices in a mapping are handled safely and return
  `Unmapped`.

### Hardware profiles

Pre-configured control profiles are included for:

- RME UFX+ / TotalMix FX (faders, mutes, solos, pans, snapshots)
- Genelec GLM (volume, mute, dim, solo, monitor groups, presets)
- Allen & Heath Xone:K2/K3 (pots, encoders, faders, buttons)
- Novation Launch Control XL (knobs, faders, buttons, LED feedback via SysEx)

These profiles use standard MIDI CC and Note messages and work with the OS
MIDI drivers; no proprietary driver installation is required.

### Known limitations

- MIDI Clock/MTC parsing is supported, but the crate does not yet provide a
  transport-sync engine that slaves the sequencer to external clock.
- Live device arrival/departure notifications depend on the OS and the
  `midir` backend. Apps should poll for changes if real-time hot-plug updates
  are required.
- Two devices with the same name in the same direction are treated as a single
  logical device by the hot-plug differ.

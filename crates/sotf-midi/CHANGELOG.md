# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.4] - 2026-05-31

### Added
- Add stack-sized `MidiMessage::System` for MIDI system common and real-time messages.
- Add non-mutating MIDI device enumeration helpers.
- Add checked template-to-mapping conversion and allocation-free MIDI region iterators.

### Fixed
- Reject channel messages whose data bytes have the status bit set instead of masking malformed input.
- Reassemble split SysEx input packets before dispatching MIDI callbacks.
- Avoid duplicate control bindings when manual overrides or MIDI learn remap an existing control.
- Ignore stale templates whose `param_index` exceeds the focused plugin's parameter list.
- Identify Launch Control XL arrow buttons as CC controls.
- Validate RME TotalMix bank usage consistently and send Mackie mute/solo buttons as NoteOn/NoteOff press pairs.
- Make auto-map unit matching case-insensitive for frequency parameters and clarify zero-control paging.

## [0.1.3] - 2025-05-13

### Fixed
- Did a round of test fixing.

### Changed
- Large commit with significant improvements to the host: timeline & clip
  sequencing, multi-track recording, sample-accurate automation, latency
  compensation v2, MIDI sequencing, sidechain routing v3, non-destructive
  editing, offline bounce, VST/AU hosting. First step of automatic UI
  generation via a set of constraints; non-regression is built in with `insta`.

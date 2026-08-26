# Stereo Room EQ reference fixture

This deterministic fixture is owned by the SOTF dev-driver. It models a
calibrated two-channel, single-position measurement on a shared logarithmic
frequency grid from 20 Hz to 20 kHz. Values are magnitude in dB re 1.0 and
unwrapped phase in degrees.

It deliberately contains only the data consumed by the UI/optimizer contract:
there are no host-device, absolute-user-directory, or sibling-repository
dependencies. The suite runner copies it into each scenario's isolated
artifact directory before handing its path to the debug-only fixture adapter.

Fixture invariants:

- channels are `L` and `R`, ordered as a stereo playback layout;
- each response has the same 16-bin logarithmic grid, strictly ascending;
- calibrated reference is 94 dB SPL at 1 kHz;
- each magnitude and phase array has exactly the frequency-grid length.

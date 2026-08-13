# Loudness Compensation — UI Specification

The canonical generated layout is defined by `src/params.rs`.

- Main mode selector: Manual / ISO 226 / Auto.
- Manual: low shelf, optional mid peak, and high shelf controls.
- ISO 226: playback and reference SPL.
- Auto: engine playback volume (read-only), measured reference SPL, and the
  required **SPL Calibrated** confirmation.
- Level policy: **Headroom Normalized**. Off preserves the 1 kHz reference; on
  shows that broadband attenuation is part of the transfer function.
- Output: one AutoGain position selector (Disabled / Pre / Post), maximum
  correction, and smoothing. The legacy enabled boolean is not a separate UI
  state.

Auto mode must present a validation error until calibration is confirmed. Compact
layouts stack groups; wide layouts may place output controls in the right column.

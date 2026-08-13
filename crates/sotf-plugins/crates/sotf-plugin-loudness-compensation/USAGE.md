# Loudness Compensation

## Modes

- **Manual** uses two half-gain low shelves, an optional mid peak, and two
  half-gain high shelves. The requested shelf gain is the total asymptotic gain;
  it is not doubled by the cascade.
- **ISO 226** computes the requested equal-loudness delta at all 29 ISO 226:2003
  frequencies and jointly fits a 20-biquad bank. The bank is normalized around
  the standard's 1 kHz phon reference.
- **Auto** uses the ISO bank and derives playback SPL from a measured SPL at
  engine volume 0 dB plus `playback_volume_db`. It is rejected until
  `auto_calibrated` is enabled. Digital dBFS, LUFS, and a volume scalar alone are
  not acoustic calibration.

ISO 226 is a population-average free-field relationship. Headphone or room use
may require a separate transfer correction and listening validation.

## Level and AutoGain policies

`headroom_normalized=false` is the default. It preserves the requested 1 kHz
reference and may create positive peaks, so provide downstream headroom or a
limiter. When `headroom_normalized=true`, the plugin scans the realized active
cascade through Nyquist and applies broadband attenuation equal to its positive
peak. Cuts never consume headroom. This changes the absolute 1 kHz level and is
therefore a visible user choice, not an implicit safety correction.

AutoGain has one canonical three-state control: `disabled`, `pre`, or `post`.
The legacy `auto_gain_enabled` boolean remains accepted for old presets and maps
to `post`/`disabled`. AutoGain is an LUFS matching loop and is separate from SPL
calibration and ISO contour generation.

## Realtime contract

All filter design, ISO optimization, and full-band peak scans happen during
construction or control updates. `process_in_place` only processes prepared
state. Coefficient and mode changes crossfade old and new banks over 256 samples.
Processing and reset allocate no memory, return exactly `context.num_frames`, and
require an exact interleaved buffer length and matching initialized sample rate.
Frequencies are clamped to 45% of Nyquist-safe sample-rate space at 16–192 kHz.

## Example

```json
{
  "mode": 2,
  "reference_level_db": 78.0,
  "playback_volume_db": -18.0,
  "auto_calibrated": true,
  "headroom_normalized": false,
  "auto_gain_position": "disabled"
}
```

Here `reference_level_db` must be the measured listener-position SPL produced by
the actual playback chain at engine volume 0 dB. Recalibrate after changing DAC,
OS/amp gain, speaker placement, listening distance, or headphones.

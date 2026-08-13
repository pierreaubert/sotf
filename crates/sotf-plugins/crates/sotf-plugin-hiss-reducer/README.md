# sotf-plugin-hiss-reducer

SOTF high-frequency noise reducer with a zero-latency default and an optional
fixed-latency spectral restoration mode.

The plugin wraps `plugins_denoiser::hiss::HissReducer` in the SOTF host trait.
It is deliberately a lightweight time-domain high-band downward expander, not
an STFT noise estimator: the Threshold control is an absolute dBFS high-band
level, not an SNR value. `Spectral mode` instead selects a 1024-point WOLA
minimum-statistics estimator for stationary hiss. It is structural because it
changes the plugin latency from 0 to exactly 1024 samples.

## DSP contract

- Interleaved, in-place, channel-independent processing with zero algorithmic
  latency and `O(frames × channels)` bounded work.
- An exact-mapped complementary one-pole split defines the high band. The
  shallow 6 dB/octave transition is intentional; live cutoff changes ramp over
  5 ms and the visible cutoff is limited to `min(16 kHz, 0.45 × sample rate)`.
- Fast (5 ms) and slow (100 ms) high-band power envelopes identify persistent
  low-level energy. A 30 ms persistence requirement, threshold hysteresis,
  20 ms hold, continuous reduction depth, and 1/50 ms gain attack/release avoid
  waveform-cycle modulation and binary gain clicks.
- Live bypass keeps analysis/filter state warm and crossfades over 5 ms. A
  plugin initialized disabled is exactly dry, as is settled bypass or zero
  Strength.
- Processing is allocation-free. Non-finite input is replaced with silence and
  decaying filter/envelope state snaps to zero before becoming subnormal.
- Spectral mode uses a periodic Hann window, 256-sample hop, and minimum
  statistics over eight slots spanning about 512 ms at every sample rate.
  Above Frequency, a calibrated high-band RMS Threshold gates 15/50 ms
  attack/release-smoothed Wiener gains; three-bin tonal-main-lobe protection
  preserves sustained narrowband programme. A 5 ms aligned wet/dry ramp keeps
  live bypass click-free. Disabled spectral processing stays latency-correct
  by emitting 1024-sample delayed dry audio; callbacks always return their
  requested frame count.

The plugin must be initialized at a supported nonzero sample rate before
processing, and each `ProcessContext` must use that same rate.

# sotf-plugin-hiss-reducer

SOTF zero-latency reducer for persistent, low-level high-frequency energy.

The plugin wraps `plugins_denoiser::hiss::HissReducer` in the SOTF host trait.
It is deliberately a lightweight time-domain high-band downward expander, not
an STFT noise estimator: the Threshold control is an absolute dBFS high-band
level, not an SNR value.

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

The plugin must be initialized at a supported nonzero sample rate before
processing, and each `ProcessContext` must use that same rate.

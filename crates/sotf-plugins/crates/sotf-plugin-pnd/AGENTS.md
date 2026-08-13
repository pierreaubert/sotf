# sotf-plugin-pnd

PND is a fixed-frame, duration-preserving pitch-drift correction insert.

Preserve these contracts:

- every successful callback returns and overwrites exactly `context.num_frames`;
- device-clock correction and variable-duration SRC stay outside this plugin;
- causal phase-vocoder latency is 2047 frames for every callback partition;
- initialization owns FFT planning/allocation; process and reset allocate nothing;
- all channels keep their order and frame count and share one correction ratio;
- multi-channel analysis excludes low-confidence observations before consensus;
- zero reference is change-only tracking and cannot identify constant offset;
- legacy `phase_vocoder` false/true values both migrate explicitly to the sole duration-preserving engine;
- no formant-preservation or transient phase-locking claim without implementation and tests.

Use impulse partition, long-duration correction, transactional error, detector
amplitude/SNR/bin-edge/motion, unity SNR, transient localization, multichannel,
and allocation tests for behavioral changes.

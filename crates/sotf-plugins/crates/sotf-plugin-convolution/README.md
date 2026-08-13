# sotf-plugin-convolution

SOTF realtime impulse-response convolution with uniform and non-uniform partitioned FFT backends.

Normal UPC/NUPC operation reports and preserves 1024 samples of latency, including while no IR is
loaded or a replacement is pending. The optional NUPC time-domain head reports zero latency.

IR decoding, resampling, FFT planning, and backend construction run off the audio thread. Async
requests are generation-tagged; failed or stale replacements leave the last-known-good IR active.
Old large states are reclaimed in the background. Use `load_status()` to distinguish idle, loading,
ready, and failed states.

WAV, FLAC, and AIFF IRs are supported. The loader requires valid sample-rate metadata and enforces
32 channels, 30 seconds, and a 512 MiB estimated realtime-backend budget.

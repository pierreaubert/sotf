# Declick usage

Construct Declick with a nonzero channel count and sample rate. Construction is
fallible because its fixed state and smoothing coefficients depend on both.

```rust
let mut declick = DeclickPlugin::new(2, 48_000)?;
```

The plugin consumes and overwrites exactly `num_frames * channels` interleaved
samples. `ProcessContext.sample_rate` must match the construction or latest
successful `initialize()` rate. Every call returns `num_frames`.

Declick always reports eight samples of latency. The first eight output frames
after construction, reset, or reinitialization are silence. Active and bypassed
audio then use the same delayed timeline, so parallel host branches stay
aligned. Reset clears delay and detector history but preserves parameters.

Detection uses eight samples before and after each candidate. Short deviations
that return to the surrounding trajectory are replaced by a robust pre/post
interpolation. Persistent steps, onsets, high-frequency tones, and square waves
are retained by the local-variation and return tests. Adjacent channel pairs
are linked by default; set `link_channels=false` for independent decisions.

Non-finite samples are replaced locally with the previous finite input and do
not poison later audio. The realtime path performs no allocation, locking,
logging, filesystem access, or worker dispatch.

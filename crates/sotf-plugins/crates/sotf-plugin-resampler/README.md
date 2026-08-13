# SOTF Resampler

Allocation-free streaming sample-rate conversion using rubato's asynchronous sinc resampler.

The plugin accepts interleaved audio and retains partial input until `chunk_size` frames are
available. The preallocated residual planes feed rubato directly without a second planar
full-chunk copy. `output_frames_for_input()` is a conservative destination-capacity bound;
`available_output_frames()` reports what the next call can emit immediately. The returned frame
count is authoritative and may be zero or larger than the input callback. `DawHost` preserves
that count rather than padding it with input-rate silence.

A fixed input callback size cannot also be a fixed output callback size when rates differ: 256
frames at 44.1 kHz span about 279 frames at 48 kHz. A device-facing fixed-frame consumer needs a
separate output-clock FIFO/pull scheduler at the clock-domain boundary. The plugin does not
silently relabel, duplicate, or discard samples to simulate one.

At end of stream, call the object-safe `Plugin::drain()` repeatedly until `complete` is true.
Drain follows rubato's complete-stream procedure: submit the final partial chunk, pump zero input
until the exact cumulative output length plus output-domain delay is present, and retain the final
sinc ringing. The host propagates each upstream tail through downstream plugins before draining
their own state. Seek, stop, or graph replacement uses `reset()` and discards pending state.

Equal configured rates with dynamic ratio disabled use a bit-exact, zero-latency copy path.
Non-finite samples are copied by the direct plugin API; `DawHost` rejects them at the graph
boundary. Enabling dynamic ratio disables unity bypass. `ratio` automation updates rubato in place
without allocation. `quality` is structural: canonical indices are Fast=0, Medium=1, High=2, and
an activated plugin rejects live changes so no pending audio or filter history is dropped.

Latency is reported entirely in output-rate frames: rubato's `output_delay()` plus the input-chunk
priming duration converted conservatively to the output clock. The host rate passed to
`initialize()` must equal the configured input rate, and downstream nodes are initialized and
processed at the Resampler's declared output rate.

Fast, Medium, and High use 64-, 128-, and 256-tap Blackman-Harris-windowed sinc filters with
rubato's Linear table interpolation. Longer filters provide a narrower transition band and more
stop-band rejection at higher CPU cost.

Run `cargo test -p sotf-plugin-resampler` and
`cargo run -p sotf-plugin-resampler --features qa --bin qa-resampler`.

# sotf-plugin-resampler

Active code is split across `resampler_plugin.rs`, `resampler_quality.rs`, `params.rs`, unit and
integration tests, and `bin/qa_resampler.rs`. Audio is interleaved at the plugin boundary and
planar inside rubato.

Preserve these contracts:

- returned output frames are authoritative; never pad zero-output callbacks to input frames;
- distinguish destination capacity from immediately available output;
- drain through the object-safe `Plugin` contract until complete and preserve the sinc tail;
- report latency only in output-clock frames;
- reject a host input clock different from the configured input rate;
- keep ratio automation allocation-free and quality structural/off-thread;
- keep quality indices Fast=0, Medium=1, High=2 consistent across ParamSpec, Plugin, bridge, FFI;
- unity non-dynamic operation is bit-exact and zero latency;
- all capacity errors are transactional and retry-equivalent;
- reset clears residual, drain, ratio, and last-output state.

Use impulse, tone, residual-boundary, extreme-ratio, irregular-partition, high-channel, allocation,
latency, bridge, host topology, and EOF-chain tests for behavioral changes.

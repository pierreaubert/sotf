# sotf-plugin-delay

SOTF Delay plugin with feedback.

Audio delay line supporting up to 5000ms of delay with feedback control.

The default moving-head mode retains tape-style Doppler motion. The opt-in
`pitch_preserving` mode instead transitions between two fixed fractional-delay
taps for 20 ms. Every nonidentical tap change fades the old tap fully out before
fading the new tap in. This conservative switch avoids moving-head Doppler,
phase rotation, and destructive summing for arbitrary input. It is
callback-partition independent, adds no fixed latency, and performs no
processing-time allocation. Existing presets default it off. LFO rate and depth
must both be zero in this mode: input-agnostic modulation between differently
delayed taps cannot guarantee carrier and phase retention without nulls.

Delay-time automation is tape-style: moving a read head produces the expected
Doppler pitch glide. Four-point Lagrange interpolation controls fractional-tap
error but does not make time changes pitch preserving. LFO motion clamps only
the out-of-range half-cycle at a delay boundary. One shared LFO phase is
intentional: it preserves stereo/multichannel image coherence rather than
turning Delay into a phase-spread chorus.

`try_new_with_max_delay` and `new_per_channel_with_max_delay` declare the
maximum live automation range and size the ring accordingly. The ordinary
scalar constructor retains the full five-second range; the RoomEQ per-channel
constructor uses the largest configured route delay and exposes no effect
controls.

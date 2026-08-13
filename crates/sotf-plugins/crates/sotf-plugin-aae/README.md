# sotf-plugin-aae

SOTF Active Acoustic Enhancement plugin — LARES-inspired multichannel reverb.

Takes stereo input and outputs multichannel audio (5.0–9.1.6) with synthesized early reflections and late reverberation (8-line Hadamard FDN, time-variant) distributed across speakers via VBAP.

Speaker layout and room preset are setup-only controls: changing either requires
the host to build and initialize a replacement plugin. Level automation is
sample-smoothed with a 5 ms one-pole envelope and is independent of host block
size. Construction rejects non-finite/out-of-range state and unknown layout or
preset names. Initialized processing performs no heap allocation.

`room_size` scales only the late-reverb FDN delays; the selected room preset
defines early-reflection timing. Live pre-delay and room-size changes use 10 ms
dual-read-head transitions, while RT60 filter coefficients interpolate over
5 ms. The safety control is emergency feedback headroom above nominal full
scale, so the default +6 dB setting does not distort normal-level decay.

The LFE output is a synthesized effects send, not complementary bass
management. It uses a fourth-order Linkwitz-Riley low-pass at 120 Hz; main
channels are not high-passed. Spatial VBAP rows exclude LFE and retain only the
three strongest normalized speakers.

Bypass is continuous-tail: reverb, dialogue detection, metering, auto gain, and
limiting continue to advance while output crossfades over 5 ms to dry FL/FR.
AAE reports zero algorithmic latency because its direct path is immediate.

Quality regressions cover block-partition invariance, LFE rejection at 250 Hz
and 1 kHz, normalized FDN inter-line correlation, panned-dialogue detection,
percussion rejection, delay/filter transition state, sparse routing, tail
continuity, and level-safe feedback behavior. `qa-aae` additionally exercises
the maximum 9.1.6/Cathedral configuration with content awareness and auto gain,
reporting callback p50/p95/max time against the audio deadline.

`qa-aae-quality` is the allocation-permitted offline measurement program. It
prints TSV records for bandwise Schroeder RT60, echo density/mixing time,
spatial coherence/energy/diffuseness, LFE transfer, modulation sidebands,
distortion/limiter activity, and synthetic dialogue-detector precision/recall
over a representative preset/layout/rate/partition matrix. See
`quality-validation.md` for the separate external listening/corpus protocol;
synthetic results are never presented as listening evidence.

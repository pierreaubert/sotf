# sotf-plugin-gate

SOTF Gate plugin for noise gating.

Attenuates audio below a configurable threshold to remove low-level noise and unwanted background sound.

The plugin is stateful and must be initialized before processing. Each callback
must use the initialized sample rate and provide exactly `num_frames *
input_channels()` interleaved samples. External-sidechain mode uses programme
channels followed by matching sidechain channels in each frame and never writes
the sidechain samples.

Channel linking, sidechain HPF frequency/order, detection mode, external
sidechain mode, and lookahead are structural settings: change them by rebuilding
the graph. A runtime write of the existing value is accepted as a no-op.
A rejected live structural write does not move or clear the active delay line.
Hosts replacing a lookahead configuration must align the old/new plan latency
before crossfading; the plugin cannot compensate a graph by itself.
`range_db = 0` means unlimited attenuation with a finite 240 dB numerical ceiling.
Processing, realtime parameter writes, and reset are allocation-free; non-finite
audio and detector samples are treated as silence before entering DSP state.

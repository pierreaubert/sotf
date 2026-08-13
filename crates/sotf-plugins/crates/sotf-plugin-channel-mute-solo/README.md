# sotf-plugin-channel-mute-solo

SOTF Channel Mute/Solo plugin for per-channel mute, solo, and dim.

Provides independent mute, solo, and dim controls for each audio channel with
smoothed gain transitions to avoid clicks. The default 5 ms setting is the time
constant of a per-sample one-pole smoother (about 63% of the gain change), not
an exact fade duration.

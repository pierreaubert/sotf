# sotf-plugin-band-merge

SOTF Band Merge plugin for merging frequency bands.

Recombines frequency bands that were previously split by the Band Split plugin back into a single full-range signal.

The plugin must be initialized before processing. Every callback must use the
initialized sample rate and exact overflow-checked interleaved input/output
lengths. Band count is structural; exact no-op writes succeed and actual changes
require graph replacement. Gain and mute automation share an allocation-free
10 ms smoother, while reset snaps each band to its configured mute/gain target.

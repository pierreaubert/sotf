# sotf-plugin-downmix

SOTF Downmix plugin for multichannel to stereo downmixing.

Phase-coherent downmix from multichannel formats to stereo with explicit speaker-layout awareness for correct channel summation. Ambiguous 8- and 10-channel inputs require `input_layout` (for example `7.1`, `5.1.2`, `5.1.4`, or `7.1.2`). Phase-coherent Lo/Ro and matrix Lt/Rt are mutually exclusive structural modes; both use a fixed 2048-sample WOLA latency. Lt/Rt surround encoding uses a unity-magnitude spectral ±90° rotation.

The plugin applies the documented matrix gains exactly and does not silently
normalize the mix. Correlated full-scale channels can exceed 0 dBFS; reserve
headroom or add an explicit limiter downstream.

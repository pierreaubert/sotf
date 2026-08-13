# sotf-plugin-multiband-expander

SOTF Multiband Expander plugin for multiband dynamic range expansion.

Splits the signal into frequency bands and applies independent expansion to each band for frequency-selective dynamics processing.

The same crate also owns the `expander` factory identity, which is a genuine
one-band broadband path with no crossover controls. The multiband identity
requires 2-5 bands. Band count and time-domain/spectral mode are structural and
must be changed by rebuilding the plugin off the audio thread.

Time-domain mode supports Peak/RMS detection, linked or independent channels,
sidechain HPF, lookahead, and auto makeup. Spectral mode uses a 1024-point dual
Hann STFT at 75% overlap (`hop=N/4`), `1/(1.5N)` normalization, and 1024 samples
of latency; it accepts only linked Peak detection with zero lookahead/HPF and no
auto makeup, so unsupported behavior is never silently ignored.

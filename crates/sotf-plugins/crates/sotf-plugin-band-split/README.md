# sotf-plugin-band-split

SOTF Band Split plugin for splitting a signal into frequency bands.

Splits an audio signal into multiple frequency bands using Linkwitz-Riley crossover filters. Two-band
sums have complementary responses; cascaded three- and four-band splits have unequal group delays
and are not phase-perfect.

The plugin supports 2–4 bands, LR24/LR48 slopes, 20 Hz through the lower of
20 kHz and 0.49 × sample rate, and every catalogued input layout through 12
channels. Output is band-major and latency is zero. Frequency and gain
automation are smoothed at audio rate; expensive IIR coefficient redesign is
bounded to a persistent 6 kHz control rate so behavior is callback-partition
invariant. LR24/LR48 selection is structural and requires graph rebuild.

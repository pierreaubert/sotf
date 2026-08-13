# sotf-plugin-crossover

SOTF Crossover plugin for frequency band splitting.

Splits audio into frequency bands using Linkwitz-Riley filters for use in multiband processing or multi-way speaker management.

`CrossoverPlugin::fir_memory_report()` reports the compiled FIR coefficient,
history, alignment-delay, scratch, and total byte counts before graph admission.
LR crossovers return `None`. Processing uses allocation-free interleaved block
kernels with the generic scalar multiband/per-channel paths retained.

# 0.5.114

- Corrected diffuseness/DOA analysis to use the full active-intensity vector, not just the real axis.
- Removed hot-path Vec allocations from smoothed LFE crossover table refresh.
- Moved input/output buffer validation before bypass processing so bypass returns clean errors instead of panicking.
- Marked FFT/decorrelation rebuilding controls as structural/setup so hosts don’t treat them as realtime automation targets.
- Added regressions for quadrature intensity classification and bypass buffer mismatch handling.

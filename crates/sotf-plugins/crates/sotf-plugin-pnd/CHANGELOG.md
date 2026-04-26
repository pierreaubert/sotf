# 0.5.2

- Added input/output buffer validation in both resampler and phase-vocoder paths, so bad host buffers return Err instead of slicing/panicking.
- Added prepared-capacity checks for oversized blocks and output-ring overflow instead of relying on debug assertions or ring overruns.
- Removed phase-vocoder hot-path resize() by rejecting blocks larger than prepared capacity.
- Fixed PndAnalyzer::reset() leaving median_scratch length zero, which could panic after the next valid drift estimate.
- Preallocated analyzer peak/ratio scratch and max drift-history storage so analysis-window changes do not reallocate.
- Marked analysis-window, multi-channel analysis, and phase-vocoder controls as structural/setup.
- Fixed from_params() returning stale cached parameter values.

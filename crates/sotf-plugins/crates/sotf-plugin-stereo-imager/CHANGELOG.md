# Unreleased

## Fixes
- Fixed unsmooted crossover frequency changes: `low_mid_freq` and `mid_high_freq` now use `LogSmoother` (50 ms) instead of instant coefficient updates, preventing clicks during automation.
- Fixed real-time allocation in `set_parameter()`: removed `cached_parameters` Vec and `rebuild_cached_parameters()`, replacing it with an on-the-fly `parameters()` method.



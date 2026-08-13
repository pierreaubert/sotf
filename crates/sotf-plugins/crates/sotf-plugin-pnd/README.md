# sotf-plugin-pnd

SOTF PND plugin for experimental polyphonic drift analysis and pitch correction.

The duration-preserving mode uses an STFT phase vocoder with instantaneous-frequency
estimation and spectral-bin remapping. It does not preserve formants. The legacy
Rubato path is variable-rate SRC constrained by the fixed-frame insert contract;
it is retained for compatibility but is not a general clock-domain synchronizer.

Automatic drift estimates compare adjacent analysis frames. Without an explicit
pilot, note, or clock reference they can detect change but cannot identify a
constant absolute pitch offset.

`drift_smoothing` is stored in seconds (`0.001..1.0`) and displayed in
milliseconds. It is an analysis-sample-clock time constant, so host callback
partitioning does not change correction convergence.

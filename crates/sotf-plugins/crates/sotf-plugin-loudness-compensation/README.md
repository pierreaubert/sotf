# sotf-plugin-loudness-compensation

SOTF Loudness Compensation plugin for equal loudness contour compensation.

Provides Manual tone controls, a jointly fitted ISO 226 contour, and a calibrated
Auto mode. The default level policy preserves the ISO 226 1 kHz reference;
optional headroom normalization is explicit and its broadband level shift remains
visible. Auto mode requires a measured SPL calibration for the playback chain.

See `USAGE.md` for calibration, AutoGain position, headroom, and realtime-update
contracts.

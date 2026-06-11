# math-rir

Room Impulse Response analysis: SSIR-based reflection detection, segmentation, mixing time estimation, and ISO 3382 room-acoustic metrics.

## Architecture

Two analysis paths share a common configuration and helper layer:

1. SSIR (Spatial Segmentation of Impulse Response) from Pawlak & Lee (Applied Acoustics 249, 2026).
2. ISO 3382-1/-2 room-acoustic parameters (EDT, T20, T30, C50, C80, D50, Ts) with optional octave / third-octave band filtering.

- `config.rs` — `SsirConfig`: analysis parameters (sample rate, thresholds)
- `detection.rs` — `detect_reflections()`, `find_direct_sound_toa()`: peak detection and TOA estimation
- `segmentation.rs` — `build_segments()`: splits RIR into variable-length sound events
- `mixing_time.rs` — `estimate_mixing_time()`: estimates when reflections become diffuse
- `types.rs` — `RirSegment` (per-reflection data: start, end, TOA, energy), `SsirResult` (full analysis result)
- `metrics.rs` — Schroeder backward integration, `DecayCurve`, `Iso3382Metrics`, `analyze_iso3382()`
- `bands.rs` — ISO/IEC 61260 octave & third-octave centres, zero-phase Butterworth `bandpass()`, `analyze_iso3382_octaves()`, `analyze_iso3382_third_octaves()`, `analyze_iso3382_bands()`

## Key Public API

- `analyze_rir(rir, config) -> SsirResult` — SSIR segmentation entry point (`lib.rs`)
- `analyze_iso3382(rir, sr) -> Iso3382Metrics` — broadband ISO 3382 parameters (`metrics.rs`)
- `analyze_iso3382_octaves(rir, sr) -> Vec<(f64, Iso3382Metrics)>` — per-octave parallel analysis (`bands.rs`)
- `analyze_iso3382_third_octaves(rir, sr)` — per-third-octave version (`bands.rs`)
- `bandpass(rir, fc, BandWidth::Octave, sr, order)` — zero-phase Butterworth bandpass (`bands.rs`)
- `DecayCurve::from_rir(...)` — Schroeder backward-integrated decay curve in dB (`metrics.rs`)
- `SsirConfig::new(sample_rate)` — SSIR config with defaults (`config.rs`)
- `SsirResult::num_events()`, `num_reflections()`, `mixing_time_ms()`, `reflections()` — result accessors (`types.rs`)
- `RirSegment::toa_ms(sr)`, `duration_ms(sr)` — per-reflection metrics (`types.rs`)
- Re-exports `filtfilt` from `math-iir-fir`

## Testing

```bash
cargo test -p math-rir
```

## Important Notes

- Mono RIR input only for ISO 3382 broadband analysis — multi-channel analysis requires per-channel calls
- SSIR multi-channel: `analyze_srir(channels, config)` takes a B-format RIR (W, X, Y, Z)
- Depends on `math-iir-fir` for the bandpass filtering used by both DOA band-limiting and octave-band analysis (`filtfilt`)
- Schroeder integration starts at the detected direct-sound arrival; noise-tail truncation uses Chu's method (simple two-pass estimator). For ISO-grade reporting on real measurements consider feeding the noise cutoff explicitly via `DecayCurve::from_rir`
- Each ISO 3382 reverberation time carries its linear-fit `r²`; treat `r² < 0.95` as a quality warning (per ISO 3382-1 Annex B)

# math-rir

Room Impulse Response analysis: SSIR-based reflection detection, segmentation, and mixing time estimation.

## Architecture

Implements the SSIR (Spatial Segmentation of Impulse Response) method from Pawlak & Lee (Applied Acoustics 249, 2026).

- `config.rs` — `SsirConfig`: analysis parameters (sample rate, thresholds)
- `detection.rs` — `detect_reflections()`, `find_direct_sound_toa()`: peak detection and TOA estimation
- `segmentation.rs` — `build_segments()`: splits RIR into variable-length sound events
- `mixing_time.rs` — `estimate_mixing_time()`: estimates when reflections become diffuse
- `types.rs` — `RirSegment` (per-reflection data: start, end, TOA, energy), `SsirResult` (full analysis result)

## Key Public API

- `analyze_rir(rir, config) -> SsirResult` — main entry point, analyzes a mono RIR (`lib.rs`)
- `SsirConfig::new(sample_rate)` — config with defaults (`config.rs`)
- `SsirResult::num_events()`, `num_reflections()`, `mixing_time_ms()`, `reflections()` — result accessors (`types.rs`)
- `RirSegment::toa_ms(sr)`, `duration_ms(sr)` — per-reflection metrics (`types.rs`)
- Re-exports `filtfilt` from `math-iir-fir`

## Testing

```bash
cargo test -p math-rir
```

## Important Notes

- Mono RIR input only — multi-channel analysis requires per-channel calls
- Depends on `math-iir-fir` for bandpass filtering (`filtfilt`)
- The SSIR method preserves full temporal energy while enabling per-reflection manipulation

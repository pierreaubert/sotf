# Track: RoomEQ Cardioid Subwoofer Support

## 1. Overview
Implement support for Gradient Cardioid subwoofer arrays (2 subs).
The user specifies `front` and `rear` measurements and `separation_meters`.
The system applies fixed processing to the rear sub (Delay = separation/c, Invert) to achieve cardioid directivity, then optimizes the combined response.

## 2. Changes

### 2.1 Types (`crates/autoeq/src/roomeq/types.rs`)
*   Add `CardioidConfig` struct:
    *   `front`: `MeasurementSource`
    *   `rear`: `MeasurementSource`
    *   `separation_meters`: `f64`
    *   `name`: `String` (optional)
*   Update `SpeakerConfig` enum to include `Cardioid(CardioidConfig)`.

### 2.2 Optimize (`crates/autoeq/src/roomeq/optimize.rs`)
*   Update `process_speaker_internal` to handle `SpeakerConfig::Cardioid`.
*   Implement `process_cardioid`:
    *   Load measurements.
    *   Calculate delay `tau = separation / 343.0`.
    *   Simulate combined response: `Sum = Front + Rear * Invert * Delay(tau)`.
    *   Optimize EQ on `Sum`.
    *   Return chain with appropriate processing.

### 2.3 Output (`crates/autoeq/src/roomeq/output.rs`)
*   Add `build_cardioid_dsp_chain` helper.
    *   Ch 1 (Front): EQ, Gain
    *   Ch 2 (Rear): EQ, Gain, Invert, Delay

## 3. Risks
*   Assumes measurements are taken at MLP.
*   Assumes standard gradient cardioid (end-fire or stacked gradient).

## 4. Verification
*   Test compilation.
*   Validate schema.

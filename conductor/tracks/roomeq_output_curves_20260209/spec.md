# Track: RoomEQ Output Curves & Phase

## 1. Overview
The user reported missing final curves in the output and requested "freq, spl, phase".
Currently `CurveData` lacks phase, and `workflows.rs` leaves `final_curve` as `None` in some paths.

## 2. Changes

### 2.1 Types (`crates/autoeq/src/roomeq/types.rs`)
*   Update `CurveData`:
    *   Add `pub phase: Option<Vec<f64>>`.
*   Update `From<Curve> for CurveData` to copy phase.
*   Update `From<CurveData> for Curve` to restore phase.

### 2.2 Workflows (`crates/autoeq/src/roomeq/workflows.rs`)
*   In `optimize_stereo_2_0`:
    *   Calculate `final_curve` (apply filters/gain to aligned curve).
    *   Populate `chain.final_curve`.
*   In `optimize_stereo_2_1`:
    *   Calculate `final_curve` for L, R, Sub.
        *   Need to simulate the full chain: Alignment -> Pre-EQ -> Crossover -> Post-EQ.
    *   Populate `chain.final_curve`.

### 2.3 Optimize (`crates/autoeq/src/roomeq/optimize.rs`)
*   Verify `process_single_speaker` etc. correctly populate phase.

## 3. Risks
*   Phase data might be missing in input measurements (`None`). `CurveData` `phase` should be optional.
*   Calculating final response requires applying all filter stages. `response::apply_complex_response` handles this.

## 4. Verification
*   Compile check.
*   Run `roomeq` test or fuzzer and inspect output JSON (if possible) or check code logic.

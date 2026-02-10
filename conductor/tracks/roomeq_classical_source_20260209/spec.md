# Track: RoomEQ Classical Source

## 1. Overview
Add a `Classical` source type to `math-xem-common` which models a directional source with frequency-dependent beamwidth (narrowing as frequency increases).
Update `autoeq-datagen` to use this source for mains (non-subwoofers), while keeping `Omni` for subwoofers.

## 2. Changes

### 2.1 math-xem-common (`crates/math-xem-common/src/source.rs`)
*   Refactor `DirectivityPattern` to `Directivity` enum.
    *   `Sampled(DirectivityGrid)` (renamed from `DirectivityPattern` struct).
    *   `Classical { h_angle: f64, v_angle: f64 }`.
*   Implement `Directivity::amplitude(theta, phi, frequency)`.
    *   For `Sampled`, ignore frequency (legacy behavior).
    *   For `Classical`, compute beamwidth based on frequency:
        *   f < 80: Omni (360).
        *   f > 500: Target `h_angle`/`v_angle`.
        *   Transition between 80 and 500.
    *   Use Gaussian model `0.5^(x^2)` for attenuation.

### 2.2 autoeq-datagen (`crates/autoeq-datagen/src/scenarios.rs`)
*   Update `make_hp_source` and `make_fullrange_source` to use `Source::classical(pos, 60.0, 40.0, amp)`.
*   Keep `make_lp_source` (subs) as `Source::omnidirectional`.

## 3. Risks
*   Refactoring `DirectivityPattern` to Enum breaks serialization format if not careful. `#[serde(untagged)]` might help or just accepting the breaking change (it's internal tool). I'll use standard enum serialization.
*   Interpolation logic for `Classical` pattern needs to be robust (handle wrap-around, poles).

## 4. Verification
*   Compile.
*   Unit tests in `source.rs` for `Classical` pattern attenuation.

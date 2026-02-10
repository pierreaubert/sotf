# Track: RoomEQ Crossover Refactor

## 1. Overview
The user wants to unify crossover handling. Currently, multi-driver speakers use the `crossovers` section, but subwoofers use a separate `bass_management` section.
The goal is to remove `bass_management` and have `system.subwoofers` reference a crossover definition from the `crossovers` map, just like `SpeakerGroup` does.

## 2. Changes

### 2.1 Types (`crates/autoeq/src/roomeq/types.rs`)
*   `SubwooferSystemConfig`: Add `pub crossover: Option<String>`.
*   `RoomConfig`: Remove `bass_management`.
*   Remove `BassManagementConfig` struct and related defaults.

### 2.2 Workflows (`crates/autoeq/src/roomeq/workflows.rs`)
*   In `optimize_stereo_2_1`:
    *   Get crossover key from `sys.subwoofers.crossover`.
    *   Look up `CrossoverConfig` in `config.crossovers`.
    *   Extract frequency.
    *   Remove reliance on `bass_management`.

### 2.3 Schema & Docs
*   Update `input_schema.json`:
    *   Add `crossover` to `SubwooferSystemConfig`.
    *   Remove `bass_management`.
*   Update `INPUT_FORMAT.md` and `README.md`.

### 2.4 Legacy Conversion (`convert_recording.rs`)
*   Remove `bass_management` field initialization.

## 3. Risks
*   `SubwooferSystemConfig` uses `#[serde(flatten)]` for the mapping. Adding a named field `crossover` works fine with flattening (serde handles struct fields first, then flattened map).
*   Need to ensure `CrossoverConfig` supports what `BassManagement` did. `BassManagement` had `lfe_slope`. `CrossoverConfig` has `type` (e.g. "LR24"). LR24 implies 24dB slope. If user wants custom slope, they choose a different type. This seems acceptable and cleaner.

## 4. Verification
*   Compile check.
*   Schema validation check.

# Specification: SPL Normalization & Group Consistency for RoomEQ

## Overview
Implement proper SPL normalization for input curves in the `roomeq` system, ensuring that limited-bandwidth devices (like subwoofers) are normalized over their relevant range. This track includes updating the data schema with `speaker_name`, versioning the schema, and adapting the converter and fuzzer tools.

## Terminology
- **Measurement Group:** The existing RoomEQ concept where multiple measurements (e.g., different positions) are combined for a single target.
- **Acoustic Speaker Group:** A concept representing speakers expected to be acoustically similar (e.g., a stereo pair).

## Functional Requirements
- **Data Schema & Versioning:**
  - Add `speaker_name` field to the input data format (e.g., "Genelec 8361A").
  - **Validation:** `speaker_name` must be alphanumeric, spaces, and hyphens only.
  - **Schema Version:** Increment the schema version to reflect the addition of `speaker_name` and normalization parameters.
  - Update internal data structures in `crates/autoeq-roomsim`.

- **Frequency Range Averaging:**
  - Implement `compute_average_response(curve, freq_range: Option<(f64, f64)>)`.
  - Implement `find_db_point(curve, target_db)` using linear interpolation to detect passband edges (e.g., -3dB points).
  - Use detected or specified ranges to ensure subwoofers are normalized only over their active bandwidth.

- **Acoustic Group Identification:**
  - **Primary:** Group speakers with the same `speaker_name` and symmetrical positions (e.g., "Genelec 8361A" at L and R).
  - **Fallback Heuristics:** `L`/`R`, `SL`/`SR`, `SBL`/`SBR`, and Top pairs are grouped if they share similar characteristics.

- **Speaker Consistency Warnings:**
  - **Range Difference Check:** Warning if average SPL of the detected range differs by > **3 dB** within a group.
  - **Octave-Wise Difference Check:** Warning if any single octave differs by > **6 dB** within a group.
  - **Warning Format:** "Speaker group [group_name] has significant difference: [metric] [value] dB."

## Tooling Adaptations
- **Converter (`crates/autoeq/bin/convert_recording.rs`):**
  - Update to support parsing and injecting the `speaker_name` field.
  - Ensure compatibility with the new schema version.
- **Room Fuzzer:**
  - Adapt the fuzzer to generate randomized but valid `speaker_name` values.
  - Update fuzzer logic to handle the new schema and normalization fields.

## Implementation Details
- **Target Location:** `crates/autoeq-roomsim` for core logic; `crates/math-dsp` for utility functions; `crates/autoeq` for conversion.
- **Averaging:** Log-frequency weighted averaging.

## Acceptance Criteria
- [ ] Schema version is incremented and `speaker_name` is correctly parsed/validated.
- [ ] `compute_average_response` accurately handles band-limited normalization.
- [ ] Converter and Fuzzer are updated and functional with the new schema.
- [ ] System accurately warns about >3dB range differences and >6dB octave differences.
- [ ] Unit tests cover normalization, grouping, and warning thresholds.

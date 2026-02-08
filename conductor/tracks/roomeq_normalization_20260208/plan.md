# Implementation Plan: SPL Normalization & Group Consistency for RoomEQ

This plan follows the TDD workflow: Write Failing Tests -> Implement -> Refactor.

## Phase 1: Core DSP Utilities
Implement the fundamental mathematical functions for passband detection and range-aware averaging.

- [x] Task: Implement `find_db_point` in `math-dsp` b4684217
    - [ ] Write tests for finding frequencies at -3dB (low and high pass) using linear interpolation
    - [ ] Implement `find_db_point(curve, target_db)` logic
- [ ] Task: Implement range-aware `compute_average_response`
    - [ ] Write tests for averaging over full-band and specific sub-ranges
    - [ ] Implement `compute_average_response(curve, freq_range: Option<(f64, f64)>)`
- [ ] Task: Conductor - User Manual Verification 'Core DSP Utilities' (Protocol in workflow.md)

## Phase 2: Schema & Data Model Updates
Update the data structures to support the new `speaker_name` field and schema versioning.

- [ ] Task: Update `autoeq-roomsim` data structures
    - [ ] Write tests for serializing/deserializing the updated schema with `speaker_name`
    - [ ] Add `speaker_name` field to relevant structs (e.g., `SpeakerConfig`, `Measurement`)
    - [ ] Increment the schema version constant
- [ ] Task: Implement `speaker_name` validation
    - [ ] Write tests for valid/invalid speaker names (alphanumeric, spaces, hyphens)
    - [ ] Implement validation logic (likely a `Validate` trait or a constructor check)
- [ ] Task: Conductor - User Manual Verification 'Schema & Data Model Updates' (Protocol in workflow.md)

## Phase 3: Tooling Adaptation
Update the converter and fuzzer to work with the new schema.

- [ ] Task: Update Converter (`crates/autoeq/bin/convert_recording.rs`)
    - [ ] Write tests for the converter with the new `speaker_name` field
    - [ ] Update parsing logic to extract and inject `speaker_name`
    - [ ] Ensure compatibility with the new schema version
- [ ] Task: Update Room Fuzzer (`crates/autoeq/bin/roomeq_fuzzer.rs`)
    - [ ] Update fuzzer to generate randomized but valid `speaker_name` values
    - [ ] Adapt fuzzer to handle the updated schema version
- [ ] Task: Conductor - User Manual Verification 'Tooling Adaptation' (Protocol in workflow.md)

## Phase 4: Normalization & Grouping Logic
Implement the logic to detect passbands and group speakers for comparison.

- [ ] Task: Implement automatic passband detection for normalization
    - [ ] Write tests verifying that subwoofers are normalized only over their active range (-3dB points)
    - [ ] Integrate `find_db_point` into the normalization pipeline in `autoeq-roomsim`
- [ ] Task: Implement Acoustic Group identification
    - [ ] Write tests for the grouping heuristics (L/R, SL/SR, etc.)
    - [ ] Implement grouping logic based on `speaker_name` and speaker position labels
- [ ] Task: Conductor - User Manual Verification 'Normalization & Grouping Logic' (Protocol in workflow.md)

## Phase 5: Consistency Warnings
Implement the acoustic comparison checks and warning reporting.

- [ ] Task: Implement Range Difference warning (3dB threshold)
    - [ ] Write tests that trigger a warning when two speakers in a group differ by > 3dB
    - [ ] Implement the range-based comparison logic
- [ ] Task: Implement Octave-Wise Difference warning (6dB threshold)
    - [ ] Write tests for octave-specific differences (e.g., 100-200Hz) exceeding 6dB
    - [ ] Implement octave-wise segmentation and comparison logic
- [ ] Task: Conductor - User Manual Verification 'Consistency Warnings' (Protocol in workflow.md)

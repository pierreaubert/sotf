# Implementation Plan: SPL Normalization & Group Consistency for RoomEQ

This plan follows the TDD workflow: Write Failing Tests -> Implement -> Refactor.

## Phase 1: Core DSP Utilities [checkpoint: 483ad0f]
Implement the fundamental mathematical functions for passband detection and range-aware averaging.

- [x] Task: Implement `find_db_point` in `math-dsp` b4684217
    - [x] Write tests for finding frequencies at -3dB (low and high pass) using linear interpolation
    - [x] Implement `find_db_point(curve, target_db)` logic
- [x] Task: Implement range-aware `compute_average_response` a2abd9e6
    - [x] Write tests for averaging over full-band and specific sub-ranges
    - [x] Implement `compute_average_response(curve, freq_range: Option<(f64, f64)>)`
- [x] Task: Conductor - User Manual Verification 'Core DSP Utilities' (Protocol in workflow.md) 483ad0f

## Phase 2: Schema & Data Model Updates [checkpoint: db29cf3]
Update the data structures to support the new `speaker_name` field and schema versioning.

- [x] Task: Update `autoeq-roomsim` data structures 7ca1ccfd
    - [x] Write tests for serializing/deserializing the updated schema with `speaker_name`
    - [x] Add `speaker_name` field to relevant structs (e.g., `SpeakerConfig`, `Measurement`)
    - [x] Increment the schema version constant
- [x] Task: Implement `speaker_name` validation 7ca1ccfd
    - [x] Write tests for valid/invalid speaker names (alphanumeric, spaces, hyphens)
    - [x] Implement validation logic (likely a `Validate` trait or a constructor check)
- [x] Task: Conductor - User Manual Verification 'Schema & Data Model Updates' (Protocol in workflow.md) db29cf3

## Phase 3: Tooling Adaptation [checkpoint: b63a853]
Update the converter and fuzzer to work with the new schema.

- [x] Task: Update Converter (`crates/autoeq/bin/convert_recording.rs`)
    - [x] Write tests for the converter with the new `speaker_name` field
    - [x] Update parsing logic to extract and inject `speaker_name`
    - [x] Ensure compatibility with the new schema version
- [x] Task: Update Room Fuzzer (`crates/autoeq/bin/roomeq_fuzzer.rs`)
    - [x] Update fuzzer to generate randomized but valid `speaker_name` values
    - [x] Adapt fuzzer to handle the updated schema version
- [x] Task: Conductor - User Manual Verification 'Tooling Adaptation' (Protocol in workflow.md) b63a853

## Phase 4: Normalization & Grouping Logic [checkpoint: 02118c4]
Implement the logic to detect passbands and group speakers for comparison.

- [x] Task: Implement automatic passband detection for normalization 8931382c
    - [x] Write tests verifying that subwoofers are normalized only over their active range (-3dB points)
    - [x] Integrate `find_db_point` into the normalization pipeline in `autoeq-roomsim`
- [x] Task: Implement Acoustic Group identification 8d7ad686
    - [x] Write tests for the grouping heuristics (L/R, SL/SR, etc.)
    - [x] Implement grouping logic based on `speaker_name` and speaker position labels
- [x] Task: Conductor - User Manual Verification 'Normalization & Grouping Logic' (Protocol in workflow.md) 02118c4

## Phase 5: Consistency Warnings [checkpoint: c450c5f]
Implement the acoustic comparison checks and warning reporting.

- [x] Task: Implement Range Difference warning (3dB threshold) c450c5fd
    - [x] Write tests that trigger a warning when two speakers in a group differ by > 3dB
    - [x] Implement the range-based comparison logic
- [x] Task: Implement Octave-Wise Difference warning (6dB threshold) c450c5fd
    - [x] Write tests for octave-specific differences (e.g., 100-200Hz) exceeding 6dB
    - [x] Implement octave-wise segmentation and comparison logic
- [x] Task: Conductor - User Manual Verification 'Consistency Warnings' (Protocol in workflow.md) c450c5f

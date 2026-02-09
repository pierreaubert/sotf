# Implementation Plan - RoomEQ v2 Core Optimizer Refactor

## Phase 1: Configuration Schema & Ecosystem Migration
- [x] Task: Update `RoomConfig` and `OptimizerConfig` to match v2 Spec 6b3865b
    - [x] Define explicit `ProcessingMode` enum (LowLatency, PhaseLinear, Hybrid).
    - [x] Consolidate Subwoofer strategies into a `BassManagement` config section.
    - [x] Add configuration fields for GD-Opt and VoG targets.
- [x] Task: Update Ecosystem Adapters 6b3865b
    - [x] Update `autoeq-datagen` to generate v2-compliant config files.
    - [x] Update `convert_recording` tool to handle new schema.
    - [x] Update documentation to reflect the new JSON structure.
- [x] Task: Conductor - User Manual Verification 'Configuration Schema & Ecosystem Migration' (Protocol in workflow.md) 6b3865b

## Phase 2: Pipeline Orchestration (The "Conductor" Engine)
- [x] Task: Refactor `optimize_room` entry point to enforce v2 Workflow f64595a
    - [x] Implement "Phase 2: Computation" sequence from spec:
        1. Sanity Check (Polarity/Dead channels).
        2. Bass Logic (Route to existing MSO/DBA/Single modules).
        3. Main Alignment (T-Zero).
        4. Processing Mode Execution (A/B/C).
- [x] Task: Validate & Wire "Bass Logic" f64595a
    - [x] Ensure MSO/DBA modules return a "Virtual Sub" response that feeds into the Main Alignment step correctly.
    - [x] Verify Pre-Correction (Linearization) happens *before* this stage.
- [x] Task: Conductor - User Manual Verification 'Pipeline Orchestration' (Protocol in workflow.md) f64595a

## Phase 3: Processing Modes & Advanced Calibration
- [ ] Task: Validate & Refine Mode Logic
    - [ ] **Mode A (Low-Latency):** Enforce IIR-only + LR4 crossovers.
    - [ ] **Mode B/C (FIR/Hybrid):** Enforce Brick Wall crossovers (if selected) and Windowing.
- [ ] Task: Implement Group Delay Optimization (GD-Opt)
    - [ ] *Note: This appears to be the primary new logic to integrate.*
    - [ ] Compute GD derivative.
    - [ ] Generate All-Pass (IIR) or Excess Phase Inversion (FIR) based on selected Mode.
- [ ] Task: Conductor - User Manual Verification 'Processing Modes & Advanced Calibration' (Protocol in workflow.md)

## Phase 4: System Verification
- [ ] Task: End-to-End Regression Testing
    - [ ] Run `roomeq` with v2 configs for a 2.1 system (Mode A).
    - [ ] Run `roomeq` with v2 configs for a DBA system (Mode C).
    - [ ] Verify output `dsp_chain` matches the expected filter types for each mode.
- [ ] Task: Conductor - User Manual Verification 'System Verification' (Protocol in workflow.md)
```
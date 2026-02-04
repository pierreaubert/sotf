# Implementation Plan - Refactor and Stabilize Core Audio Engine

## Phase 1: Engine Core Stabilization
- [x] Task: Audit and Fix Device Matching Logic 0900a97
    - [ ] Review prioritization logic in signal_recorder.rs and playback_thread.rs
    - [ ] Add unit tests for device name matching edge cases
- [x] Task: Refine HAL Input and Resampling 8981b32
    - [ ] Validate available_read_frames logic in HalInputReader
    - [ ] Ensure ResamplerPlugin is correctly instantiated and updated during rate changes
- [ ] Task: Conductor - User Manual Verification 'Phase 1: Engine Core Stabilization' (Protocol in workflow.md)

## Phase 2: Integration and Daemon Cleanup
- [ ] Task: Stabilize Playback Thread State Management
    - [ ] Review current changes in playback_thread.rs for thread-safety
    - [ ] Fix any regression in daemon state reporting
- [ ] Task: Conductor - User Manual Verification 'Phase 2: Integration and Daemon Cleanup' (Protocol in workflow.md)

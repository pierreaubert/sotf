# Plan: GPUI Audio Performance Optimization

## Phase 1: Research & Baseline
- [ ] Task: Analyze Threading & Lock Contention
    - [ ] Compare `app-gpui` and `app-tui` engine integration points (message passing vs direct access)
    - [ ] Identify if `gpui` rendering or layout passes are holding locks also needed by the audio engine's processing threads
    - [ ] Use `log` or `tracing` to measure message arrival rates from GPUI UI controls
- [ ] Task: Reproduce & Measure
    - [ ] Create a "stress test" plugin configuration in GPUI (e.g., maxing out Convolution/Upmixer)
    - [ ] Document specific frames/durations of audible glitches during UI interaction (resizing, dragging)
- [ ] Task: Conductor - User Manual Verification 'Phase 1: Research & Baseline' (Protocol in workflow.md)

## Phase 2: Decoupling Visualizer Data Paths
- [ ] Task: TDD - Implement Lock-Free Data Buffers for Visualizers
    - [ ] Write tests for a lock-free "latest frame" buffer or ring buffer to be used for Spectrum Analyzer/Level Meters
    - [ ] Implement the buffer to ensure the audio thread *never* blocks when providing data to the UI
- [ ] Task: Refactor Visualizer Integration
    - [ ] Update `app-gpui`'s visualizer views to pull from the new non-blocking buffers instead of shared Mutex-protected state
    - [ ] Verify visualizer updates do not stall if the UI thread is busy with layout or rendering
- [ ] Task: Conductor - User Manual Verification 'Phase 2: Decoupling Visualizer Data Paths' (Protocol in workflow.md)

## Phase 3: UI Interaction & Message Throttling
- [ ] Task: TDD - Implement Parameter Update Throttler
    - [ ] Write tests for a throttler that limits the rate of parameter messages (e.g., max 60 messages/sec per parameter)
    - [ ] Implement the throttler in the GPUI UI interaction layer (sliders, knobs)
- [ ] Task: Optimize Window Event Handling
    - [ ] Investigate if window resize/move events in GPUI trigger redundant engine reconfiguration or layout cycles
    - [ ] Implement debouncing or passive handling for heavy window events
- [ ] Task: Conductor - User Manual Verification 'Phase 3: UI Interaction & Message Throttling' (Protocol in workflow.md)

## Phase 4: Validation & Parity
- [ ] Task: Stress Testing and Performance Validation
    - [ ] Run GPUI with high-load plugins and visualizers active; verify zero crackling during window resizing
    - [ ] Confirm `app-gpui` performance parity with `app-tui` for the same plugin configurations
- [ ] Task: Final Quality Gates
    - [ ] Verify code follows `code_styleguides/rust.md`
    - [ ] Ensure all new logic has >80% coverage and passing tests
- [ ] Task: Conductor - User Manual Verification 'Phase 4: Validation & Parity' (Protocol in workflow.md)

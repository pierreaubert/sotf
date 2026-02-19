# Plan: GPUI Audio Performance Optimization

## Phase 1: Research & Baseline
- [x] Task: Analyze Threading & Lock Contention [1384efa8]
    - [x] Compare `app-gpui` and `app-tui` engine integration points (message passing vs direct access)
    - [x] Identify if `gpui` rendering or layout passes are holding locks also needed by the audio engine's processing threads
    - [x] Use `log` or `tracing` to measure message arrival rates from GPUI UI controls
- [x] Task: Reproduce & Measure
    - [x] Create a "stress test" plugin configuration in GPUI (e.g., maxing out Convolution/Upmixer)
    - [x] Document specific frames/durations of audible glitches during UI interaction (resizing, dragging)
- [x] Task: Conductor - User Manual Verification 'Phase 1: Research & Baseline' (Protocol in workflow.md)

## Phase 2: Decoupling Visualizer Data Paths
- [x] Task: TDD - Implement Lock-Free Data Buffers for Visualizers [1384efa8]
    - [x] Write tests for a lock-free "latest frame" buffer or ring buffer to be used for Spectrum Analyzer/Level Meters
    - [x] Implement the buffer to ensure the audio thread *never* blocks when providing data to the UI
- [x] Task: Refactor Visualizer Integration
    - [x] Update `app-gpui`'s visualizer views to pull from the new non-blocking buffers instead of shared Mutex-protected state
    - [x] Verify visualizer updates do not stall if the UI thread is busy with layout or rendering
- [x] Task: Conductor - User Manual Verification 'Phase 2: Decoupling Visualizer Data Paths' (Protocol in workflow.md)

## Phase 3: UI Interaction & Message Throttling
- [x] Task: TDD - Implement Parameter Update Throttler [1384efa8]
    - [x] Write tests for a throttler that limits the rate of parameter messages (e.g., max 60 messages/sec per parameter)
    - [x] Implement the throttler in the GPUI UI interaction layer (sliders, knobs)
- [x] Task: Optimize Window Event Handling [1384efa8]
    - [x] Investigate if window resize/move events in GPUI trigger redundant engine reconfiguration or layout cycles
    - [x] Implement debouncing or passive handling for heavy window events
- [x] Task: Conductor - User Manual Verification 'Phase 3: UI Interaction & Message Throttling' (Protocol in workflow.md)

## Phase 4: Validation & Parity
- [x] Task: Stress Testing and Performance Validation [1384efa8]
    - [x] Run GPUI with high-load plugins and visualizers active; verify zero crackling during window resizing
    - [x] Confirm `app-gpui` performance parity with `app-tui` for the same plugin configurations
- [x] Task: Final Quality Gates [1384efa8]
    - [x] Verify code follows `code_styleguides/rust.md`
    - [x] Ensure all new logic has >80% coverage and passing tests
- [x] Task: Conductor - User Manual Verification 'Phase 4: Validation & Parity' (Protocol in workflow.md)

## Phase: Review Fixes
- [x] Task: Apply review suggestions [2aa69f92]

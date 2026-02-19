# Plan: Sync TUI Plugin Parameters and Add Crossfeed

## Phase 1: Shared Metadata Infrastructure
- [ ] Task: Define shared parameter metadata structures in `crates/app-tui/app.rs`
    - [ ] Create `TuiParamType` enum (Float, Int, Bool, Choice).
    - [ ] Create `TuiParamDescriptor` struct (Name, Type, Range, Group).
    - [ ] Implement a `get_descriptors()` method for each plugin type.
- [ ] Task: Audit and define descriptors for all existing plugins
    - [ ] Focus on Upmixer: include all parameters from `UpmixerPluginParams` (30+).
    - [ ] Audit Gain, EQ, Compressor, Limiter, Gate, Expander, etc.
- [ ] Task: Conductor - User Manual Verification 'Phase 1: Shared Metadata' (Protocol in workflow.md)

## Phase 2: Integrate Crossfeed into Backend
- [ ] Task: Add Crossfeed to central plugin definitions
    - [ ] Add `Crossfeed` to `PluginType` and `PluginSettings` in `crates/engine/src/plugins.rs`.
    - [ ] Implement `to_plugin_config` for Crossfeed.
    - [ ] Register "crossfeed" in the plugin factory in `crates/plugins/src/lib.rs`.
- [ ] Task: Implement `TuiEditablePlugin` for Crossfeed in `app.rs`
    - [ ] Define Crossfeed descriptors (Bauer, Meier, Mb, Auto-Gain).
    - [ ] Implement parameter adjustment logic.
- [ ] Task: Conductor - User Manual Verification 'Phase 2: Crossfeed Backend' (Protocol in workflow.md)

## Phase 3: Refactor TUI Logic & UI
- [ ] Task: Refactor `TuiEditablePlugin` implementation in `app.rs`
    - [ ] Update `get_params` to dynamically build `TuiParamSpec` from descriptors.
    - [ ] Update `adjust_param` to use descriptor indexing and validation.
- [ ] Task: Refactor `ui.rs` to support grouping and dynamic parameters
    - [ ] Update `get_plugin_parameters` to use shared descriptors.
    - [ ] Update `draw_plugin_editor_modal` to render group separators.
- [ ] Task: Add Crossfeed to the "Add Plugin" list in the TUI.
- [ ] Task: Conductor - User Manual Verification 'Phase 3: TUI Integration' (Protocol in workflow.md)

## Phase 4: Validation & Parity
- [ ] Task: Verify parameter synchronization
    - [ ] Audit Upmixer in TUI: ensure all parameters update the correct values.
    - [ ] Audit Crossfeed in TUI: verify all modes and auto-gain settings.
- [ ] Task: Run full regression tests for `plugins`, `engine`, and `app-tui`.
- [ ] Task: Conductor - User Manual Verification 'Phase 4: Validation' (Protocol in workflow.md)

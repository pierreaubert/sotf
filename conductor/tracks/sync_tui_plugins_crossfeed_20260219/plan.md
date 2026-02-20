# Plan: Sync TUI Plugin Parameters and Add Crossfeed

## Phase 1: Shared Metadata Infrastructure
- [x] Task: Define shared parameter metadata structures in `crates/app-tui/app.rs` [ac24fe77]
    - [x] Create `TuiParamType` enum (Float, Int, Bool, Choice).
    - [x] Create `TuiParamDescriptor` struct (Name, Type, Range, Group).
    - [x] Implement a `get_descriptors()` method for each plugin type.
- [x] Task: Audit and define descriptors for all existing plugins [ac24fe77]
    - [x] Focus on Upmixer: include all parameters from `UpmixerPluginParams` (30+).
    - [x] Audit Gain, EQ, Compressor, Limiter, Gate, Expander, etc.
- [x] Task: Conductor - User Manual Verification 'Phase 1: Shared Metadata' (Protocol in workflow.md) [ac24fe77]

## Phase 2: Integrate Crossfeed into Backend
- [x] Task: Add Crossfeed to central plugin definitions [f79eb0df]
    - [x] Add `Crossfeed` to `PluginType` and `PluginSettings` in `crates/engine/src/plugins.rs`.
    - [x] Implement `to_plugin_config` for Crossfeed.
    - [x] Register "crossfeed" in the plugin factory in `crates/engine/src/processing_thread.rs`.
- [x] Task: Implement `TuiEditablePlugin` for Crossfeed in `app.rs` [f79eb0df]
    - [x] Define Crossfeed descriptors (Bauer, Meier, Mb, Auto-Gain).
    - [x] Implement parameter adjustment logic.
- [x] Task: Conductor - User Manual Verification 'Phase 2: Crossfeed Backend' (Protocol in workflow.md) [f79eb0df]

## Phase 3: Refactor TUI Logic & UI
- [x] Task: Refactor `TuiEditablePlugin` implementation in `app.rs` [f79eb0df]
    - [x] Update `get_params` to dynamically build `TuiParamSpec` from descriptors.
    - [x] Update `adjust_param` to use descriptor indexing and validation.
- [x] Task: Refactor `ui.rs` to support grouping and dynamic parameters [f79eb0df]
    - [x] Update `get_plugin_parameters` to use shared descriptors.
    - [x] Update `draw_plugin_editor_modal` to render group separators.
- [x] Task: Add Crossfeed to the "Add Plugin" list in the TUI. [f79eb0df]
- [x] Task: Conductor - User Manual Verification 'Phase 3: TUI Integration' (Protocol in workflow.md) [f79eb0df]

## Phase 4: Validation & Parity [checkpoint: 9b78daa]
- [x] Task: Verify parameter synchronization [a9a5f07b]
    - [x] Audit Upmixer in TUI: ensure all parameters update the correct values.
    - [x] Audit Crossfeed in TUI: verify all modes and auto-gain settings.
- [x] Task: Implement EQ filter limit control (Max Filters) [a9a5f07b]
- [x] Task: Run full regression tests for `plugins`, `engine`, and `app-tui`. [a9a5f07b]
- [x] Task: Conductor - User Manual Verification 'Phase 4: Validation' (Protocol in workflow.md) [9b78daa]

# Specification: Sync TUI Plugin Parameters and Add Crossfeed

## Overview
This track addresses a critical synchronization bug in the TUI application where plugin parameter logic and display are out of sync. It also introduces full TUI support for the Crossfeed plugin and ensures all existing plugins (especially the Upmixer) have their complete and correct parameter sets exposed.

## Problem Statement
The TUI app currently defines plugin parameters in two separate locations: `get_params` in `app.rs` (for logic/adjustment) and `get_plugin_parameters` in `ui.rs` (for display). These lists have diverged in order and count, causing index mismatches where adjusting one parameter (e.g., in the Upmixer) updates a different one. Additionally, the Crossfeed plugin is missing from the TUI, and many Upmixer parameters are not accessible.

## Functional Requirements
- **Shared Parameter Metadata:** Implement a data-driven descriptor system (using static arrays/structs) that serves as the single source of truth for parameter names, units, and ranges.
- **Unified Parameter Mapping:** Refactor `app.rs` and `ui.rs` to use this shared metadata, ensuring that the selection index in the UI always maps to the correct parameter in the logic.
- **Add Crossfeed Plugin Support:**
    - Implement `TuiEditablePlugin` for `Crossfeed` settings.
    - Expose controls for Algorithm (Bauer, Meier, Mb) and Preset selection.
    - Expose manual tuning for all Crossfeed parameters (Cutoff, Level, Feed, etc.).
    - Expose Auto-Gain configuration.
- **Update Upmixer Plugin Support:**
    - Audit and expose all missing parameters (e.g., dialogue detection, height transient reduction).
    - Ensure all parameters are correctly grouped with visual separators in the TUI editor.
- **Parameter Validation:** Ensure all TUI adjustments respect the minimum/maximum bounds defined in the shared metadata or `param_specs`.

## Non-Functional Requirements
- **UI Clarity:** Use section headers/separators in the TUI editor to handle plugins with high parameter counts (Upmixer).
- **Maintainability:** Ensure that adding a new parameter only requires updating the shared metadata, with the UI and logic automatically staying in sync.

## Acceptance Criteria
- Adjusting any parameter in any plugin correctly updates the intended value.
- The Crossfeed plugin is available in the "Available Plugins" list and fully editable.
- The Upmixer plugin exposes its full set of 30+ parameters, organized into logical groups (Front, Surround, LFE, etc.).
- No audible glitches or incorrect value jumps during TUI parameter interaction.

## Out of Scope
- Redesigning the entire TUI navigation system.
- Implementing graphical visualizations for the TUI plugins (e.g., EQ curves).

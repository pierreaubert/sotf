# Phase 6: Architectural Refactoring Plan

## Problem Statement

The current `App` struct in `app/state/app.rs` has **~130 fields** and **1099 lines**, making it a "god object" that:

1. **Cannot be unit tested** - Every test needs to construct/mock the entire state
2. **Has hidden dependencies** - Fields interact in undocumented ways
3. **Encourages bugs** - Easy to miss state updates across related fields
4. **Prevents isolation** - Changes ripple across the entire codebase

## Current State (as of 2026-01-09)

### Existing Infrastructure

The following state structs **already exist** but are **not yet used**:

| State Struct | File | Fields | Status |
|-------------|------|--------|--------|
| `PlaybackState` | `app/state/playback.rs` | 11 | Created, duplicated in App |
| `LibraryState` | `app/state/library.rs` | 10 | Created with methods, duplicated in App |
| `PluginState` | `app/state/plugin.rs` | 19 | Created, duplicated in App |
| `UIState` | `app/state/ui.rs` | 22 | Created, duplicated in App |

The `App` struct has these composed structs instantiated at lines 324-330:

```rust
pub playback: PlaybackState,
pub library_state: LibraryState,
pub plugin_state: PluginState,
pub ui_state: UIState,
```

**Problem**: All fields are duplicated - they exist both on `App` directly AND inside the composed structs. The composed structs are instantiated but never used.

### Missing Managers

- `InputManager` - not created yet
- `RecordingManager` - not created yet
- `AudioDeviceManager` - not mentioned but needed

## Current Architecture

```
App (1099 lines, ~130 fields)
├── Playback state (volume, position, queue, etc.) - DUPLICATED in PlaybackState
├── Library state (albums, filters, search, pagination) - DUPLICATED in LibraryState  
├── Plugin state (chain, graph, canvas, parameters) - DUPLICATED in PluginState
├── Input state (mode, focus, keybindings) - NOT extracted
├── UI state (theme, layout, window, settings tab) - DUPLICATED in UIState
├── Audio state (engine, devices, stream info) - NOT extracted
└── Recording state (channels, analysis, progress) - NOT extracted
```

All fields are directly on `App`, accessed via `app.field_name`, with no encapsulation.

## Proposed Architecture

Break the god object into focused managers with clear boundaries:

```
AppContext
├── PlaybackManager (~30 fields)
│   ├── volume, muted
│   ├── position, duration
│   ├── queue, current_index
│   ├── playback_state (enum)
│   └── audio_engine handle
│
├── LibraryManager (~40 fields)
│   ├── albums, library_path
│   ├── search_query
│   ├── filters (genre, decade, year, artist)
│   ├── sort_order, pagination
│   └── fts_index
│
├── PluginManager (~50 fields)
│   ├── plugin_chain
│   ├── plugin_graph (node connections)
│   ├── canvas_state (positions, zoom)
│   ├── selected_plugin, edit_mode
│   └── automation state
│
├── InputManager (~15 fields)
│   ├── input_mode (enum)
│   ├── focus_handle
│   ├── text_buffer
│   └── pending_actions
│
├── UIManager (~20 fields)
│   ├── theme, colors
│   ├── layout_mode
│   ├── window_state
│   └── settings_tab
│
└── RecordingManager (~25 fields)
    ├── recording_state
    ├── channel_configs
    ├── analysis_results
    └── progress
```

## Benefits

### 1. Testable Units

Each manager can be tested in isolation:

```rust
#[test]
fn test_playback_volume_preserved() {
    let mut pm = PlaybackManager::default();
    pm.set_volume(0.5);
    pm.next_track();
    assert_eq!(pm.volume(), 0.5);
}
```

### 2. Clear Ownership

Methods live on the struct that owns the data:

```rust
// Before (god object)
impl App {
    fn next_track(&mut self) { /* touches 20 fields */ }
    fn filter_albums(&self) { /* touches 15 fields */ }
    fn update_plugin(&mut self) { /* touches 30 fields */ }
}

// After (focused managers)
impl PlaybackManager {
    fn next_track(&mut self) { /* touches only playback fields */ }
}

impl LibraryManager {
    fn filter_albums(&self) { /* touches only library fields */ }
}
```

### 3. Explicit Dependencies

Managers communicate via explicit messages:

```rust
enum AppMessage {
    Playback(PlaybackMessage),
    Library(LibraryMessage),
    Plugin(PluginMessage),
    Input(InputMessage),
}

enum PlaybackMessage {
    Play,
    Pause,
    NextTrack,
    SetVolume(f32),
    Seek(f64),
}
```

### 4. State Machine Clarity

Each manager can implement a clear state machine:

```rust
impl PlaybackManager {
    fn transition(&mut self, event: PlaybackEvent) -> Result<(), InvalidTransition> {
        match (self.state, event) {
            (PlaybackState::Playing, PlaybackEvent::Pause) => {
                self.state = PlaybackState::Paused;
                Ok(())
            }
            // ... explicit transitions
        }
    }
}
```

## Migration Strategy

### Phase 6.0: Audit and Cleanup (PREREQUISITE)

Before migrating, we need to understand actual field usage:

1. **Audit field access patterns** - grep for `app.field_name` across codebase
2. **Remove duplicate fields** - delete fields from `App` that exist in composed structs
3. **Verify composed structs are complete** - ensure all related fields are in the right struct

### Phase 6.1: Migrate PlaybackState (Low Risk)

`PlaybackState` already exists at `app/state/playback.rs`. Migration steps:

1. **Remove duplicate fields from App**:
   - `is_playing`, `current_queue_index`, `volume`, `muted`
   - `position_secs`, `duration_secs`
   - `input_loudness_info`, `loudness_info`, `spectrum_info`, `compressor_info`
2. **Update all callers**: `app.volume` → `app.playback.volume`
3. **Add methods to PlaybackState** for common operations
4. **Update tests** to use `PlaybackState` directly

### Phase 6.2: Migrate LibraryState (Medium Risk)

`LibraryState` already exists with methods at `app/state/library.rs`. Migration steps:

1. **Remove duplicate fields from App**:
   - `library`, `library_stats`, `library_scanner`
   - `search_query`, `selected_album_index`, `album_list_offset`
   - `library_sort_order`, `channel_filter`, `selected_genre`, etc.
   - `library_items_per_page`, `library_columns`
2. **Update all callers**: `app.library` → `app.library_state.library`
3. **Handle queue interaction** - LibraryState needs to communicate with queue

### Phase 6.3: Create InputManager (Low Risk)

Input state needs to be extracted (doesn't exist yet):

1. **Create `app/state/input.rs`** with:
   - `input_mode`, `directory_input`, `plugin_file_input`
   - `apo_file_input`, `sofa_file_input`
   - `autocomplete_suggestions`, `autocomplete_index`
   - `editing_param`, `editing_value`
2. **Move text input methods**
3. **Update callers**

### Phase 6.4: Migrate PluginState (High Risk)

`PluginState` already exists at `app/state/plugin.rs`. Migration steps:

1. **Remove duplicate fields from App**:
   - `plugin_chain`, `plugin_chain_modified`, `pending_plugin_update`
   - `editing_plugin_index`, `plugin_param_selection`, `selected_eq_band`
   - `matrix_selected_cell`, `plugin_view_mode`, `plugin_graph`
   - `graph_selection`, `graph_connection_drag`, `graph_node_drag`
   - `workflow_canvas`, `workflow_node_mapping`, `editing_plugin_node`
   - `available_plugin_presets`, `selected_preset_index`, `last_loaded_preset`
2. **Handle audio engine sync** - plugin updates need to reach the Player
3. **Update all callers**

### Phase 6.5: Migrate UIState and Create RecordingManager

`UIState` exists at `app/state/ui.rs`. Also create `RecordingManager`:

1. **Remove duplicate UI fields from App**
2. **Create `app/state/recording.rs`** for:
   - `recording_state`, `measure_state`
   - `room_eq_state`, `room_eq_applied_plugins`
   - `headphone_eq_state`, `spinorama_eq_state`
3. **Create `app/state/audio_device.rs`** for:
   - `output_devices`, `input_devices`
   - `selected_output_device_index`, `selected_input_device_index`
   - `current_output_device_name`, `current_input_device_name`
   - `playback_source`

## Interface Design

### Manager Protocol

Each manager implements a standard protocol:

```rust
trait Manager {
    type State;
    type Event;
    type Query;
    type Response;

    fn handle_event(&mut self, event: Self::Event) -> Result<(), ManagerError>;
    fn query(&self, query: Self::Query) -> Self::Response;
    fn state(&self) -> &Self::State;
}
```

### Cross-Manager Communication

Managers don't call each other directly. They send messages through AppContext:

```rust
impl AppContext {
    fn dispatch(&mut self, msg: AppMessage) {
        match msg {
            AppMessage::Playback(pm) => {
                self.playback.handle_event(pm);
                // May trigger follow-up messages
            }
            // ...
        }
    }
}
```

### Shared State

Some state needs to be shared (e.g., current track info):

```rust
struct SharedState {
    current_track: Option<Arc<TrackInfo>>,
    audio_device: Arc<AudioDevice>,
}

// Managers receive read-only access
impl PlaybackManager {
    fn new(shared: Arc<SharedState>) -> Self { ... }
}
```

## Testing After Refactor

With managers extracted, testing becomes straightforward:

```rust
// Unit test a single manager
#[test]
fn test_library_search() {
    let mut lm = LibraryManager::with_albums(test_albums());
    lm.search("jazz");
    assert_eq!(lm.filtered_albums().len(), 3);
}

// Integration test manager interactions
#[test]
fn test_play_from_search() {
    let mut ctx = TestAppContext::new();
    ctx.dispatch(LibraryMessage::Search("jazz".into()));
    ctx.dispatch(LibraryMessage::PlayAlbum(0));

    assert!(ctx.playback.is_playing());
    assert!(ctx.playback.current_track().title.contains("Jazz"));
}
```

## Risk Mitigation

1. **Feature flags**: Hide refactored code behind feature flags initially
2. **Parallel implementation**: Keep old code working while building new
3. **Incremental migration**: One manager at a time, with full test coverage
4. **Compatibility layer**: Temporary `impl App` that delegates to managers

## Estimated Effort

| Phase | Manager | Fields | Effort | Risk | Notes |
|-------|---------|--------|--------|------|-------|
| 6.0 | Audit/Cleanup | - | 1 day | Low | Prerequisite |
| 6.1 | PlaybackState | 11 | 1-2 days | Low | Struct exists, need caller migration |
| 6.2 | LibraryState | 10+ | 2-3 days | Medium | Struct exists with methods |
| 6.3 | InputManager | ~10 | 1 day | Low | New struct needed |
| 6.4 | PluginState | 19 | 3-4 days | High | Struct exists, audio engine coupling |
| 6.5 | UI/Recording/Device | ~30 | 2-3 days | Medium | UIState exists, others new |

**Total**: 1.5-2 weeks for full refactor

*Note: Effort reduced from original estimate because structs already exist. Main work is caller migration.*

## Success Criteria

1. Each manager can be instantiated and tested independently
2. No direct field access across manager boundaries
3. All existing tests pass (or are updated to test managers directly)
4. New bugs can be traced to specific managers
5. Adding features requires changes to only 1-2 managers

## Next Steps

1. ✅ Review this plan (completed 2026-01-09)
2. **Phase 6.0**: Audit field usage with `grep` to identify all callers
3. **Phase 6.1**: Start with PlaybackState migration
   - Remove duplicate fields from `App`
   - Update all `app.volume` → `app.playback.volume` etc.
   - Add unit tests for `PlaybackState`
4. Continue with remaining phases in order
5. Add integration tests for cross-manager communication

## Field Mapping Reference

### PlaybackState (app/state/playback.rs)

```rust
is_playing, current_queue_index, volume, muted,
position_secs, duration_secs,
input_loudness_info, loudness_info, spectrum_info, compressor_info
```

### LibraryState (app/state/library.rs)

```rust
library, sort_order, filter, search_query,
selected_index, current_page, items_per_page,
scan_in_progress, scan_progress_tracks, scan_progress_albums
```

### PluginState (app/state/plugin.rs)

```rust
plugin_chain, plugin_chain_modified, pending_plugin_update,
editing_plugin_index, plugin_param_selection, selected_eq_band,
matrix_selected_cell, plugin_view_mode, plugin_graph,
graph_selection, graph_connection_drag, graph_node_drag,
workflow_canvas, workflow_node_mapping, editing_plugin_node,
available_plugin_presets, selected_preset_index, last_loaded_preset
```

### UIState (app/state/ui.rs)

```rust
current_screen, last_screen, input_mode, active_menu,
layout_mode, window_height, window_width,
theme_id, theme, language, translations, keymap_preset,
toast_message, context_menu, active_settings_tab,
filter_menu_open, show_device_popup, show_studio_menu,
pending_studio_close, should_quit, startup_db_check_done
```

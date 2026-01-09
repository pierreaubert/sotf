# Phase 6: Architectural Refactoring Plan

## Problem Statement

The current `App` struct in `app/state/app.rs` has **300+ fields** and **1099 lines**, making it a "god object" that:

1. **Cannot be unit tested** - Every test needs to construct/mock the entire state
2. **Has hidden dependencies** - Fields interact in undocumented ways
3. **Encourages bugs** - Easy to miss state updates across related fields
4. **Prevents isolation** - Changes ripple across the entire codebase

## Current Architecture

```
App (1099 lines, 300+ fields)
├── Playback state (volume, position, queue, etc.)
├── Library state (albums, filters, search, pagination)
├── Plugin state (chain, graph, canvas, parameters)
├── Input state (mode, focus, keybindings)
├── UI state (theme, layout, window, settings tab)
├── Audio state (engine, devices, stream info)
└── Recording state (channels, analysis, progress)
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

### Phase 6.1: Extract PlaybackManager (Low Risk)

Playback state has the clearest boundaries. Start here.

1. Create `app/managers/playback.rs`
2. Move fields: `volume`, `muted`, `position`, `duration`, `queue`, `current_queue_index`, `is_playing`, `playback_state`
3. Move methods: `play()`, `pause()`, `next_track()`, `prev_track()`, `seek()`, `set_volume()`
4. Replace `app.volume` with `app.playback.volume()` throughout codebase
5. Update tests to use `PlaybackManager` directly

### Phase 6.2: Extract LibraryManager (Medium Risk)

Library state is well-defined but has more touchpoints.

1. Create `app/managers/library.rs`
2. Move fields: `albums`, `search_query`, `selected_*`, `library_*`
3. Move methods: `filtered_albums()`, `search()`, `apply_filter()`, pagination methods
4. Handle interaction with PlaybackManager (loading albums to queue)

### Phase 6.3: Extract InputManager (Low Risk)

Input state is small and isolated.

1. Create `app/managers/input.rs`
2. Move fields: `input_mode`, text buffers, focus handles
3. Move methods: `enter_input_mode()`, `exit_input_mode()`, `process_key()`

### Phase 6.4: Extract PluginManager (High Risk)

Plugin state is complex with canvas/graph interactions.

1. Create `app/managers/plugin.rs`
2. Move fields: `plugin_chain`, `plugin_graph`, `canvas_*`, `selected_plugin`
3. Move methods: plugin manipulation, graph operations, canvas interactions
4. Carefully handle audio engine integration

### Phase 6.5: Extract UIManager and RecordingManager

Final cleanup for remaining state.

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

| Phase | Manager | Fields | Effort | Risk |
|-------|---------|--------|--------|------|
| 6.1 | PlaybackManager | ~30 | 2-3 days | Low |
| 6.2 | LibraryManager | ~40 | 3-4 days | Medium |
| 6.3 | InputManager | ~15 | 1-2 days | Low |
| 6.4 | PluginManager | ~50 | 4-5 days | High |
| 6.5 | UI/Recording | ~45 | 3-4 days | Medium |

**Total**: 2-3 weeks for full refactor

## Success Criteria

1. Each manager can be instantiated and tested independently
2. No direct field access across manager boundaries
3. All existing tests pass (or are updated to test managers directly)
4. New bugs can be traced to specific managers
5. Adding features requires changes to only 1-2 managers

## Next Steps

1. Review this plan with team
2. Create `app/managers/` directory structure
3. Start with Phase 6.1 (PlaybackManager)
4. Write comprehensive tests for each extracted manager
5. Gradually migrate callers from `app.field` to `app.manager.method()`

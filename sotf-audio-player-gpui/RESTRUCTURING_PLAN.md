# SOTF GPUI Audio Player - Restructuring Plan

## Executive Summary

This document outlines a plan to restructure the GPUI audio player for:
1. Better adherence to GPUI best practices
2. Improved performance for long lists (albums, queue)
3. Low-latency GPU-accelerated audio visualization
4. Comprehensive testing strategy

---

## Part 1: Current Architecture Analysis

### Issues Identified

#### 1.1 Monolithic State (`app/state.rs`)
- **Problem**: `App` struct has 100+ fields covering UI state, playback state, library state, plugin state
- **Impact**: Hard to reason about, all fields mutated from many places
- **GPUI Violation**: Should use separate `Entity<T>` for independent state domains

#### 1.2 Monolithic UI (`ui.rs`)
- **Problem**: 1600+ lines, `PlayerView` handles all screen rendering and 50+ action handlers
- **Impact**: Long compile times, hard to navigate, no component isolation
- **GPUI Violation**: Should compose smaller `Render` components

#### 1.3 Business Logic in UI Layer
- **Problem**: `level_meters.rs` implements `App` methods in `ui/components/plugins/`
- **Impact**: Circular dependencies, unclear module boundaries

#### 1.4 List Rendering Without Virtualization
- **Problem**: `render_library_grid()` creates elements for all visible albums
- **Impact**: Performance degrades with large libraries (1000+ albums)
- **GPUI Violation**: Should use `uniform_list` or `list` for virtualization

#### 1.5 Polling-Based Updates
- **Problem**: 100ms timer polls for playback state, loudness, spectrum
- **Impact**: Unnecessary CPU usage, delayed UI updates
- **GPUI Violation**: Should use `cx.observe()` and `cx.subscribe()` patterns

#### 1.6 No GPU Acceleration for Audio Widgets
- **Problem**: Spectrum analyzer and level meters use standard `div()` elements
- **Impact**: Limited frame rate, no smooth animations for real-time data
- **GPUI Opportunity**: Can use custom `Element` with GPU paths for rendering

---

## Part 2: Proposed Architecture

### 2.1 State Domain Separation

Split `App` into focused entities:

```
┌─────────────────────────────────────────────────────────────┐
│                        AppState                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │ LibraryState│  │ PlayerState │  │ PluginState │         │
│  │ - albums    │  │ - is_playing│  │ - chain     │         │
│  │ - filter    │  │ - position  │  │ - editing   │         │
│  │ - sort      │  │ - volume    │  │ - presets   │         │
│  └─────────────┘  └─────────────┘  └─────────────┘         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │  QueueState │  │  UIState    │  │ AudioMeters │         │
│  │ - items     │  │ - screen    │  │ - levels    │         │
│  │ - current   │  │ - input_mode│  │ - spectrum  │         │
│  │ - expanded  │  │ - theme     │  │ - loudness  │         │
│  └─────────────┘  └─────────────┘  └─────────────┘         │
└─────────────────────────────────────────────────────────────┘
```

**Implementation**:
```rust
// New structure
pub struct AppState {
    pub library: Entity<LibraryState>,
    pub player: Entity<PlayerState>,
    pub queue: Entity<QueueState>,
    pub plugins: Entity<PluginState>,
    pub ui: Entity<UIState>,
    pub meters: Entity<AudioMeterState>,
}

// Each domain is an independent Entity
pub struct LibraryState {
    library: MusicLibrary,
    view_mode: LibraryViewMode,
    sort_order: LibrarySortOrder,
    filter: ChannelFilter,
    search_query: String,
    selected_index: usize,
    // ...
}

impl EventEmitter<LibraryEvent> for LibraryState {}

pub enum LibraryEvent {
    AlbumsChanged,
    SelectionChanged,
    FilterChanged,
}
```

### 2.2 Component Hierarchy

```
PlayerView (root)
├── MenuBar (RenderOnce)
├── Header (RenderOnce)
│   └── PlaybackControls (RenderOnce)
├── MainContent
│   ├── LibraryScreen (Render)
│   │   ├── LibraryToolbar (RenderOnce)
│   │   ├── LibraryList (Render + uniform_list)  ← NEW
│   │   └── Pagination (RenderOnce)
│   ├── QueueScreen (Render)
│   │   └── QueueList (Render + uniform_list)  ← NEW
│   ├── SettingsScreen (Render)
│   │   ├── PluginRack (Render)
│   │   └── DeviceSelector (RenderOnce)
│   └── SpectrumScreen (Render)
│       └── SpectrumVisualizer (custom Element)  ← NEW
├── Footer (RenderOnce)
│   └── LevelMeterGroup (custom Element)  ← NEW
└── Overlays
    ├── ContextMenu (Render)
    ├── HelpModal (RenderOnce)
    └── ToastContainer (RenderOnce)
```

### 2.3 File Structure

```
sotf-audio-player-gpui/src/
├── lib.rs
├── main.rs
├── actions.rs
├── config.rs
├── keybindings.rs
├── theme.rs
├── i18n/
│   ├── mod.rs
│   └── translations.rs
├── state/                    # NEW: Domain-separated state
│   ├── mod.rs
│   ├── library.rs           # LibraryState + impl
│   ├── player.rs            # PlayerState + impl
│   ├── queue.rs             # QueueState + impl
│   ├── plugins.rs           # PluginState + impl
│   ├── ui.rs                # UIState + impl
│   └── meters.rs            # AudioMeterState + impl
├── components/              # NEW: Reusable components
│   ├── mod.rs
│   ├── button.rs
│   ├── list_item.rs
│   ├── modal.rs
│   ├── toast.rs
│   └── ...
├── screens/                 # Screen-level components
│   ├── mod.rs
│   ├── library/
│   │   ├── mod.rs
│   │   ├── library_screen.rs
│   │   ├── library_list.rs  # uniform_list based
│   │   └── album_card.rs
│   ├── queue/
│   │   ├── mod.rs
│   │   └── queue_screen.rs
│   ├── settings/
│   │   ├── mod.rs
│   │   └── plugin_rack.rs
│   └── spectrum/
│       ├── mod.rs
│       └── spectrum_visualizer.rs  # custom Element
├── elements/                # NEW: Custom GPU elements
│   ├── mod.rs
│   ├── level_meter.rs      # GPU-accelerated meter
│   ├── spectrum_bars.rs    # GPU-accelerated spectrum
│   └── eq_curve.rs         # GPU-accelerated EQ graph
└── views/
    ├── mod.rs
    └── player_view.rs      # Slimmed down root view
```

---

## Part 3: Performance - Long Lists

### 3.1 Problem

Current `render_library_grid()` creates elements for all visible albums:
```rust
.children(albums.iter().enumerate().map(|(idx, album)| {
    // Creates div() for every album on every render
}))
```

### 3.2 Solution: uniform_list

Use GPUI's `uniform_list` for virtualized rendering:

```rust
pub struct LibraryList {
    albums: Entity<LibraryState>,
    scroll_handle: UniformListScrollHandle,
}

impl Render for LibraryList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let albums = self.albums.read(cx);
        let count = albums.filtered_albums().len();

        uniform_list(
            "album-list",
            count,
            cx.listener(|this, range, window, cx| {
                // Only render items in visible range
                this.render_albums_in_range(range, window, cx)
            })
        )
        .track_scroll(&self.scroll_handle)
        .size_full()
    }
}

impl LibraryList {
    fn render_albums_in_range(
        &self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>
    ) -> Vec<impl IntoElement> {
        let albums = self.albums.read(cx);
        let filtered = albums.filtered_albums();

        range.map(|idx| {
            AlbumCard::new(
                filtered[idx].clone(),
                idx,
                albums.selected_index == idx,
            )
        }).collect()
    }
}
```

### 3.3 Album Image Caching

Add image caching to prevent reloading:

```rust
pub struct ImageCache {
    cache: HashMap<PathBuf, gpui::ImageData>,
    pending: HashSet<PathBuf>,
}

impl ImageCache {
    pub fn get_or_load(&mut self, path: &Path, cx: &mut App) -> Option<gpui::ImageData> {
        if let Some(data) = self.cache.get(path) {
            return Some(data.clone());
        }

        if !self.pending.contains(path) {
            self.pending.insert(path.to_path_buf());
            let path = path.to_path_buf();
            cx.background_spawn(async move {
                // Load and decode image
            }).detach();
        }

        None // Return placeholder
    }
}
```

### 3.4 Expected Performance Gains

| Scenario | Current | With uniform_list |
|----------|---------|-------------------|
| 100 albums | 50ms render | 5ms render |
| 1000 albums | 500ms render | 5ms render |
| 10000 albums | 5s render | 5ms render |
| Scroll performance | Jerky | Smooth 60fps |
| Memory usage | O(n) | O(visible) |

---

## Part 4: Performance - Audio Visualization

### 4.1 Problem

Current spectrum and level meters:
- Use standard `div()` elements with `bg()` color
- Recreate element tree on every 100ms update
- No smooth interpolation between values
- Limited to CSS-based styling

### 4.2 Solution: Custom GPU Elements

Create custom `Element` implementations for direct GPU rendering:

```rust
// Level meter custom element
pub struct LevelMeterElement {
    level_db: f32,
    peak_db: f32,
    channel_name: SharedString,
    is_clipping: bool,
}

impl Element for LevelMeterElement {
    type RequestLayoutState = ();
    type PrepaintState = Bounds<Pixels>;

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        cx: &mut WindowContext,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = cx.request_layout(
            Style {
                size: size(px(20.0), relative(1.0)),
                ..Default::default()
            },
            [],
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        cx: &mut WindowContext,
    ) -> Self::PrepaintState {
        bounds
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        bounds: Self::PrepaintState,
        _: &mut Self::RequestLayoutState,
        cx: &mut WindowContext,
    ) {
        // Direct GPU path drawing
        let level_height = bounds.size.height * self.level_to_height();

        // Background
        cx.paint_quad(fill(bounds, theme.surface));

        // Level bar with gradient
        let level_bounds = Bounds {
            origin: point(bounds.origin.x, bounds.bottom() - level_height),
            size: size(bounds.size.width, level_height),
        };

        // Use gradient for meter color
        cx.paint_quad(fill_gradient(
            level_bounds,
            self.get_gradient(), // Green → Yellow → Red
        ));

        // Peak indicator line
        let peak_y = bounds.bottom() - bounds.size.height * self.peak_to_height();
        cx.paint_line(
            point(bounds.origin.x, peak_y),
            point(bounds.right(), peak_y),
            px(2.0),
            if self.is_clipping { rgb(0xff0000) } else { rgb(0xffffff) },
        );
    }
}
```

### 4.3 Spectrum Analyzer with WebGPU

For high-performance spectrum rendering:

```rust
pub struct SpectrumElement {
    magnitudes: Arc<[f32]>,  // Shared reference to avoid copying
    colors: [Rgba; 3],       // Low, mid, high colors
    smoothing: f32,
    previous_magnitudes: Vec<f32>,
}

impl Element for SpectrumElement {
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        cx: &mut WindowContext,
    ) {
        let bar_count = self.magnitudes.len();
        let bar_width = bounds.size.width / bar_count as f32;

        for (i, &mag) in self.magnitudes.iter().enumerate() {
            // Apply smoothing for animation
            let smoothed = self.previous_magnitudes.get(i)
                .map(|&prev| prev * self.smoothing + mag * (1.0 - self.smoothing))
                .unwrap_or(mag);

            let height = bounds.size.height * self.db_to_height(smoothed);
            let bar_bounds = Bounds {
                origin: point(
                    bounds.origin.x + px(i as f32 * bar_width),
                    bounds.bottom() - height,
                ),
                size: size(px(bar_width - 1.0), height),
            };

            // Color based on frequency band
            let color = self.freq_to_color(i, bar_count);
            cx.paint_quad(fill(bar_bounds, color));
        }
    }
}
```

### 4.4 Update Strategy: Push vs Poll

Replace polling with event-driven updates:

```rust
impl AudioMeterState {
    pub fn update_from_engine(&mut self, data: &PlaybackData, cx: &mut Context<Self>) {
        self.levels = data.channel_peaks.clone();
        self.spectrum = data.spectrum.clone();
        self.loudness = data.loudness;
        cx.notify(); // Triggers re-render
    }
}

// In engine thread, send updates via channel
impl AudioEngine {
    fn on_frame_processed(&self, data: PlaybackData) {
        // Send to UI thread
        self.meter_tx.send(data).ok();
    }
}

// In PlayerView setup
cx.spawn(async move |view, cx| {
    while let Ok(data) = meter_rx.recv_async().await {
        view.update_in(&mut cx, |view, window, cx| {
            view.meters.update(cx, |meters, cx| {
                meters.update_from_engine(&data, cx);
            });
        }).ok();
    }
}).detach();
```

### 4.5 Expected Performance Gains

| Metric | Current | With GPU Elements |
|--------|---------|-------------------|
| Render time | 10-20ms | <2ms |
| Frame rate | ~30fps | 60fps smooth |
| CPU usage | High | Minimal |
| Latency | 100ms | 16ms (one frame) |
| Animation | Stepped | Smooth interpolation |

---

## Part 5: Testing Strategy

### 5.1 Test Categories

```
┌─────────────────────────────────────────────────────────────┐
│                     Test Pyramid                             │
│                                                              │
│                      ┌──────┐                                │
│                     /  E2E  \                                │
│                    /  Tests  \                               │
│                   ├──────────┤                               │
│                  /  Visual    \                              │
│                 /   Regression \                             │
│                ├────────────────┤                            │
│               /   Integration    \                           │
│              /     Tests          \                          │
│             ├──────────────────────┤                         │
│            /      Component         \                        │
│           /        Tests             \                       │
│          ├────────────────────────────┤                      │
│         /         Unit Tests           \                     │
│        /          (State Logic)         \                    │
│       └──────────────────────────────────┘                   │
└─────────────────────────────────────────────────────────────┘
```

### 5.2 Unit Tests (State Logic)

Test pure state transformations without GPUI:

```rust
// tests/unit/library_state_tests.rs
#[test]
fn test_library_filter_by_channel_count() {
    let mut state = LibraryState::new();
    state.add_album(Album::stereo("Album 1"));
    state.add_album(Album::multichannel("Album 2", 6));

    state.set_filter(ChannelFilter::Stereo);
    assert_eq!(state.filtered_albums().len(), 1);

    state.set_filter(ChannelFilter::Multichannel);
    assert_eq!(state.filtered_albums().len(), 1);

    state.set_filter(ChannelFilter::All);
    assert_eq!(state.filtered_albums().len(), 2);
}

#[test]
fn test_library_sort_by_artist() {
    let mut state = LibraryState::new();
    state.add_album(Album::new("Album Z", "Artist A"));
    state.add_album(Album::new("Album A", "Artist Z"));

    state.set_sort_order(LibrarySortOrder::Artist);
    let albums = state.sorted_albums();
    assert_eq!(albums[0].artist, "Artist A");
    assert_eq!(albums[1].artist, "Artist Z");
}

#[test]
fn test_queue_advance_on_track_end() {
    let mut state = QueueState::new();
    state.add_album(test_album_3_tracks());
    state.start_playback();

    assert_eq!(state.current_track_index(), Some(0));
    state.advance_to_next();
    assert_eq!(state.current_track_index(), Some(1));
    state.advance_to_next();
    assert_eq!(state.current_track_index(), Some(2));
    state.advance_to_next();
    assert_eq!(state.current_track_index(), None); // Queue finished
}
```

### 5.3 Component Tests (GPUI Rendering)

Test components render correctly:

```rust
// tests/component/album_card_tests.rs
use gpui::TestAppContext;

#[gpui::test]
fn test_album_card_displays_title(cx: &mut TestAppContext) {
    let album = Album::new("Test Album", "Test Artist");

    cx.update(|cx| {
        let element = AlbumCard::new(album.clone(), 0, false);

        // Verify element contains expected text
        let rendered = element.render_test(cx);
        assert!(rendered.contains_text("Test Album"));
        assert!(rendered.contains_text("Test Artist"));
    });
}

#[gpui::test]
fn test_album_card_selected_state(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let normal = AlbumCard::new(test_album(), 0, false);
        let selected = AlbumCard::new(test_album(), 0, true);

        // Selected card should have accent background
        assert!(selected.has_style("background", theme.accent));
        assert!(normal.has_style("background", theme.surface));
    });
}
```

### 5.4 Integration Tests

Test component interactions:

```rust
// tests/integration/library_interaction_tests.rs
#[gpui::test]
async fn test_search_filters_library(cx: &mut TestAppContext) {
    let app = TestApp::new(cx);

    // Add test albums
    app.library().update(cx, |lib, _| {
        lib.add_album(Album::new("Pink Floyd - DSOTM", "Pink Floyd"));
        lib.add_album(Album::new("Beatles - Abbey Road", "Beatles"));
    });

    // Simulate search input
    app.dispatch_action(ToggleSearch);
    app.simulate_typing("Pink");

    // Verify filtered results
    app.library().read(cx, |lib, _| {
        let filtered = lib.filtered_albums();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "Pink Floyd - DSOTM");
    });
}

#[gpui::test]
async fn test_play_album_from_library(cx: &mut TestAppContext) {
    let app = TestApp::new(cx);
    app.add_test_album();

    // Select and play
    app.dispatch_action(SelectNext);
    app.dispatch_action(Enter);

    // Verify queue updated
    app.queue().read(cx, |queue, _| {
        assert_eq!(queue.items().len(), 1);
        assert!(queue.is_playing());
    });
}
```

### 5.5 Visual Regression Tests

Capture and compare screenshots:

```rust
// tests/visual/library_screen_tests.rs
#[gpui::test]
fn test_library_grid_visual(cx: &mut TestAppContext) {
    let app = TestApp::new(cx);
    app.add_sample_albums(12);

    // Render and capture
    let screenshot = cx.capture_screenshot("library-grid");

    // Compare with baseline
    assert_visual_match!(screenshot, "baseline/library-grid.png", threshold: 0.01);
}

#[gpui::test]
fn test_level_meters_visual(cx: &mut TestAppContext) {
    let app = TestApp::new(cx);
    app.meters().update(cx, |m, _| {
        m.set_levels(&[-20.0, -15.0, -10.0, -5.0, 0.0]);
    });

    let screenshot = cx.capture_screenshot("level-meters");
    assert_visual_match!(screenshot, "baseline/level-meters.png");
}
```

### 5.6 Performance Tests

Benchmark critical paths:

```rust
// benches/library_rendering.rs
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_library_grid_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("library");

    for album_count in [100, 1000, 10000] {
        group.bench_function(
            format!("grid_{}albums", album_count),
            |b| {
                let app = setup_app_with_albums(album_count);
                b.iter(|| {
                    app.render_library_grid()
                });
            }
        );
    }
}

fn bench_spectrum_update(c: &mut Criterion) {
    c.bench_function("spectrum_60fps", |b| {
        let element = SpectrumElement::new(vec![0.0; 64]);
        b.iter(|| {
            element.paint_test();
        });
    });
}

criterion_group!(benches, bench_library_grid_render, bench_spectrum_update);
criterion_main!(benches);
```

### 5.7 Test Infrastructure

Add test helpers:

```rust
// tests/common/mod.rs
pub struct TestApp {
    state: Entity<AppState>,
    window: WindowHandle<PlayerView>,
}

impl TestApp {
    pub fn new(cx: &mut TestAppContext) -> Self {
        let state = cx.new(|_| AppState::new_for_test());
        let window = cx.open_window(|_, cx| {
            cx.new(|cx| PlayerView::new(state.clone(), cx))
        });
        Self { state, window }
    }

    pub fn dispatch_action<A: Action>(&self, action: A) {
        self.window.dispatch_action(action);
    }

    pub fn library(&self) -> &Entity<LibraryState> {
        &self.state.read(cx).library
    }

    pub fn simulate_typing(&self, text: &str) {
        for c in text.chars() {
            self.window.dispatch_keystroke(Keystroke::from_char(c));
        }
    }
}
```

### 5.8 Test Matrix

| Test Type | Count | Coverage |
|-----------|-------|----------|
| Unit (State) | 80+ | State logic |
| Component | 40+ | UI rendering |
| Integration | 20+ | Cross-component |
| Visual | 15+ | Screenshot comparison |
| Performance | 10+ | Benchmarks |
| E2E | 10+ | Full workflows |

---

## Part 6: Implementation Phases

### Phase 1: Foundation (Week 1-2)
- [ ] Create `state/` module with domain separation
- [ ] Extract `LibraryState` from `App`
- [ ] Add unit tests for `LibraryState`
- [ ] Verify existing tests still pass

### Phase 2: List Virtualization (Week 3-4)
- [ ] Create `LibraryList` component with `uniform_list`
- [ ] Create `AlbumCard` as `RenderOnce` component
- [ ] Add scroll handle and keyboard navigation
- [ ] Add performance benchmarks
- [ ] Verify 1000+ album performance

### Phase 3: Component Extraction (Week 5-6)
- [ ] Extract `PlayerState`, `QueueState`, `PluginState`
- [ ] Create `QueueList` with `uniform_list`
- [ ] Add component tests
- [ ] Update screen components to use new state

### Phase 4: Audio Visualization (Week 7-8)
- [ ] Create `LevelMeterElement` custom element
- [ ] Create `SpectrumElement` custom element
- [ ] Implement push-based meter updates
- [ ] Add visual regression tests

### Phase 5: Polish (Week 9-10)
- [ ] Create `EQCurveElement` for GPU-rendered EQ graph
- [ ] Add image caching for album art
- [ ] Performance optimization pass
- [ ] Complete test coverage

---

## Part 7: Migration Strategy

### 7.1 Backward Compatibility

During migration, maintain both old and new implementations:

```rust
// Feature flag for gradual rollout
#[cfg(feature = "new_library")]
pub fn render_library_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
    LibraryScreen::new(self.library.clone())
}

#[cfg(not(feature = "new_library"))]
pub fn render_library_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
    // Existing implementation
}
```

### 7.2 Test-Driven Migration

1. Write tests for new component behavior
2. Implement new component
3. Verify tests pass
4. Run existing tests against old implementation
5. Switch default, keep feature flag
6. Remove old implementation after stabilization

---

## Part 8: Success Metrics

### Performance
- [ ] Library grid renders in <10ms for 10,000 albums
- [ ] Spectrum analyzer runs at 60fps
- [ ] Level meters update with <20ms latency
- [ ] Memory usage constant regardless of library size

### Code Quality
- [ ] No file >500 lines
- [ ] All public APIs documented
- [ ] Test coverage >80%
- [ ] No circular dependencies between modules

### Developer Experience
- [ ] Incremental builds <10s
- [ ] Tests run in <30s
- [ ] Clear module boundaries
- [ ] Easy to add new screen/component

---

## Appendix: GPUI Patterns Reference

### A. Entity Lifecycle

```rust
// Create
let entity = cx.new(|cx| MyState::new());

// Read
let value = entity.read(cx).field;

// Update
entity.update(cx, |state, cx| {
    state.field = new_value;
    cx.notify();
});

// Subscribe to changes
cx.observe(&entity, |this, _entity, cx| {
    cx.notify();
}).detach();
```

### B. Event Emission

```rust
impl EventEmitter<MyEvent> for MyState {}

// Emit
cx.emit(MyEvent::Changed);

// Subscribe
cx.subscribe(&entity, |this, _emitter, event, cx| {
    match event {
        MyEvent::Changed => this.on_change(cx),
    }
}).detach();
```

### C. Async Patterns

```rust
// Background work
cx.background_spawn(async move {
    expensive_computation()
}).detach();

// Foreground with entity access
cx.spawn(async move |weak_handle, mut cx| {
    let result = fetch_data().await;
    weak_handle.update(&mut cx, |this, cx| {
        this.data = result;
        cx.notify();
    }).ok();
}).detach();
```

### D. Custom Elements

```rust
impl Element for MyElement {
    type RequestLayoutState = ();
    type PrepaintState = Bounds<Pixels>;

    fn request_layout(...) -> (LayoutId, Self::RequestLayoutState);
    fn prepaint(...) -> Self::PrepaintState;
    fn paint(...);
}
```

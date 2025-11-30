# Spinorama Demo Refactoring Plan

## Current State Analysis

**File**: `/Users/pierre/src/sotf/gpui-d3rs/bin/spinorama_demo.rs`
**Size**: 2,572 lines
**Status**: Monolithic single-file binary

### Key Components Identified

1. **Type Definitions** (Lines 44-510)
   - `PlotCurve` struct with impl (lines 56-88)
   - `SecondaryAxisConfig` struct (lines 89-98)
   - `BrushOverlay` struct (lines 99-107)
   - `ContourRenderMode` enum with impl (lines 308-334)
   - `Colormap` enum with impl (lines 335-471)
   - `ChartId` enum (lines 472-479)
   - `PlotSection` enum with impl (lines 480-509)
   - `LoadState` enum (lines 510-517)

2. **Constants and Helper Functions** (Lines 33-107, 463-471)
   - `CEA2034_CURVES` constant
   - `cea2034_colors()` function
   - `interpolate_colors()` function

3. **Rendering Functions** (Lines 108-306, 1798-2446)
   - `render_freq_spl_plot()` function (~200 lines)
   - `render_mode_toggle()` method (~90 lines)
   - `render_contour_from_contour_data()` method (~280 lines)
   - `render_contour_from_directivity()` method (~280 lines)
   - `render_contour_plot()` method (~110 lines)

4. **Main App State** (Lines 518-561)
   - `SpinoramaApp` struct with 23 fields

5. **App Implementation** (Lines 563-2522)
   - Constructor: `new()` (lines 564-611)
   - Data loading: `load_speakers()`, `load_versions()`, `load_speaker_data()` (lines 613-780)
   - Dropdown handlers: `on_speaker_selected()`, `on_version_selected()`, `on_section_selected()` (lines 781-1222)
   - Render methods for UI components (lines 783-2446)

6. **Main Entry Point** (Lines 2539-2572)
   - GPUI application setup
   - Window creation
   - Action handlers

---

## Proposed Module Structure

### Directory Layout
```
bin/spinorama_demo/
├── lib.rs                    # Module definitions and re-exports
├── main.rs                   # Application entry point and GPUI setup
├── app/
│   ├── mod.rs               # SpinoramaApp definition and re-exports
│   ├── state.rs             # App state struct and initialization
│   ├── data_loading.rs      # Data fetch methods (load_speakers, load_versions, etc.)
│   └── handlers.rs          # Event handlers (on_speaker_selected, etc.)
├── render/
│   ├── mod.rs               # Render trait impl and layout re-exports
│   ├── header.rs            # Header UI rendering (dropdowns, controls)
│   ├── content.rs           # Main content area rendering (welcome, loading, error states)
│   ├── cea2034.rs           # CEA2034 frequency/SPL plot rendering
│   ├── directivity.rs       # Directivity plot rendering
│   ├── contour.rs           # Contour plot rendering (both variants)
│   └── legend.rs            # Legend rendering for curves
├── types/
│   ├── mod.rs               # Type definitions and re-exports
│   ├── enums.rs             # ChartId, LoadState, PlotSection, ContourRenderMode, Colormap
│   ├── plot_curve.rs        # PlotCurve struct and impl
│   └── config.rs            # SecondaryAxisConfig, BrushOverlay
└── utils/
    ├── mod.rs               # Utilities re-exports
    ├── colors.rs            # CEA2034 colors, color interpolation
    └── constants.rs         # CEA2034_CURVES constant
```

---

## Migration Strategy

### Phase 1: Setup Structure
1. Create `bin/spinorama_demo/` directory
2. Create `lib.rs` as the main module root with all mod declarations
3. Create empty submodule files
4. Update `Cargo.toml` to convert binary to a binary using a library

### Phase 2: Extract Types (No Dependencies on App State)
Move in this order (simplest to most complex):

#### Step 1: `types/constants.rs`
- **Content**: `CEA2034_CURVES` constant
- **Lines affected**: ~8 lines from original line 34-41
- **No dependencies**: Pure constant

#### Step 2: `types/config.rs`
- **Content**:
  - `SecondaryAxisConfig` struct (lines 89-98)
  - `BrushOverlay` struct (lines 99-107)
- **Dependencies**: None (simple data structures)

#### Step 3: `types/plot_curve.rs`
- **Content**:
  - `PlotCurve` struct (lines 56-66)
  - `impl PlotCurve` (lines 67-88)
- **Dependencies**: `d3rs::shape::LineConfig`, `d3rs::color::D3Color`

#### Step 4: `types/enums.rs`
- **Content**:
  - `LoadState` enum (lines 510-517)
  - `ChartId` enum (lines 472-479)
  - `PlotSection` enum with impl (lines 480-509)
  - `ContourRenderMode` enum with impl (lines 308-334)
  - `Colormap` enum with impl (lines 335-471)
- **Dependencies**: `d3rs` types
- **Note**: `Colormap` impl uses `interpolate_colors()` which needs to be moved to utils

#### Step 5: `types/mod.rs`
- Re-export all types with `pub use`

### Phase 3: Extract Utilities
#### Step 6: `utils/colors.rs`
- **Content**:
  - `cea2034_colors()` function (lines 44-53)
  - `interpolate_colors()` function (lines 463-471)
- **Dependencies**: `d3rs::color::D3Color`, `HashMap`

#### Step 7: `utils/constants.rs`
- **Content**: `CEA2034_CURVES` constant (move from types if needed, or leave in colors)
- **Keep in colors.rs**: More logical location

#### Step 8: `utils/mod.rs`
- Re-export utilities

### Phase 4: Extract App State and Implementation
#### Step 9: `app/state.rs`
- **Content**:
  - `SpinoramaApp` struct definition (lines 518-561)
  - Constructor: `SpinoramaApp::new()` (lines 564-611)
- **Dependencies**: All types, `d3rs` zoom/brush, `Arc`, `Rc`, `RefCell`

#### Step 10: `app/data_loading.rs`
- **Content**:
  - `load_speakers()` method (lines 613-648)
  - `load_versions()` method (lines 650-706)
  - `load_speaker_data()` method (lines 708-780)
- **Dependencies**: `SpinoramaApp`, `LoadState`, autoeq types, tokio

#### Step 11: `app/handlers.rs`
- **Content**:
  - `on_speaker_selected()` handler (lines 781-915)
  - `on_version_selected()` handler (lines 916-1029)
  - `on_section_selected()` handler (lines 1030-1132)
  - Any other event handler methods
- **Dependencies**: `SpinoramaApp`, handlers use `cx` for async work

#### Step 12: `app/mod.rs`
- Include state, data_loading, handlers modules
- Re-export `SpinoramaApp` and impl blocks

### Phase 5: Extract Rendering
#### Step 13: `render/header.rs`
- **Content**:
  - `render_header()` method (lines 783-915)
  - `render_speaker_dropdown()` method (lines 916-1029)
  - `render_version_dropdown()` method (lines 1030-1132)
  - `render_section_dropdown()` method (lines 1133-1223)
- **Dependencies**: `SpinoramaApp`, UI components, event handlers
- **Implementation note**: Methods stay on `SpinoramaApp`, this module becomes private helper functions

#### Step 14: `render/cea2034.rs`
- **Content**:
  - `render_cea2034_plot()` method (lines 1510-1621)
  - `render_legend()` method (lines 1622-1655)
  - Standalone `render_freq_spl_plot()` function (lines 108-306)
- **Dependencies**: All d3rs rendering, axis, grid, brush types

#### Step 15: `render/directivity.rs`
- **Content**:
  - `render_directivity_plot()` method (lines 1656-1797)
- **Dependencies**: d3rs visualization

#### Step 16: `render/contour.rs`
- **Content**:
  - `render_mode_toggle()` method (lines 1798-1888)
  - `render_contour_from_contour_data()` method (lines 1889-2166)
  - `render_contour_from_directivity()` method (lines 2167-2445)
  - `render_contour_plot()` method (lines 2446-2522)
- **Dependencies**: All d3rs contour types, colormap logic

#### Step 17: `render/content.rs`
- **Content**:
  - `render_content()` method (lines 1224-1251)
  - `render_welcome()` method (lines 1252-1467)
  - `render_loading()` method (lines 1468-1484)
  - `render_error()` method (lines 1485-1509)
- **Dependencies**: Basic GPUI elements

#### Step 18: `render/mod.rs`
- Include render trait impl for `SpinoramaApp`
- Re-export rendering functions as needed
- Note: Render methods stay on `SpinoramaApp` struct, this just organizes where they're defined

### Phase 6: Finalize Main Entry Point
#### Step 19: Create `main.rs`
- **Content**:
  - Main function (lines 2539-2572)
  - Actions definition (lines 2536-2537)
  - Import from lib
- **No changes needed to logic**: Just reorganizes where code lives

#### Step 20: Create `lib.rs`
- Module declarations for all submodules
- Public re-exports to maintain external API
- All internal structure hidden behind lib.rs

---

## Implementation Details

### Binary Structure Change

**Before**:
```
bin/spinorama_demo.rs  (2,572 lines - single monolithic file)
```

**After**:
```
bin/spinorama_demo/
├── lib.rs              (module root, ~50 lines)
├── main.rs             (entry point, ~35 lines)
├── app/
│   ├── mod.rs          (~30 lines)
│   ├── state.rs        (~120 lines)
│   ├── data_loading.rs (~170 lines)
│   └── handlers.rs     (~280 lines)
├── render/
│   ├── mod.rs          (~20 lines)
│   ├── header.rs       (~350 lines)
│   ├── content.rs      (~280 lines)
│   ├── cea2034.rs      (~250 lines)
│   ├── directivity.rs  (~150 lines)
│   ├── contour.rs      (~600 lines)
│   └── legend.rs       (~50 lines)
├── types/
│   ├── mod.rs          (~30 lines)
│   ├── enums.rs        (~200 lines)
│   ├── plot_curve.rs   (~35 lines)
│   └── config.rs       (~25 lines)
└── utils/
    ├── mod.rs          (~10 lines)
    └── colors.rs       (~30 lines)
```

### Cargo.toml Changes

**Current entry**:
```toml
[[bin]]
name = "spinorama-demo"
path = "bin/spinorama_demo.rs"
required-features = ["spinorama"]
```

**New entry**:
```toml
[[bin]]
name = "spinorama-demo"
path = "bin/spinorama_demo/main.rs"
required-features = ["spinorama"]
```

### Key Design Decisions

1. **Keep SpinoramaApp struct and impl blocks in place**: Don't split impl blocks into separate types. Instead, keep the full struct definition and all its impl blocks organized logically across files that get included in the app module.

2. **Maintain Same Public API**: The external binary behavior remains identical. The refactoring is purely internal organization.

3. **No circular dependencies**: Utilities don't depend on app state, types don't depend on app, rendering depends on app but app doesn't depend on rendering.

4. **Gradual extraction**: Extract in dependency order - constants first, then simple types, then utilities, then app state, then rendering (which depends on app state).

5. **One-at-a-time moves**: After each logical unit is moved, run `cargo check` to verify compilation. Do NOT move multiple items at once.

---

## Compilation Verification Steps

After each migration step:
```bash
cargo check --bin spinorama-demo --all-features
```

Final verification after all steps:
```bash
cargo check --bin spinorama-demo --all-features
cargo clippy --bin spinorama-demo --all-features
cargo test --lib spinorama-demo
cargo run --bin spinorama-demo --release
```

---

## Testing Strategy

1. **No unit test changes needed**: This is a purely structural refactoring
2. **Binary still exists**: `cargo run --bin spinorama-demo --release` must work identically
3. **No API changes**: External crates don't depend on this binary
4. **Functional verification**: The GUI application behaves exactly the same

---

## Success Criteria

1. ✓ All code is properly split into modules
2. ✓ Compilation succeeds with `cargo check`
3. ✓ No clippy warnings introduced
4. ✓ Binary `spinorama-demo` builds and runs successfully
5. ✓ Application behaves identically to original
6. ✓ File size: Each module file < 400 lines (most < 300 lines)
7. ✓ Clear module hierarchy reflecting functionality
8. ✓ No circular dependencies between modules

---

## Rollback Plan

If issues occur:
1. The original `bin/spinorama_demo.rs` stays in git history
2. Can quickly revert by reverting to previous commit
3. Individual module files can be dropped if they cause issues

---

## Notes for Implementation

- **Imports must be updated carefully**: Each module will need appropriate imports from d3rs, autoeq, gpui, etc.
- **Visibility levels**: Use `pub(crate)` for internal helper functions, `pub` for exported types
- **Module organization**: Each submodule directory gets a `mod.rs` that re-exports public items
- **Library root (`lib.rs`)**: Declare all modules and re-export the main `SpinoramaApp` struct
- **Binary root (`main.rs`)**: Import from lib, set up GPUI application

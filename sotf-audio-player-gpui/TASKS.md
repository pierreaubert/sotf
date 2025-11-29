# UI Kit Integration Tasks

This document indexes all Rust files in `sotf-audio-player-gpui/src/` and identifies opportunities for ui_kit component integration.

## Summary

| Category | Files | Using ui_kit | Not Using | Potential |
|----------|-------|--------------|-----------|-----------|
| Screens | 5 | 2 | 3 | HIGH |
| Components | 8 | 2 | 6 | HIGH |
| Plugins | 16 | 0 | 16 | MEDIUM |
| Host/Rack | 3 | 0 | 3 | MEDIUM |
| EQ Graph | 5 | 0 | 5 | LOW |
| Elements | 3 | 0 | 3 | N/A (GPU) |
| UI Kit | 22 | - | - | N/A |
| App/State | 12 | - | - | N/A |

---

## HIGH PRIORITY - Screens

### 1. `src/ui/screens/library.rs`
- **Status**: PARTIAL - Uses Button, ButtonVariant, ButtonSize
- **Current**: Custom search input, view mode buttons, filter controls
- **Opportunities**:
  - [ ] Replace search input with `Input` component
  - [ ] Use `Badge` for album/track counts
  - [ ] Use `Tabs` for view mode switching (Tree/Grid/Flat)
  - [ ] Use `Select` for channel filter dropdown
  - [ ] Use `HStack`/`VStack` for consistent layout

### 2. `src/ui/screens/queue.rs`
- **Status**: NOT USING ui_kit
- **Current**: Custom M/S/D buttons, channel meter display
- **Opportunities**:
  - [ ] Use `IconButton` for Mute/Solo/Dim controls
  - [ ] Use `Badge` for channel labels
  - [ ] Use `VStack`/`HStack` for layout
  - [ ] Use `Card` for channel group containers
  - [ ] Replace hardcoded colors with theme

### 3. `src/ui/screens/devices.rs`
- **Status**: NOT USING ui_kit
- **Current**: Device cards with hardcoded colors (0x007acc, 0x2d2d2d)
- **Opportunities**:
  - [ ] Use `Card` for device cards
  - [ ] Use `Badge` for "Default" indicator
  - [ ] Use `Text` for device name/specs
  - [ ] Use `HStack` for specs row (channels, sample rate)
  - [ ] Replace hardcoded colors with theme

### 4. `src/ui/screens/directory.rs`
- **Status**: NOT USING ui_kit
- **Current**: Custom directory tree, autocomplete input, progress
- **Opportunities**:
  - [ ] Use `Input` for directory path input
  - [ ] Use `Button` for add/remove actions
  - [ ] Use `Progress` for scan progress
  - [ ] Use `VStack` for directory list
  - [ ] Use `Text` for path display
  - [ ] Replace hardcoded colors (0x264f78, 0x2d2d2d) with theme

### 5. `src/ui/screens/settings.rs`
- **Status**: PARTIAL - Uses Button, HStack, StackSpacing
- **Current**: Custom accordion implementation (not using Accordion component)
- **Opportunities**:
  - [ ] Current accordion is custom - could use `Accordion` component directly
  - [ ] Use `Toggle` for boolean settings
  - [ ] Use `Select` for dropdown selections
  - [ ] Use `Card` for section containers
  - [ ] Use `Divider` for section separators

---

## HIGH PRIORITY - Core Components

### 6. `src/ui/components/header.rs`
- **Status**: PARTIAL - Uses menu_bar_button, Button, HStack, VStack, Divider
- **Current**: Custom dropdown menus
- **Opportunities**:
  - [ ] Use `Menu`/`MenuItem` for dropdown menus (currently custom)
  - [ ] Use `Badge` for scan progress indicator

### 7. `src/ui/components/footer.rs`
- **Status**: PARTIAL - Uses HStack, VStack, StackSpacing
- **Current**: Custom transport buttons, potentiometer
- **Opportunities**:
  - [ ] Use `IconButton` for transport controls (play, pause, stop, prev, next)
  - [ ] Use `Progress` for seek bar
  - [ ] Use `Text` for time display
  - [ ] Use `Avatar` for album artwork (if square shape)

### 8. `src/ui/components/dialogs.rs`
- **Status**: NOT USING ui_kit
- **Current**: Custom modal with hardcoded colors (0x1e1e1e, 0x007acc)
- **Opportunities**:
  - [ ] Use `Dialog` component for all modals
  - [ ] Use `Button` for dialog actions
  - [ ] Use `Text`/`Heading` for dialog content
  - [ ] Use `VStack` for keybinding lists
  - [ ] Replace hardcoded colors with theme

### 9. `src/ui/components/album_card.rs`
- **Status**: NOT USING ui_kit
- **Current**: Custom RenderOnce component with theme colors
- **Opportunities**:
  - [ ] Use `Card` as base container
  - [ ] Use `Badge` for track count
  - [ ] Use `Text` for title/artist/year
  - [ ] Use `Avatar` for album artwork

### 10. `src/ui/components/optimization_forms.rs`
- **Status**: NOT USING ui_kit
- **Current**: Custom parameter form sections
- **Opportunities**:
  - [ ] Use `Input` for numeric parameters
  - [ ] Use `Select` for dropdown selections
  - [ ] Use `Button` for action buttons
  - [ ] Use `Card` for form sections
  - [ ] Use `VStack`/`HStack` for layout

### 11. `src/ui/components/potentiometer.rs`
- **Status**: NOT USING ui_kit
- **Current**: Custom circular knob control
- **Opportunities**:
  - [ ] Keep as custom (no ui_kit equivalent)
  - [ ] Could wrap with theme-aware container

---

## MEDIUM PRIORITY - Plugin Components

### 12. `src/ui/components/plugins/common.rs`
- **Status**: NOT USING ui_kit
- **Current**: Shared utilities (param rows, sliders, meters)
- **Opportunities**:
  - [ ] Use `Input` for parameter value display
  - [ ] Use `Progress` for meters
  - [ ] Use `Text` for labels
  - [ ] Use `HStack` for parameter rows
  - [ ] Use `Toggle` for boolean parameters
  - **Impact**: HIGH - changes here affect all plugins

### 13. `src/ui/components/plugins/eq.rs`
- **Status**: NOT USING ui_kit
- **Current**: EQ graph with band controls
- **Opportunities**:
  - [ ] Use `Card` for band control sections
  - [ ] Use `Badge` for band number indicators
  - [ ] Use `Select` for filter type dropdown
  - [ ] Use `VStack`/`HStack` for layout

### 14. `src/ui/components/plugins/compressor.rs`
- **Status**: NOT USING ui_kit
- **Current**: Transfer curve + parameter sliders
- **Opportunities**:
  - [ ] Use `Card` for parameter groups
  - [ ] Use `Toggle` for link channels, auto-makeup
  - [ ] Use `VStack`/`HStack` for layout

### 15. `src/ui/components/plugins/limiter.rs`
- **Status**: NOT USING ui_kit
- **Current**: Threshold/release sliders
- **Opportunities**:
  - [ ] Use `Card` for container
  - [ ] Use `VStack` for layout

### 16. `src/ui/components/plugins/gate.rs`
- **Status**: NOT USING ui_kit
- **Current**: Gate parameter sliders
- **Opportunities**:
  - [ ] Use `Card` for container
  - [ ] Use `Toggle` for link channels
  - [ ] Use `VStack` for layout

### 17. `src/ui/components/plugins/gain.rs`
- **Status**: NOT USING ui_kit
- **Current**: Simple gain slider
- **Opportunities**:
  - [ ] Use `Card` for container
  - [ ] Use `Text` for value display

### 18. `src/ui/components/plugins/upmixer.rs`
- **Status**: NOT USING ui_kit
- **Current**: Speaker config + many sliders
- **Opportunities**:
  - [ ] Use `Select` for speaker configuration
  - [ ] Use `Toggle` for HR Direct, subharmonic
  - [ ] Use `Card` for parameter groups
  - [ ] Use `Tabs` for parameter categories

### 19. `src/ui/components/plugins/binaural.rs`
- **Status**: NOT USING ui_kit
- **Current**: SOFA file + parameters
- **Opportunities**:
  - [ ] Use `Input` for file path
  - [ ] Use `Button` for browse
  - [ ] Use `Toggle` for optimization
  - [ ] Use `Badge` for file status

### 20. `src/ui/components/plugins/convolution.rs`
- **Status**: NOT USING ui_kit
- **Current**: IR file + mix/gain
- **Opportunities**:
  - [ ] Use `Input` for file path
  - [ ] Use `Button` for browse
  - [ ] Use `Card` for container

### 21. `src/ui/components/plugins/loudness.rs`
- **Status**: NOT USING ui_kit
- **Current**: LUFS controls + meter
- **Opportunities**:
  - [ ] Use `Progress` for loudness meter
  - [ ] Use `Card` for container
  - [ ] Use `Badge` for LUFS value

### 22. `src/ui/components/plugins/mute_solo.rs`
- **Status**: NOT USING ui_kit
- **Current**: Channel mute/solo grid
- **Opportunities**:
  - [ ] Use `IconButton` for M/S buttons
  - [ ] Use `Toggle` for enabled state
  - [ ] Use `Badge` for channel labels

### 23. `src/ui/components/plugins/spectrum.rs`
- **Status**: NOT USING ui_kit
- **Current**: Spectrum bars + controls
- **Opportunities**:
  - [ ] Use `Select` for bin count
  - [ ] Use `Card` for container

---

## MEDIUM PRIORITY - Host/Rack

### 24. `src/ui/components/host/rack.rs`
- **Status**: NOT USING ui_kit
- **Current**: Plugin modules with drag-drop
- **Opportunities**:
  - [ ] Use `Card` for plugin modules
  - [ ] Use `Badge` for plugin type indicator
  - [ ] Use `IconButton` for settings/delete
  - [ ] Use `Toggle` for enable/bypass

### 25. `src/ui/components/host/plugin_editing.rs`
- **Status**: NOT USING ui_kit
- **Current**: Parameter list with editing
- **Opportunities**:
  - [ ] Use `Input` for parameter values
  - [ ] Use `VStack` for parameter list
  - [ ] Use `Text` for hints

### 26. `src/ui/components/host/mod.rs`
- **Status**: NOT USING ui_kit
- **Current**: Host state management
- **Opportunities**:
  - [ ] Use `Button` for add plugin
  - [ ] Use `Select` for plugin type selection

---

## LOW PRIORITY - EQ Graph (Specialized)

### 27-31. `src/ui/components/eq_graph/*.rs`
- **Files**: mod.rs, axis.rs, grid.rs, label.rs, legend.rs
- **Status**: NOT USING ui_kit (specialized graphing)
- **Opportunities**:
  - [ ] Use `Text` for axis labels (minor improvement)
  - [ ] Keep custom rendering for graphs
  - **Note**: These are specialized visualization components

---

## N/A - GPU Elements (Custom Rendering)

### 32-34. `src/ui/elements/*.rs`
- **Files**: eq_curve.rs, level_meter.rs, spectrum.rs
- **Status**: Custom GPU Element implementations
- **Opportunities**: NONE - these require custom GPU rendering
- **Note**: Keep as-is, no ui_kit equivalent

---

## N/A - UI Kit Library

### 35-56. `src/ui_kit/*.rs`
22 component files - these ARE the ui_kit library:
- accordion.rs, alert.rs, avatar.rs, badge.rs, breadcrumbs.rs
- button.rs, card.rs, checkbox.rs, dialog.rs, icon_button.rs
- input.rs, menu.rs, mod.rs, progress.rs, select.rs
- spinner.rs, stack.rs, tabs.rs, text.rs, toast.rs
- toggle.rs, tooltip.rs

---

## N/A - App/State (No UI Rendering)

### 57-68. Non-UI Files
- `src/main.rs` - Entry point
- `src/lib.rs` - Module exports
- `src/theme.rs` - Theme definitions
- `src/config.rs` - Configuration
- `src/actions.rs` - Action definitions
- `src/keybindings.rs` - Keyboard shortcuts
- `src/i18n.rs` - Translations
- `src/optimization_params.rs` - EQ params
- `src/app/*.rs` - State management (7 files)

---

## Recommended Implementation Order

### Phase 1: Core Infrastructure (Highest Impact)
1. **`plugins/common.rs`** - Shared utilities affect all plugins
2. **`dialogs.rs`** - Use Dialog component for consistency
3. **`queue.rs`** - High visibility, simple refactor

### Phase 2: Screens
4. **`devices.rs`** - Simple Card/Badge usage
5. **`directory.rs`** - Input/Button/Progress
6. **`library.rs`** - Complete the partial integration

### Phase 3: Components
7. **`album_card.rs`** - Card/Badge/Text
8. **`footer.rs`** - Complete IconButton integration
9. **`header.rs`** - Use Menu/MenuItem properly

### Phase 4: Plugin UIs
10. **Individual plugins** - Apply common.rs patterns

### Phase 5: Host/Rack
11. **`rack.rs`** - Card/Badge/IconButton
12. **`plugin_editing.rs`** - Input/VStack

---

## Color Hardcoding Issues

Files with hardcoded RGB values that should use theme:

| File | Hardcoded Colors |
|------|------------------|
| queue.rs | 0x2d3748, 0x252525 |
| devices.rs | 0x007acc, 0x2d2d2d |
| directory.rs | 0x264f78, 0x2d2d2d |
| dialogs.rs | 0x1e1e1e, 0x007acc, 0x4ec9b0 |
| header.rs | 0x2a2a2a, 0x3a3a3a |
| rack.rs | Plugin-specific colors (acceptable) |

---

## Available UI Kit Components

| Component | Purpose | Best For |
|-----------|---------|----------|
| Button | Action buttons | All clickable actions |
| IconButton | Icon-only buttons | Transport, M/S/D controls |
| Card | Containers | Plugin modules, device cards |
| Dialog | Modal dialogs | Help, settings modals |
| Input | Text input | Search, file paths, params |
| Checkbox | Boolean input | Simple toggles |
| Toggle | Switch input | Enable/disable states |
| Select | Dropdown | Filter, type selection |
| Badge | Labels | Counts, status indicators |
| Progress | Progress bars | Scan, meters |
| Spinner | Loading | Async operations |
| Avatar | Images | Album art (square) |
| Text/Heading | Typography | Consistent text styling |
| Alert | Notifications | Inline messages |
| Toast | Notifications | Temporary messages |
| Tabs | Tab navigation | View modes, settings |
| Menu/MenuItem | Menus | Dropdowns, context menus |
| Accordion | Collapsible | Settings sections |
| VStack/HStack | Layout | Consistent spacing |
| Divider | Separators | Section dividers |
| Breadcrumbs | Navigation | Path display |
| Tooltip | Help text | Hover hints |

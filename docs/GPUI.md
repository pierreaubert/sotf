# GPUI Application Development Guide

This guide describes how to build GPUI applications using the project's UI libraries. It prioritizes:

1. **gpui-ui-kit** - Reusable components (Button, Slider, Accordion, etc.)
2. **gpui-px** - Data visualization (charts, graphs)
3. **Theming** - Consistent, switchable color schemes
4. **Keybindings** - Configurable keyboard shortcuts with preset support
5. **Internationalization** - Multi-language support

---

## Table of Contents

1. [Quick Start with MiniApp](#quick-start-with-miniapp)
2. [Using gpui-ui-kit Components](#using-gpui-ui-kit-components)
3. [Data Visualization with gpui-px](#data-visualization-with-gpui-px)
4. [Theming System](#theming-system)
5. [Keybindings System](#keybindings-system)
6. [Internationalization (i18n)](#internationalization-i18n)
7. [Application State Architecture](#application-state-architecture)
8. [GPUI Core Concepts](#gpui-core-concepts)
9. [Best Practices](#best-practices)

---

## Quick Start with MiniApp

Use `MiniApp` from gpui-ui-kit to bootstrap applications quickly:

```rust
use gpui::*;
use gpui_ui_kit::{MiniApp, MiniAppConfig};

struct MyApp {
    counter: usize,
}

impl Render for MyApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(format!("Counter: {}", self.counter))
    }
}

fn main() {
    MiniApp::run(
        MiniAppConfig::new("My Application")
            .size(1200.0, 800.0)
            .scrollable(true),
        |cx| cx.new(|_| MyApp { counter: 0 }),
    );
}
```

**File reference**: `gpui-ui-kit/src-app/miniapp.rs`

MiniApp provides:
- Standard menu bar with Quit option (Cmd+Q)
- Configurable window title and size
- Optional vertical scrolling
- Application lifecycle management

---

## Using gpui-ui-kit Components

### Component Hierarchy

Always prefer gpui-ui-kit components over raw GPUI primitives:

```
gpui-ui-kit (preferred)
    └── Button, Slider, Accordion, IconButton, etc.
        └── gpui primitives (div, v_flex, h_flex)
            └── Raw GPU rendering
```

### Button Component

```rust
use gpui_ui_kit::{Button, ButtonVariant, ButtonSize, ButtonTheme};

// Basic button
Button::new("save-btn", "Save")
    .variant(ButtonVariant::Primary)
    .size(ButtonSize::Md)
    .on_click(|window, cx| {
        // Handle click
    })

// Button with theme
Button::new("cancel-btn", "Cancel")
    .variant(ButtonVariant::Secondary)
    .theme(theme.to_button_theme())  // From app theme

// Button with icons
Button::new("add-btn", "Add Item")
    .icon_left("+")
    .full_width(true)
    .disabled(is_loading)

// Using .build() for cx.listener
Button::new("action-btn", "Action")
    .variant(ButtonVariant::Primary)
    .build()
    .on_click(cx.listener(|this, event, window, cx| {
        this.handle_action(window, cx);
    }))
```

**Variants**: `Primary`, `Secondary`, `Destructive`, `Ghost`, `Outline`
**Sizes**: `Xs`, `Sm`, `Md`, `Lg`

**File reference**: `gpui-ui-kit/src/button.rs`

### Slider Component

```rust
use gpui_ui_kit::{Slider, SliderSize, SliderTheme};

Slider::new("volume-slider")
    .value(self.volume)
    .min(0.0)
    .max(100.0)
    .step(1.0)
    .label("Volume")
    .show_value(true)
    .width(200.0)
    .theme(theme.to_slider_theme())
    .on_change(|value, window, cx| {
        // Handle value change
    })
```

**File reference**: `gpui-ui-kit/src/slider.rs`

### Accordion Component

```rust
use gpui_ui_kit::{Accordion, AccordionTheme};

Accordion::new("settings-accordion")
    .title("Advanced Settings")
    .initially_expanded(false)
    .theme(theme.to_accordion_theme())
    .child(
        div()
            .p_4()
            .child("Accordion content here")
    )
```

**File reference**: `gpui-ui-kit/src/accordion.rs`

### IconButton Component

```rust
use gpui_ui_kit::{IconButton, IconButtonVariant, IconButtonTheme};

IconButton::new("play-btn", "▶")
    .variant(IconButtonVariant::Ghost)
    .selected(is_playing)
    .theme(theme.to_icon_button_theme())
    .on_click(|window, cx| {
        // Toggle playback
    })
```

**File reference**: `gpui-ui-kit/src/icon_button.rs`

---

## Data Visualization with gpui-px

gpui-px provides Plotly Express-style charting API.

### Chart Types

```rust
use gpui_px::{scatter, line, bar, heatmap, contour, isoline, boxplot};
use gpui_px::{ColorScale, ScaleType};

// Scatter plot
let chart = scatter(&x_data, &y_data)
    .title("Correlation Analysis")
    .color(0x1f77b4)  // Plotly blue
    .build()?;

// Line chart with log scale
let chart = line(&frequency, &magnitude_db)
    .title("Frequency Response")
    .x_scale(ScaleType::Log)
    .build()?;

// Bar chart
let chart = bar(&categories, &values)
    .title("Sales by Region")
    .color(0x2ca02c)
    .build()?;

// Heatmap
let z = compute_heatmap_data();
let chart = heatmap(&z, grid_width, grid_height)
    .title("Temperature Distribution")
    .color_scale(ColorScale::Viridis)
    .x_scale(ScaleType::Log)  // Optional log scale
    .build()?;

// Filled contour
let chart = contour(&z, width, height)
    .thresholds(vec![0.0, 0.5, 1.0, 1.5, 2.0])
    .color_scale(ColorScale::Plasma)
    .build()?;

// Isolines (unfilled contour)
let chart = isoline(&z, width, height)
    .levels(vec![0.5, 1.0, 1.5])
    .color(0x333333)
    .stroke_width(1.5)
    .build()?;

// Box plot
let chart = boxplot(&groups, &values)
    .title("Distribution Comparison")
    .build()?;
```

### Color Scales

```rust
ColorScale::Viridis   // Perceptually uniform (default)
ColorScale::Plasma    // Perceptually uniform
ColorScale::Inferno   // Perceptually uniform
ColorScale::Magma     // Perceptually uniform
ColorScale::Heat      // Diverging: blue → white → red
ColorScale::Coolwarm  // Diverging
ColorScale::Greys     // Sequential grayscale
ColorScale::custom(|t| /* 0.0-1.0 → RGBA */)
```

### Scale Types

```rust
ScaleType::Linear  // Default
ScaleType::Log     // Base-10 logarithmic (requires positive values)
```

**File reference**: `gpui-px/src/lib.rs`

---

## Theming System

### Theme Structure

Define a comprehensive theme with all UI colors:

```rust
use gpui::Rgba;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeId {
    #[default]
    Dark,
    Light,
    Midnight,
    Forest,
}

impl ThemeId {
    pub fn all() -> &'static [ThemeId] {
        &[ThemeId::Dark, ThemeId::Light, ThemeId::Midnight, ThemeId::Forest]
    }

    pub fn name(&self) -> &'static str {
        match self {
            ThemeId::Dark => "Dark",
            ThemeId::Light => "Light",
            ThemeId::Midnight => "Midnight",
            ThemeId::Forest => "Forest",
        }
    }

    pub fn next(&self) -> ThemeId {
        match self {
            ThemeId::Dark => ThemeId::Light,
            ThemeId::Light => ThemeId::Midnight,
            ThemeId::Midnight => ThemeId::Forest,
            ThemeId::Forest => ThemeId::Dark,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    // Base colors
    pub background: Rgba,
    pub background_secondary: Rgba,
    pub surface: Rgba,
    pub surface_hover: Rgba,
    pub surface_selected: Rgba,

    // Text colors
    pub text_primary: Rgba,
    pub text_secondary: Rgba,
    pub text_muted: Rgba,

    // Accent colors
    pub accent: Rgba,
    pub accent_hover: Rgba,

    // Semantic colors
    pub success: Rgba,
    pub warning: Rgba,
    pub error: Rgba,

    // Border colors
    pub border: Rgba,
}
```

### Theme Conversion Methods

Provide conversion methods for each gpui-ui-kit component:

```rust
impl Theme {
    pub fn from_id(id: ThemeId) -> Self {
        match id {
            ThemeId::Dark => Self::dark(),
            ThemeId::Light => Self::light(),
            // ...
        }
    }

    pub fn to_button_theme(&self) -> gpui_ui_kit::ButtonTheme {
        gpui_ui_kit::ButtonTheme {
            accent: self.accent,
            accent_hover: self.accent_hover,
            surface: self.surface,
            surface_hover: self.surface_hover,
            text_primary: self.text_primary,
            text_secondary: self.text_secondary,
            error: self.error,
            border: self.border,
        }
    }

    pub fn to_slider_theme(&self) -> gpui_ui_kit::SliderTheme {
        gpui_ui_kit::SliderTheme {
            track: self.surface_hover,
            fill: self.accent,
            thumb: self.text_primary,
            label: self.text_primary,
            value: self.text_secondary,
        }
    }

    pub fn to_accordion_theme(&self) -> gpui_ui_kit::AccordionTheme {
        gpui_ui_kit::AccordionTheme {
            header_bg: self.surface,
            header_hover_bg: self.surface_hover,
            content_bg: self.background,
            border: self.border,
            title_color: self.text_primary,
            indicator_color: self.text_muted,
        }
    }
}
```

### Applying Themes in Views

```rust
struct AppState {
    theme_id: ThemeId,
}

impl Render for AppState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::from_id(self.theme_id);

        div()
            .size_full()
            .bg(theme.background)
            .text_color(theme.text_primary)
            .child(
                Button::new("action", "Click Me")
                    .theme(theme.to_button_theme())
            )
            .child(
                Slider::new("volume")
                    .theme(theme.to_slider_theme())
            )
    }
}
```

### Theme Switching Action

```rust
actions!(app, [CycleTheme]);

impl AppState {
    fn cycle_theme(&mut self, _: &CycleTheme, _window: &mut Window, cx: &mut Context<Self>) {
        self.theme_id = self.theme_id.next();
        cx.notify();
    }
}
```

**File reference**: `sotf-audio-player/app-gpui/theme.rs`

---

## Keybindings System

### Keymap Presets

Support multiple keybinding schemes:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum KeymapPreset {
    #[default]
    Default,
    Vim,
    Emacs,
    VSCode,
}

impl KeymapPreset {
    pub fn name(&self) -> &'static str {
        match self {
            KeymapPreset::Default => "Default",
            KeymapPreset::Vim => "Vim",
            KeymapPreset::Emacs => "Emacs",
            KeymapPreset::VSCode => "VSCode",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            KeymapPreset::Default => KeymapPreset::Vim,
            KeymapPreset::Vim => KeymapPreset::Emacs,
            KeymapPreset::Emacs => KeymapPreset::VSCode,
            KeymapPreset::VSCode => KeymapPreset::Default,
        }
    }
}
```

### Building Keybindings

```rust
use gpui::*;

pub fn get_keybindings(preset: KeymapPreset) -> Vec<KeyBinding> {
    let mut bindings = Vec::new();

    // Common bindings shared across all presets
    bindings.extend(common_bindings());

    // Preset-specific bindings
    match preset {
        KeymapPreset::Default => bindings.extend(default_bindings()),
        KeymapPreset::Vim => bindings.extend(vim_bindings()),
        KeymapPreset::Emacs => bindings.extend(emacs_bindings()),
        KeymapPreset::VSCode => bindings.extend(vscode_bindings()),
    }

    bindings
}

fn common_bindings() -> Vec<KeyBinding> {
    vec![
        // Universal bindings
        KeyBinding::new("space", actions::PlayPause, None),
        KeyBinding::new("escape", actions::Cancel, None),
        KeyBinding::new("cmd-q", actions::QuitApp, None),
        KeyBinding::new("cmd-,", actions::OpenSettings, None),
    ]
}

fn default_bindings() -> Vec<KeyBinding> {
    vec![
        // Navigation
        KeyBinding::new("up", actions::SelectUp, None),
        KeyBinding::new("down", actions::SelectDown, None),
        KeyBinding::new("left", actions::SelectLeft, None),
        KeyBinding::new("right", actions::SelectRight, None),
        // Also support vim-style
        KeyBinding::new("k", actions::SelectUp, None),
        KeyBinding::new("j", actions::SelectDown, None),
        KeyBinding::new("h", actions::SelectLeft, None),
        KeyBinding::new("l", actions::SelectRight, None),
        // Actions
        KeyBinding::new("enter", actions::Confirm, None),
        KeyBinding::new("/", actions::ToggleSearch, None),
    ]
}

fn vim_bindings() -> Vec<KeyBinding> {
    vec![
        // Pure vim navigation
        KeyBinding::new("k", actions::SelectUp, None),
        KeyBinding::new("j", actions::SelectDown, None),
        KeyBinding::new("h", actions::Collapse, None),
        KeyBinding::new("l", actions::Expand, None),
        KeyBinding::new("ctrl-u", actions::PageUp, None),
        KeyBinding::new("ctrl-d", actions::PageDown, None),
        KeyBinding::new("g g", actions::GoToFirst, None),
        KeyBinding::new("G", actions::GoToLast, None),
        KeyBinding::new("o", actions::Confirm, None),
        KeyBinding::new("d d", actions::Delete, None),
    ]
}
```

### Registering Keybindings

```rust
fn setup_keybindings(cx: &mut App, preset: KeymapPreset) {
    let bindings = get_keybindings(preset);
    cx.bind_keys(bindings);
}

// In Application::new().run()
Application::new().run(|cx: &mut App| {
    setup_keybindings(cx, KeymapPreset::Default);
    // ...
});
```

### Documented Keybindings for Help

```rust
#[derive(Debug, Clone, Copy)]
pub enum KeybindingCategory {
    Navigation,
    Playback,
    Search,
    System,
}

pub struct DocumentedKeybinding {
    pub key: &'static str,
    pub description: &'static str,
    pub category: KeybindingCategory,
}

pub fn get_documented_keybindings(preset: KeymapPreset) -> Vec<DocumentedKeybinding> {
    match preset {
        KeymapPreset::Default => vec![
            DocumentedKeybinding {
                key: "↑/↓ or j/k",
                description: "Navigate items",
                category: KeybindingCategory::Navigation,
            },
            DocumentedKeybinding {
                key: "Enter",
                description: "Confirm selection",
                category: KeybindingCategory::Navigation,
            },
            DocumentedKeybinding {
                key: "/",
                description: "Toggle search",
                category: KeybindingCategory::Search,
            },
            // ...
        ],
        KeymapPreset::Vim => vec![
            // Vim-specific documentation
        ],
        // ...
    }
}
```

**File reference**: `sotf-audio-player/app-gpui/keybindings.rs`

---

## Internationalization (i18n)

### Language Enum

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Language {
    #[default]
    English,
    French,
    German,
    Spanish,
}

impl Language {
    pub fn all() -> &'static [Language] {
        &[Language::English, Language::French, Language::German, Language::Spanish]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::French => "Français",
            Language::German => "Deutsch",
            Language::Spanish => "Español",
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::French => "fr",
            Language::German => "de",
            Language::Spanish => "es",
        }
    }

    pub fn next(&self) -> Language {
        match self {
            Language::English => Language::French,
            Language::French => Language::German,
            Language::German => Language::Spanish,
            Language::Spanish => Language::English,
        }
    }
}
```

### Translations Struct

```rust
#[derive(Debug, Clone)]
pub struct Translations {
    // App
    pub app_title: &'static str,

    // Navigation
    pub nav_library: &'static str,
    pub nav_settings: &'static str,
    pub nav_queue: &'static str,

    // Actions
    pub action_save: &'static str,
    pub action_cancel: &'static str,
    pub action_delete: &'static str,

    // Messages
    pub msg_loading: &'static str,
    pub msg_error: &'static str,
    pub msg_success: &'static str,

    // Dialogs
    pub dialog_confirm_delete: &'static str,
}

impl Translations {
    pub fn for_language(language: Language) -> Self {
        match language {
            Language::English => Self::english(),
            Language::French => Self::french(),
            Language::German => Self::german(),
            Language::Spanish => Self::spanish(),
        }
    }

    pub fn english() -> Self {
        Self {
            app_title: "My Application",
            nav_library: "Library",
            nav_settings: "Settings",
            nav_queue: "Queue",
            action_save: "Save",
            action_cancel: "Cancel",
            action_delete: "Delete",
            msg_loading: "Loading...",
            msg_error: "An error occurred",
            msg_success: "Success!",
            dialog_confirm_delete: "Are you sure you want to delete this item?",
        }
    }

    pub fn french() -> Self {
        Self {
            app_title: "Mon Application",
            nav_library: "Bibliothèque",
            nav_settings: "Paramètres",
            nav_queue: "File d'attente",
            action_save: "Sauvegarder",
            action_cancel: "Annuler",
            action_delete: "Supprimer",
            msg_loading: "Chargement...",
            msg_error: "Une erreur s'est produite",
            msg_success: "Succès!",
            dialog_confirm_delete: "Êtes-vous sûr de vouloir supprimer cet élément?",
        }
    }

    // Similar for german() and spanish()...
}
```

### Using Translations in Views

```rust
struct AppState {
    language: Language,
    theme_id: ThemeId,
}

impl Render for AppState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Translations::for_language(self.language);
        let theme = Theme::from_id(self.theme_id);

        div()
            .size_full()
            .bg(theme.background)
            .child(
                h_flex()
                    .gap_2()
                    .child(Button::new("lib", t.nav_library).theme(theme.to_button_theme()))
                    .child(Button::new("set", t.nav_settings).theme(theme.to_button_theme()))
            )
            .child(
                div()
                    .child(t.msg_loading)
            )
    }
}
```

**File reference**: `sotf-audio-player/app-gpui/i18n.rs`

---

## Application State Architecture

### Recommended State Structure

```rust
use serde::{Deserialize, Serialize};

/// Persistent configuration (saved to disk)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub theme_id: ThemeId,
    pub language: Language,
    pub keymap_preset: KeymapPreset,
    // Domain-specific settings...
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme_id: ThemeId::default(),
            language: Language::default(),
            keymap_preset: KeymapPreset::default(),
        }
    }
}

/// Runtime application state
pub struct AppState {
    // Persistent config
    config: Config,

    // Runtime state
    current_screen: Screen,
    is_loading: bool,
    error_message: Option<String>,

    // Subscriptions (keep alive)
    _subscriptions: Vec<Subscription>,
}

impl AppState {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let config = Config::load_or_default();

        Self {
            config,
            current_screen: Screen::default(),
            is_loading: false,
            error_message: None,
            _subscriptions: Vec::new(),
        }
    }

    // Convenience accessors
    pub fn theme(&self) -> Theme {
        Theme::from_id(self.config.theme_id)
    }

    pub fn translations(&self) -> Translations {
        Translations::for_language(self.config.language)
    }
}
```

### Screen Management

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    Library,
    Settings,
    Queue,
    Help,
}

actions!(app, [
    SwitchToLibrary,
    SwitchToSettings,
    SwitchToQueue,
    ToggleHelp,
]);

impl AppState {
    fn switch_to_library(&mut self, _: &SwitchToLibrary, _: &mut Window, cx: &mut Context<Self>) {
        self.current_screen = Screen::Library;
        cx.notify();
    }

    fn switch_to_settings(&mut self, _: &SwitchToSettings, _: &mut Window, cx: &mut Context<Self>) {
        self.current_screen = Screen::Settings;
        cx.notify();
    }
}

impl Render for AppState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme();

        div()
            .size_full()
            .bg(theme.background)
            .on_action(cx.listener(Self::switch_to_library))
            .on_action(cx.listener(Self::switch_to_settings))
            .child(match self.current_screen {
                Screen::Library => self.render_library(window, cx),
                Screen::Settings => self.render_settings(window, cx),
                Screen::Queue => self.render_queue(window, cx),
                Screen::Help => self.render_help(window, cx),
            })
    }
}
```

---

## GPUI Core Concepts

### Context Types

- **`App`**: Root context for global state (not `Send`)
- **`Context<T>`**: Context for updating `Entity<T>`
- **`Window`**: Manages window state, focus, actions

### Entity Pattern

```rust
// Create entity
let entity = cx.new(|cx| MyState { value: 0 });

// Read
let value = entity.read(cx).value;

// Update
entity.update(cx, |state, cx| {
    state.value += 1;
    cx.notify();  // Trigger re-render
});
```

### Render vs RenderOnce

**Use `Render`** (stateful) when:
- Component maintains state across renders
- Component handles async operations
- Component needs subscriptions

```rust
struct Counter {
    count: usize,
}

impl Render for Counter {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().child(format!("Count: {}", self.count))
    }
}
```

**Use `RenderOnce`** (stateless) when:
- Pure presentation component
- Props → elements transformation

```rust
#[derive(IntoElement)]
struct Badge {
    text: SharedString,
}

impl RenderOnce for Badge {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        div().px_2().py_1().rounded_md().child(self.text)
    }
}
```

### Actions and Keybindings

```rust
// Define actions
actions!(my_app, [Save, Cancel, Delete]);

// Register handlers in render
div()
    .on_action(cx.listener(Self::handle_save))
    .on_action(cx.listener(Self::handle_cancel))

// Bind keys globally
cx.bind_keys([
    KeyBinding::new("cmd-s", Save, None),
    KeyBinding::new("escape", Cancel, None),
]);
```

### Event Emission

```rust
// Declare emitter
impl EventEmitter<DismissEvent> for Modal {}

// Emit
cx.emit(DismissEvent);

// Subscribe
let _sub = cx.subscribe(&modal, |this, modal, event, cx| {
    // Handle event
});
```

### Async Operations

```rust
// Background work (CPU-intensive)
cx.background_spawn(async move {
    expensive_computation()
})

// Foreground work (needs entity access)
cx.spawn(async move |weak_handle, mut cx| {
    let data = fetch_data().await;

    weak_handle.update(&mut cx, |this, cx| {
        this.data = Some(data);
        cx.notify();
    }).ok();
}).detach();
```

---

## Best Practices

### 1. Always Use gpui-ui-kit Components

```rust
// Prefer
Button::new("id", "Label").theme(theme.to_button_theme())

// Over raw div styling
div()
    .px_4()
    .py_2()
    .bg(theme.accent)
    .rounded_md()
    .child("Label")
```

### 2. Pass Theme Through Props

```rust
// Good: Theme flows from parent
fn render_sidebar(&self, theme: &Theme, t: &Translations) -> impl IntoElement {
    div()
        .bg(theme.surface)
        .child(t.nav_library)
}

// Avoid: Recreating theme in child
fn render_sidebar_bad(&self) -> impl IntoElement {
    let theme = Theme::from_id(self.theme_id); // Wasteful
}
```

### 3. Structure Translations by Domain

```rust
pub struct Translations {
    // Group by feature
    pub library: LibraryTranslations,
    pub settings: SettingsTranslations,
    pub playback: PlaybackTranslations,
}

pub struct LibraryTranslations {
    pub title: &'static str,
    pub search_placeholder: &'static str,
    pub empty_state: &'static str,
}
```

### 4. Keep Keybindings Configurable

```rust
// Store in config
pub struct Config {
    pub keymap_preset: KeymapPreset,
}

// Update on change
fn cycle_keymap(&mut self, _: &CycleKeymap, _: &mut Window, cx: &mut Context<Self>) {
    self.config.keymap_preset = self.config.keymap_preset.next();
    setup_keybindings(cx, self.config.keymap_preset);
    cx.notify();
}
```

### 5. Use Semantic Theme Colors

```rust
// Good: Semantic meaning
.bg(theme.error)           // For errors
.bg(theme.success)         // For success states
.bg(theme.surface_hover)   // For hover states

// Avoid: Raw colors
.bg(rgb(0xff0000))        // What does this mean?
```

### 6. Store Subscriptions

```rust
struct MyView {
    _subscriptions: Vec<Subscription>,  // Underscore prefix: kept alive
}

impl MyView {
    fn new(other: &Entity<Other>, cx: &mut Context<Self>) -> Self {
        let sub = cx.subscribe(other, |this, _, event, cx| {
            // Handle
        });

        Self {
            _subscriptions: vec![sub],
        }
    }
}
```

### 7. Handle Async Results

```rust
// Good
weak_handle.update(&mut cx, |this, cx| {
    this.data = data;
    cx.notify();
}).ok();  // Don't panic if entity dropped

// Or log errors
.log_err();
```

### 8. Avoid Blocking Main Thread

```rust
// Good: Heavy work on background
cx.background_spawn(async move {
    heavy_computation()
}).await;

// Bad: Blocks UI
let result = heavy_computation();  // UI freezes
```

---

## File References

### Project Libraries
- **gpui-ui-kit**: `gpui-ui-kit/src/`
  - Button: `button.rs`
  - Slider: `slider.rs`
  - Accordion: `accordion.rs`
  - MiniApp: `src-app/miniapp.rs`

- **gpui-px**: `gpui-px/src/`
  - Charts: `scatter.rs`, `line.rs`, `bar.rs`, `heatmap.rs`, `contour.rs`, `isoline.rs`, `boxplot.rs`

### Example Application
- **Theme System**: `sotf-audio-player/app-gpui/theme.rs`
- **Keybindings**: `sotf-audio-player/app-gpui/keybindings.rs`
- **i18n**: `sotf-audio-player/app-gpui/i18n.rs`
- **Main App**: `sotf-audio-player/app-gpui/main.rs`

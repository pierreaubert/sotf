# gpui-ui-kit

A reusable UI component library for [GPUI](https://github.com/zed-industries/zed) applications.

Provides composable, styled UI components with consistent theming for building desktop applications with the GPUI framework.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
gpui-ui-kit = { path = "../gpui-ui-kit" }
```

## Components

### Core Components

| Component | Description |
|-----------|-------------|
| `Button` | Styled button with variants (Primary, Secondary, Destructive, Ghost, Outline) |
| `IconButton` | Icon-only button with hover states |
| `Card` | Container with optional header, content, and footer sections |
| `Dialog` | Modal dialog with backdrop, title, and customizable size |
| `Menu` / `MenuBar` | Navigation menus and menu bars |
| `Tabs` | Tabbed navigation with Underline, Enclosed, and Pills variants |
| `Toast` / `ToastContainer` | Notification toasts with positioning |

### Form Components

| Component | Description |
|-----------|-------------|
| `Input` | Text input with label, placeholder, validation, and variants |
| `Checkbox` | Checkbox with label and indeterminate state |
| `Toggle` | Toggle switch |
| `Select` | Dropdown select with options |

### Data Display

| Component | Description |
|-----------|-------------|
| `Badge` / `BadgeDot` | Status badges with variants |
| `Progress` / `CircularProgress` | Progress bars and circular indicators |
| `Spinner` / `LoadingDots` | Loading indicators |
| `Avatar` / `AvatarGroup` | User avatars with status indicators |
| `Text` / `Heading` / `Code` / `Link` | Typography components |

### Feedback

| Component | Description |
|-----------|-------------|
| `Alert` / `InlineAlert` | Contextual feedback messages (Info, Success, Warning, Error) |
| `Tooltip` | Hover tooltips with placement options |

### Layout

| Component | Description |
|-----------|-------------|
| `VStack` / `HStack` | Vertical and horizontal stack layouts |
| `Spacer` | Flexible spacer element |
| `Divider` | Horizontal/vertical dividers with optional interactivity |
| `Accordion` | Collapsible content panels |
| `Breadcrumbs` | Navigation breadcrumbs |

## Usage Examples

### Button

```rust
use gpui_ui_kit::{Button, ButtonVariant, ButtonSize};

// Basic button
Button::new("btn-save", "Save")

// Primary button with click handler
Button::new("btn-submit", "Submit")
    .variant(ButtonVariant::Primary)
    .on_click(|window, cx| {
        println!("Button clicked!");
    })

// Destructive button
Button::new("btn-delete", "Delete")
    .variant(ButtonVariant::Destructive)
    .size(ButtonSize::Sm)

// Ghost button with icon
Button::new("btn-menu", "Menu")
    .variant(ButtonVariant::Ghost)
    .icon_left("☰")

// Full width disabled button
Button::new("btn-loading", "Loading...")
    .full_width(true)
    .disabled(true)
```

### Card

```rust
use gpui_ui_kit::Card;
use gpui::div;

Card::new()
    .header(div().child("Card Title"))
    .content(div().child("Card content goes here"))
    .footer(
        div().flex().gap_2()
            .child(Button::new("cancel", "Cancel").variant(ButtonVariant::Ghost))
            .child(Button::new("save", "Save"))
    )
```

### Input

```rust
use gpui_ui_kit::{Input, InputSize, InputVariant};

// Basic input with label
Input::new("email")
    .label("Email")
    .placeholder("Enter your email")

// Input with error
Input::new("username")
    .label("Username")
    .value("invalid!")
    .error("Username contains invalid characters")

// Filled variant
Input::new("search")
    .variant(InputVariant::Filled)
    .placeholder("Search...")
    .icon_left("🔍")
```

### Tabs

```rust
use gpui_ui_kit::{Tabs, TabItem, TabVariant};

Tabs::new()
    .tabs(vec![
        TabItem::new("general", "General").icon("⚙"),
        TabItem::new("audio", "Audio").icon("🔊"),
        TabItem::new("video", "Video").icon("🎬").badge("New"),
    ])
    .selected_index(0)
    .variant(TabVariant::Underline)
    .on_change(|index, window, cx| {
        println!("Selected tab: {}", index);
    })
```

### Dialog

```rust
use gpui_ui_kit::{Dialog, DialogSize};
use gpui::div;

Dialog::new("confirm-dialog")
    .title("Confirm Action")
    .size(DialogSize::Md)
    .content(div().child("Are you sure you want to proceed?"))
    .footer(
        div().flex().gap_2().justify_end()
            .child(Button::new("cancel", "Cancel").variant(ButtonVariant::Ghost))
            .child(Button::new("confirm", "Confirm"))
    )
    .on_close(|window, cx| {
        // Handle dialog close
    })
```

### Alert

```rust
use gpui_ui_kit::{Alert, AlertVariant, InlineAlert};

// Full alert with title
Alert::new("error-alert", "Something went wrong. Please try again.")
    .title("Error")
    .variant(AlertVariant::Error)
    .closeable(true)
    .on_close(|window, cx| {
        // Dismiss alert
    })

// Inline alert
InlineAlert::new("Operation completed successfully")
    .variant(AlertVariant::Success)
```

### Stack Layouts

```rust
use gpui_ui_kit::{VStack, HStack, Spacer, Divider, StackSpacing, StackAlign};

// Vertical stack
VStack::new()
    .spacing(StackSpacing::Lg)
    .align(StackAlign::Center)
    .child(Text::new("Title").size(TextSize::Xl))
    .child(Text::new("Subtitle"))
    .child(Spacer::new())
    .child(Button::new("action", "Action"))

// Horizontal stack with divider
HStack::new()
    .spacing(StackSpacing::Md)
    .child(Button::new("a", "Option A"))
    .child(Divider::vertical())
    .child(Button::new("b", "Option B"))
```

### Progress

```rust
use gpui_ui_kit::{Progress, CircularProgress, ProgressVariant, ProgressSize};

// Linear progress bar
Progress::new(75.0)
    .variant(ProgressVariant::Success)
    .size(ProgressSize::Md)
    .show_label(true)

// Circular progress
CircularProgress::new(60.0)
    .size(px(64.0))
    .variant(ProgressVariant::Default)
    .show_label(true)
```

### Checkbox and Toggle

```rust
use gpui_ui_kit::{Checkbox, Toggle, CheckboxSize, ToggleSize};

// Checkbox with label
Checkbox::new("agree")
    .label("I agree to the terms")
    .checked(true)
    .on_change(|checked, window, cx| {
        println!("Checked: {}", checked);
    })

// Toggle switch
Toggle::new("notifications")
    .label("Enable notifications")
    .checked(false)
    .size(ToggleSize::Md)
```

### Select

```rust
use gpui_ui_kit::{Select, SelectOption, SelectSize};

Select::new("theme-select")
    .label("Theme")
    .placeholder("Choose a theme")
    .options(vec![
        SelectOption::new("light", "Light"),
        SelectOption::new("dark", "Dark"),
        SelectOption::new("system", "System").disabled(true),
    ])
    .selected("dark")
    .on_change(|value, window, cx| {
        println!("Selected: {}", value);
    })
```

### Avatar

```rust
use gpui_ui_kit::{Avatar, AvatarGroup, AvatarSize, AvatarStatus};

// Single avatar with status
Avatar::new("user-1")
    .initials("JD")
    .size(AvatarSize::Lg)
    .status(AvatarStatus::Online)

// Avatar group
AvatarGroup::new()
    .avatars(vec![
        Avatar::new("u1").initials("AB"),
        Avatar::new("u2").initials("CD"),
        Avatar::new("u3").initials("EF"),
    ])
    .max_visible(3)
```

### Tooltip

```rust
use gpui_ui_kit::{Tooltip, TooltipPlacement, WithTooltip};

// Wrap any element with a tooltip
WithTooltip::new(
    Button::new("help", "?").variant(ButtonVariant::Ghost),
    Tooltip::new("Click for help").placement(TooltipPlacement::Bottom)
)
```

### Accordion

```rust
use gpui_ui_kit::{Accordion, AccordionItem, AccordionMode};
use gpui::div;

Accordion::new("faq")
    .mode(AccordionMode::Single)
    .items(vec![
        AccordionItem::new("q1", "What is GPUI?")
            .content(div().child("GPUI is a GPU-accelerated UI framework.")),
        AccordionItem::new("q2", "How do I install it?")
            .content(div().child("Add it to your Cargo.toml dependencies.")),
    ])
```

## Theming

Components use a default dark theme. Button theme can be customized:

```rust
use gpui_ui_kit::{Button, ButtonTheme};
use gpui::rgb;

let custom_theme = ButtonTheme {
    accent: rgb(0x6366f1),      // Indigo accent
    accent_hover: rgb(0x818cf8),
    surface: rgb(0x374151),
    surface_hover: rgb(0x4b5563),
    text_primary: rgb(0xffffff),
    text_secondary: rgb(0xd1d5db),
    error: rgb(0xef4444),
    border: rgb(0x6b7280),
};

Button::new("themed", "Themed Button")
    .theme(custom_theme)
```

## Design Patterns

### Builder Pattern

All components use the builder pattern for configuration:

```rust
Component::new(required_args)
    .optional_setting(value)
    .another_setting(value)
    // Either render directly or build for additional handlers
```

### Event Handlers

Components that support interaction accept closures:

```rust
Button::new("btn", "Click")
    .on_click(|window, cx| {
        // Handle click
    })

Checkbox::new("cb")
    .on_change(|checked, window, cx| {
        // Handle change
    })
```

### Using with GPUI Listeners

For components that need `cx.listener()`, use the `build()` method:

```rust
Button::new("btn", "Save")
    .build()
    .on_click(cx.listener(|this, _event, window, cx| {
        this.save(cx);
    }))
```

## Testing

The library includes a comprehensive test suite to prevent regressions:

```bash
# Run all tests
cargo test --lib --tests

# Run specific test suite
cargo test --test i18n_tests      # Translation coverage
cargo test --test component_tests  # Component API tests

# Setup git hooks for automatic testing
./scripts/setup-hooks.sh
```

**Test Coverage**:
- ✅ **Interaction Tests** (29 tests): Verify all stateful components support mouse and keyboard events
- ✅ **I18n Tests** (11 tests): Verify all translations exist across 5 languages (English, French, German, Spanish, Japanese)
- ✅ **Component Tests** (14 tests): Ensure component APIs work correctly and configurations are valid
- ✅ **Library Tests** (7 tests): Verify MiniApp configuration and utilities
- ✅ **61 total tests** covering critical functionality

See [`TESTING.md`](TESTING.md) for detailed testing guide and [`tests/README.md`](tests/README.md) for quick reference.

## Development

### Running the Showcase

```bash
cargo run --example showcase
```

The showcase demonstrates all components with:
- Interactive examples for each component
- Theme switching (Light/Dark)
- Language switching (5 languages)
- Navigation sidebar

### Before Committing

```bash
# Format code
cargo fmt

# Run tests
cargo test --lib --tests

# Run clippy
cargo clippy --all-targets -- -D warnings
```

Or setup git hooks to run automatically:
```bash
./scripts/setup-hooks.sh
```

## License

See workspace root for license information.

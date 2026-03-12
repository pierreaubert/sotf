# SOTF Design System Rules — Figma to GPUI

## 1. Token Definitions

### Colors
Defined in `crates/gpui-toolkit/gpui-ui-kit/src/theme.rs` as the `Theme` struct with 6 variants:
- `Dark` (default), `Light`, `Midnight`, `Forest`, `BlackAndWhite`, `Onyx`

Each theme defines ~30 color fields: `background`, `surface`, `surface_hover`, `muted`, `text_primary`, `text_secondary`, `text_muted`, `text_on_accent`, `accent`, `accent_hover`, `accent_muted`, `success`, `warning`, `error`, `info`, `border`, `border_hover`, `overlay_bg`, plus badge colors.

Colors are `gpui::Rgba` values. No token transformation system — colors are hardcoded hex values per theme.

### Typography
- System font: `.SystemUI` (SF Pro on macOS)
- Mono font: `B612`
- Sizes: Xs=10, Sm=12, Md=14, Lg=18, Xl=24, Xxl=32 (pixels)
- Weights: Light=300, Normal=400, Medium=500, Semibold=600, Bold=700

### Spacing
No explicit spacing tokens. Components use hardcoded `px()` values following this scale:
- xs=2, sm=4, md=8, lg=16, xl=24, xxl=32

### Border Radius
- sm=4, md=6, lg=8, xl=12, full=9999

### Component Sizes
`ComponentSize` enum: Xs (0.5x), Sm (0.75x), Md (1.0x default), Lg (1.5x), Xl (2.0x)

## 2. Component Library

Located at `crates/gpui-toolkit/gpui-ui-kit/src/`. Each component is a single `.rs` file.

### Core Components
| File | Component | Key Variants |
|------|-----------|-------------|
| `button.rs` | `Button` | ButtonVariant: Primary/Secondary/Destructive/Ghost/Outline, ButtonSize: Xs/Sm/Md/Lg |
| `icon_button.rs` | `IconButton` | IconButtonVariant: Ghost/Filled/Outline, IconButtonSize: Xs/Sm/Md/Lg/Xl |
| `input.rs` | `Input` | InputVariant: Default/Filled/Flushed, InputSize: Xs/Sm/Md/Lg |
| `number_input.rs` | `NumberInput` | NumberInputSize: Xs/Sm/Md/Lg |
| `select.rs` | `Select` | SelectSize: Xs/Sm/Md/Lg |
| `checkbox.rs` | `Checkbox` | CheckboxSize: Sm/Md/Lg |
| `toggle.rs` | `Toggle` | ToggleStyle: Sliding/Segmented, ToggleSize: Sm/Md/Lg |
| `slider.rs` | `Slider` | SliderSize: Sm/Md/Lg |
| `badge.rs` | `Badge` | BadgeVariant: Default/Primary/Success/Warning/Error/Info, BadgeSize: Sm/Md/Lg |
| `alert.rs` | `Alert` | AlertVariant: Info/Success/Warning/Error |
| `toast.rs` | `Toast` | ToastVariant: Info/Success/Warning/Error |
| `progress.rs` | `Progress` | ProgressVariant: Default/Success/Warning/Error, ProgressSize: Xs/Sm/Md/Lg |
| `spinner.rs` | `Spinner` | SpinnerSize: Xs/Sm/Md/Lg/Xl |
| `tabs.rs` | `Tabs` | TabVariant: Underline/Enclosed/Pills/VerticalCard |
| `button_set.rs` | `ButtonSet` | ButtonSetSize: Xs/Sm/Md/Lg |
| `breadcrumbs.rs` | `Breadcrumbs` | BreadcrumbSeparator |
| `menu.rs` | `Menu` | MenuItem types |
| `accordion.rs` | `Accordion` | AccordionMode: Single/Multiple, AccordionOrientation: Vertical/Horizontal/Side |
| `tooltip.rs` | `Tooltip` | TooltipPlacement |
| `card.rs` | `Card` | Slot-based (header, body, footer) |
| `dialog.rs` | `Dialog` | DialogSize: Sm/Md/Lg |
| `table.rs` | `Table` | SelectionMode, SortDirection |
| `wizard.rs` | `Wizard` | StepStatus: Complete/Current/Pending/Error |
| `avatar.rs` | `Avatar` | AvatarSize, AvatarShape: Circle/Square, AvatarStatus |

### Layout Components
| File | Component |
|------|-----------|
| `stack.rs` | `VStack`, `HStack` — StackSpacing: None/Xs/Sm/Md/Lg/Xl/Xxl, StackAlign, StackJustify |
| `divider.rs` | `Divider` |
| `pane_divider.rs` | `PaneDivider` |

### Audio Components
| File | Component |
|------|-----------|
| `audio/potentiometer.rs` | `Potentiometer` — PotentiometerSize: Xs/Sm/Md/Lg |
| `audio/vertical_slider.rs` | `VerticalSlider` |
| `audio/volume_knob.rs` | `VolumeKnob` |

## 3. Framework

- **UI Framework**: GPUI (Zed's GPU-accelerated UI framework for Rust)
- **Styling**: Inline via GPUI's fluent API (`div().bg(color).px(px(16)).rounded(px(6))`)
- **No CSS** — all styling is Rust code
- **Build system**: Cargo (Rust)

## 4. Component Architecture Pattern

Builder pattern with fluent API:

```rust
// Creating a button
Button::new("save-btn", "Save")
    .variant(ButtonVariant::Primary)
    .size(ButtonSize::Md)
    .on_click(|_, cx| { /* handler */ })

// Creating a stack layout
VStack::new()
    .spacing(StackSpacing::Md)
    .align(StackAlign::Stretch)
    .child(Button::new("btn1", "Click"))
    .child(Input::new("input1").placeholder("Type..."))

// Creating a toggle
Toggle::new("enable-toggle", is_checked)
    .style(ToggleStyle::Sliding)
    .size(ToggleSize::Md)
    .label("Enable")
    .on_toggle(|checked, cx| { /* handler */ })
```

All components implement GPUI's `RenderOnce` or `Render` trait and are composed via `.child()` calls.

## 5. Figma-to-GPUI Translation Rules

When translating from Figma designs:

1. **Auto Layout → VStack/HStack**: Vertical auto layout = `VStack`, horizontal = `HStack`. Map gap to `StackSpacing`.
2. **Fill container → `.flex_grow()`**: Figma "Fill" = `flex_grow()` in GPUI.
3. **Fixed size → `.w(px(N)).h(px(N))`**: Direct pixel mapping.
4. **Corner radius → `.rounded(px(N))`**: Map to border radius tokens.
5. **Colors → Theme fields**: Map Figma fills to theme color fields, never hardcode hex in components.
6. **Component instances → Builder calls**: Map Figma component variants to enum values in the builder pattern.
7. **Text → `Text::new("content").size(TextSize::Md).weight(TextWeight::Normal)`**
8. **Padding → `.px(px(N)).py(px(N))`**: Map Figma padding to GPUI padding calls.

## 6. Icon System

Icons are from the `gpui-icons` crate at `crates/gpui-toolkit/gpui-icons/`. SVG-based, referenced by enum variants. Used via `IconButton` or inline in components.

## 7. Asset Management

No CDN. Assets are embedded at compile time or loaded from local filesystem. Images use GPUI's image rendering APIs.

## 8. Project Structure

```
crates/gpui-toolkit/
  gpui-ui-kit/src/     # All UI components (this design system)
  gpui-icons/          # Icon library
  gpui-d3rs/           # D3-style charting for GPUI
crates/app-gpui/       # The SOTF desktop app using gpui-ui-kit
  components/          # App-specific component compositions
  app/state/           # Application state management
  ui/                  # UI layout and screens
```

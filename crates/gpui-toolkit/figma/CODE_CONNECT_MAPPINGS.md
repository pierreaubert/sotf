# Code Connect Mappings — SOTF gpui-toolkit

Figma file: https://www.figma.com/design/c7bS9PCykhmP2pjpaT0zKu
File key: `c7bS9PCykhmP2pjpaT0zKu`

## Mappings to apply via Figma MCP `add_code_connect_map`

All use `label: "Swift"` (closest to Rust in the allowed list).

| Node ID | Component Name | Source File |
|---------|---------------|-------------|
| 1:289 | Button | crates/gpui-toolkit/gpui-ui-kit/src/button.rs |
| 1:455 | Input | crates/gpui-toolkit/gpui-ui-kit/src/input.rs |
| 1:493 | NumberInput | crates/gpui-toolkit/gpui-ui-kit/src/number_input.rs |
| 1:531 | Select | crates/gpui-toolkit/gpui-ui-kit/src/select.rs |
| 1:569 | Checkbox | crates/gpui-toolkit/gpui-ui-kit/src/checkbox.rs |
| 1:611 | Toggle | crates/gpui-toolkit/gpui-ui-kit/src/toggle.rs |
| 1:655 | Slider | crates/gpui-toolkit/gpui-ui-kit/src/slider.rs |
| 1:685 | Badge | crates/gpui-toolkit/gpui-ui-kit/src/badge.rs |
| 1:734 | Alert | crates/gpui-toolkit/gpui-ui-kit/src/alert.rs |
| 1:786 | InlineAlert | crates/gpui-toolkit/gpui-ui-kit/src/alert.rs |
| 1:803 | Toast | crates/gpui-toolkit/gpui-ui-kit/src/toast.rs |
| 1:841 | Progress | crates/gpui-toolkit/gpui-ui-kit/src/progress.rs |
| 1:873 | CircularProgress | crates/gpui-toolkit/gpui-ui-kit/src/progress.rs |
| 1:892 | Spinner | crates/gpui-toolkit/gpui-ui-kit/src/spinner.rs |
| 1:923 | Tabs | crates/gpui-toolkit/gpui-ui-kit/src/tabs.rs |
| 1:956 | ButtonSet | crates/gpui-toolkit/gpui-ui-kit/src/button_set.rs |
| 1:984 | Breadcrumbs | crates/gpui-toolkit/gpui-ui-kit/src/breadcrumbs.rs |
| 1:1021 | Menu | crates/gpui-toolkit/gpui-ui-kit/src/menu.rs |
| 1:1072 | Accordion | crates/gpui-toolkit/gpui-ui-kit/src/accordion.rs |
| 1:1103 | Tooltip | crates/gpui-toolkit/gpui-ui-kit/src/tooltip.rs |
| 1:1126 | Card | crates/gpui-toolkit/gpui-ui-kit/src/card.rs |
| 1:354 | IconButton | crates/gpui-toolkit/gpui-ui-kit/src/icon_button.rs |
| audio:potentiometer | Potentiometer | crates/gpui-toolkit/gpui-audio-kit/src/audio/potentiometer.rs |
| audio:vertical-slider | VerticalSlider | crates/gpui-toolkit/gpui-audio-kit/src/audio/vertical_slider.rs |
| audio:volume-knob | VolumeKnob | crates/gpui-toolkit/gpui-audio-kit/src/audio/volume_knob.rs |
| audio:level-meter | LevelMeterElement | crates/gpui-toolkit/gpui-audio-kit/src/meter.rs |
| audio:spectrum | SpectrumElement | crates/gpui-toolkit/gpui-audio-kit/src/spectrum.rs |

The runtime story registry for design review and responsive matrices lives in
`crates/gpui-toolkit/gpui-component-lab`. Token exports and validation live in
`crates/gpui-toolkit/gpui-design-tools`.

## Code Connect Examples

When `get_design_context` returns a Button instance, generate:
```rust
Button::new("id", "Label")
    .variant(ButtonVariant::Primary)  // from Figma variant property
    .size(ButtonSize::Md)             // from Figma size property
```

When `get_design_context` returns an Input instance, generate:
```rust
Input::new("id")
    .placeholder("Type here...")
    .variant(InputVariant::Default)
    .size(InputSize::Md)
```

When `get_design_context` returns a Toggle, generate:
```rust
Toggle::new("id", checked)
    .style(ToggleStyle::Sliding)
    .size(ToggleSize::Md)
    .label("Label")
```

When `get_design_context` returns a Badge, generate:
```rust
Badge::new("text")
    .variant(BadgeVariant::Primary)
    .size(BadgeSize::Md)
```

When `get_design_context` returns an Alert, generate:
```rust
Alert::new("id", "Message")
    .variant(AlertVariant::Info)
    .title("Title")
    .closeable(true)
```

When `get_design_context` returns Tabs, generate:
```rust
Tabs::new("id", vec![
    TabItem::new("tab1", "General"),
    TabItem::new("tab2", "Audio"),
])
.variant(TabVariant::Underline)
```

When `get_design_context` returns a Select, generate:
```rust
Select::new("id", vec![
    SelectOption::new("opt1", "Option One"),
    SelectOption::new("opt2", "Option Two"),
])
.size(SelectSize::Md)
```

When `get_design_context` returns a VStack/HStack layout, generate:
```rust
VStack::new()
    .spacing(StackSpacing::Md)
    .align(StackAlign::Stretch)
    .children(vec![...])
```

When `get_design_context` returns an audio control, generate imports from
`gpui_audio_kit`:
```rust
use gpui_audio_kit::{AudioScale, Potentiometer, PotentiometerSize};

Potentiometer::new("frequency")
    .label("Frequency")
    .value(1000.0)
    .min(20.0)
    .max(20_000.0)
    .scale(AudioScale::Logarithmic)
    .size(PotentiometerSize::Md)
```

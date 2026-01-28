# GPUI Development Guide

This guide covers patterns and best practices for developing GPUI-based UI components in the SOTF project.

## Form Components Guide

This section documents the patterns required to get form components (inputs, dropdowns, selects) working correctly with both mouse and keyboard interactions.

### Key Principle: Every Dropdown Needs TWO Handlers

For dropdown/select components to work properly, you MUST provide BOTH:
1. `on_*_change` - Handles value selection
2. `on_*_toggle` - Handles open/close state

**CRITICAL**: The toggle handler MUST call `cx.notify()` to trigger re-render.

### Correct Pattern (from spinorama_eq)

```rust
.on_algo_change({
    let state = self.state.clone();
    move |algo, _window, cx| {
        state.update(cx, |state, _cx| {
            // Update the value
            state.app.some_state.config.algorithm = parse_algo(algo);
            // Close the dropdown after selection
            state.app.some_state.dropdowns.algorithm_open = false;
        });
    }
})
.on_algo_toggle({
    let state = self.state.clone();
    move |open, _window, cx| {
        state.update(cx, |state, cx| {  // NOTE: Use cx, not _cx
            // Update the open state
            state.app.some_state.dropdowns.algorithm_open = open;
            // CRITICAL: Must call cx.notify() to trigger re-render
            cx.notify();
        });
    }
})
```

### Incorrect Pattern (causes dropdowns to not respond)

```rust
// WRONG - Missing cx.notify() in toggle handler
.on_algo_toggle({
    let state = self.state.clone();
    move |open, _window, cx| {
        state.update(cx, |state, _cx| {  // _cx is unused - BAD SIGN
            state.app.some_state.dropdowns.algorithm_open = open;
            // Missing cx.notify() - dropdown won't update!
        });
    }
})
```

### NumberInput Component

NumberInput components work differently - they manage their own editing state via thread-local storage. The key pattern:

```rust
NumberInput::new("unique-id")
    .value(current_value)
    .min(0.0)
    .max(100.0)
    .step(1.0)
    .on_change({
        let state = self.state.clone();
        move |new_value, _window, cx| {
            state.update(cx, |state, _cx| {
                state.app.some_state.config.value = new_value;
            });
        }
    })
```

NumberInput supports:
- Click to start editing
- Double-click to select all
- Enter to confirm
- Escape to cancel
- Arrow Up/Down to increment/decrement

### State Structure for Dropdowns

Each component using dropdowns needs a separate state struct for tracking open states:

```rust
#[derive(Clone, Default)]
pub struct MyDropdownStates {
    pub algorithm_open: bool,
    pub peq_model_open: bool,
    pub strategy_open: bool,
    // ... one field per dropdown
}
```

### Preventing Global Shortcuts During Input

Wrap form components in a div that stops key event propagation:

```rust
.child(
    div()
        .on_key_down(|_event, _window, cx| {
            cx.stop_propagation();
        })
        .child(autoeq_form),
)
```

### AutoEqForm Complete Handler List

When using `AutoEqForm`, these are all the handlers you may need to wire up:

**Dropdowns (need both `_change` and `_toggle`):**
- `on_opt_mode_change` / `on_opt_mode_toggle`
- `on_peq_model_change` / `on_peq_model_toggle`
- `on_algo_change` / `on_algo_toggle`
- `on_strategy_change` / `on_strategy_toggle`
- `on_local_algo_change` / `on_local_algo_toggle`
- `on_fir_phase_change` / `on_fir_phase_toggle`
- `on_loss_type_change` / `on_loss_type_toggle`
- `on_target_curve_change` / `on_target_curve_toggle`
- `on_system_type_change` / `on_system_type_toggle`

**NumberInputs (only need `_change`):**
- `on_num_filters_change`
- `on_min_q_change` / `on_max_q_change`
- `on_min_db_change` / `on_max_db_change`
- `on_min_freq_change` / `on_max_freq_change`
- `on_maxeval_change`
- `on_population_change`
- `on_de_f_change` / `on_de_cr_change`
- `on_tolerance_change` / `on_atolerance_change`
- `on_smooth_change` / `on_smooth_n_change`
- `on_spacing_weight_change`
- `on_min_spacing_oct_change`
- `on_sample_rate_change`
- `on_fir_taps_change`
- `on_refine_change` (boolean toggle)

### Plugin UI Components (Potentiometer, Slider)

For plugin UIs using `Potentiometer` or `VerticalSlider`:

```rust
fn render_knob(
    label: &str,
    value: f64,
    min: f64,
    max: f64,
    entity: &Entity<AppState>,
    plugin_idx: usize,
    param_idx: usize,
) -> impl IntoElement {
    Potentiometer::new(format!("knob-{}-{}", plugin_idx, param_idx))
        .value(normalize(value, min, max))
        .on_change({
            let entity = entity.clone();
            move |new_value, _, cx| {
                entity.update(cx, |state, _| {
                    let denorm = denormalize(new_value, min, max);
                    state.app.set_plugin_param(plugin_idx, param_idx, denorm);
                });
            }
        })
}
```

### Debugging Form Issues

If a form element isn't responding:

1. **Dropdown won't open**: Check if `on_*_toggle` handler exists and calls `cx.notify()`
2. **Dropdown won't close after selection**: Check if `on_*_change` sets the `*_open` flag to `false`
3. **Input not editable**: Check if the form is wrapped in a div with `on_key_down` that stops propagation
4. **Value not updating**: Check if `on_change` handler updates the correct state field
5. **UI not reflecting changes**: Ensure `cx.notify()` is called in the appropriate handler

### Reference Implementation

See `crates/app-gpui/components/spinorama_eq/step_2_configure.rs` as the canonical reference for a fully working form implementation with all handlers properly wired.

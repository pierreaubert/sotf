use gpui::*;

/// Callback type for string parameter changes
pub(crate) type StringCallback = Box<dyn Fn(&str, &mut Window, &mut App) + 'static>;

/// Callback type for f64 parameter changes
pub(crate) type F64Callback = Box<dyn Fn(f64, &mut Window, &mut App) + 'static>;

/// Callback type for usize parameter changes
pub(crate) type UsizeCallback = Box<dyn Fn(usize, &mut Window, &mut App) + 'static>;

/// Callback type for bool parameter changes
pub(crate) type BoolCallback = Box<dyn Fn(bool, &mut Window, &mut App) + 'static>;

/// Callback type for dropdown toggle
pub(crate) type ToggleCallback = Box<dyn Fn(bool, &mut Window, &mut App) + 'static>;

pub(crate) type ActionCallback = Box<dyn Fn(&mut Window, &mut App) + 'static>;

/// Layout mode for the AutoEQ form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AutoEqLayoutMode {
    /// Original card-based layout (headphone EQ, spinorama EQ)
    #[default]
    Default,
    /// Room EQ layout: 3 sections (Optimisation Mode, Room Configuration, Optimiser Configuration)
    RoomEq,
}

//! Declarative Plugin Layout System
//!
//! Platform-agnostic layout descriptors for plugin UIs. Each plugin defines a
//! `PluginLayout` that describes what controls go where (columns, tabs, visualizations).
//! The layout solver (`layout_solver.rs`) decides which columns are visible vs.
//! collapsed to tabs based on available space. Platform renderers (GPUI, SwiftUI)
//! read the solved layout and emit native UI.
//!
//! This module contains only data types — no rendering code, no framework deps.

// ============================================================================
// Column Constraints (solver input)
// ============================================================================

/// Identifies a column's role in the 3-column layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnRole {
    /// Left column: structural params, mode selectors, channel config.
    Config,
    /// Center column: main controls. Never collapses.
    Main,
    /// Right column: meters, output gain, mix.
    Output,
    /// Right-of-main column: diagnostic displays. First to collapse.
    Diagnostic,
}

/// Describes a column's size preferences and collapse behavior for the solver.
#[derive(Debug, Clone, Copy)]
pub struct ColumnConstraint {
    pub role: ColumnRole,
    /// Minimum width in pixels to display as a visible column.
    pub min_width: f32,
    /// Ideal width when space is available.
    pub preferred_width: f32,
    /// Maximum width (fixed columns stay at preferred; Main grows).
    pub max_width: f32,
    /// Priority for staying visible: 0.0 = collapses first, 1.0 = never collapses.
    /// Main should always be 1.0 with `collapsible: false`.
    pub priority: f32,
    /// Whether this column can be collapsed into a tab when space is tight.
    pub collapsible: bool,
}

impl ColumnConstraint {
    pub const fn config(min_width: f32, priority: f32) -> Self {
        Self {
            role: ColumnRole::Config,
            min_width,
            preferred_width: min_width,
            max_width: min_width,
            priority,
            collapsible: true,
        }
    }

    pub const fn main(min_width: f32) -> Self {
        Self {
            role: ColumnRole::Main,
            min_width,
            preferred_width: 500.0,
            max_width: f32::MAX,
            priority: 1.0,
            collapsible: false,
        }
    }

    pub const fn output(min_width: f32, priority: f32) -> Self {
        Self {
            role: ColumnRole::Output,
            min_width,
            preferred_width: min_width,
            max_width: min_width,
            priority,
            collapsible: true,
        }
    }

    pub const fn diagnostic(min_width: f32, priority: f32) -> Self {
        Self {
            role: ColumnRole::Diagnostic,
            min_width,
            preferred_width: min_width,
            max_width: min_width,
            priority,
            collapsible: true,
        }
    }
}

// ============================================================================
// Control Specifications
// ============================================================================

/// How a parameter should be rendered in the UI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControlType {
    /// Standard rotary knob.
    Knob,
    /// Larger knob (1.5x) for primary single-param plugins like Gain.
    KnobLarge,
    /// Vertical slider with tick marks. Standard height 180px, compact 120px.
    VerticalSlider,
    /// On/off or labeled toggle switch.
    Toggle,
    /// Mutually exclusive button group.
    ButtonSet { labels: &'static [&'static str] },
    /// Dropdown/choice selector.
    Selector,
    /// Read-only bar meter (e.g., gain reduction).
    BarMeter { min_db: f64, max_db: f64 },
    /// Read-only text label displaying a value.
    Label,
    /// File picker with load button + filename display.
    FilePicker,
}

/// Specification for a single control in the layout.
#[derive(Debug, Clone, Copy)]
pub struct ControlSpec {
    /// Index into the plugin's PARAMS array.
    pub param_index: usize,
    /// How to render this parameter.
    pub control_type: ControlType,
    /// If true, the control is display-only (no user interaction).
    pub read_only: bool,
    /// If true, the renderer skips this control entirely. Used when the
    /// param is already exposed elsewhere in the chrome (e.g. upmixer's
    /// `speaker_config` is shown by the rack-level Output dropdown).
    /// `validate_coverage` still counts it as referenced, so PARAMS-vs-LAYOUT
    /// audits keep passing.
    pub hidden: bool,
}

impl ControlSpec {
    pub const fn knob(param_index: usize) -> Self {
        Self {
            param_index,
            control_type: ControlType::Knob,
            read_only: false,
            hidden: false,
        }
    }

    pub const fn knob_large(param_index: usize) -> Self {
        Self {
            param_index,
            control_type: ControlType::KnobLarge,
            read_only: false,
            hidden: false,
        }
    }

    pub const fn slider(param_index: usize) -> Self {
        Self {
            param_index,
            control_type: ControlType::VerticalSlider,
            read_only: false,
            hidden: false,
        }
    }

    pub const fn toggle(param_index: usize) -> Self {
        Self {
            param_index,
            control_type: ControlType::Toggle,
            read_only: false,
            hidden: false,
        }
    }

    pub const fn selector(param_index: usize) -> Self {
        Self {
            param_index,
            control_type: ControlType::Selector,
            read_only: false,
            hidden: false,
        }
    }

    pub const fn choice(param_index: usize) -> Self {
        Self {
            param_index,
            control_type: ControlType::Selector,
            read_only: false,
            hidden: false,
        }
    }

    pub const fn meter(min_db: f64, max_db: f64) -> Self {
        Self {
            // param_index is unused for meters — they read from viz_data
            param_index: usize::MAX,
            control_type: ControlType::BarMeter { min_db, max_db },
            read_only: true,
            hidden: false,
        }
    }

    pub const fn label(param_index: usize) -> Self {
        Self {
            param_index,
            control_type: ControlType::Label,
            read_only: true,
            hidden: false,
        }
    }

    pub const fn file_picker(param_index: usize) -> Self {
        Self {
            param_index,
            control_type: ControlType::FilePicker,
            read_only: false,
            hidden: false,
        }
    }

    pub const fn button_set(param_index: usize, labels: &'static [&'static str]) -> Self {
        Self {
            param_index,
            control_type: ControlType::ButtonSet { labels },
            read_only: false,
            hidden: false,
        }
    }

    /// Mark a control as hidden — the validator still sees the param as
    /// referenced, but the renderer skips it. Use when the parameter is
    /// surfaced by a chrome-level control (e.g. the rack-level Output
    /// dropdown for the upmixer's `speaker_config`).
    pub const fn hide(mut self) -> Self {
        self.hidden = true;
        self
    }
}

// ============================================================================
// Layout Structure
// ============================================================================

/// A named group of controls rendered together (e.g., "DYNAMICS", "TIMING").
#[derive(Debug, Clone, Copy)]
pub struct ControlGroup {
    /// Stable, non-localized identity used by responsive layout and state.
    pub id: &'static str,
    /// Section title displayed above the group (e.g., "DYNAMICS").
    pub title: &'static str,
    /// Controls within this group.
    pub controls: &'static [ControlSpec],
    /// Optional responsive layout overrides.
    pub layout: GroupLayoutHints,
}

/// Whether a control group may move into the responsive overflow surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroupOverflow {
    /// Allow the constraint solver to collapse this group atomically.
    #[default]
    Auto,
    /// Keep this group visible at every width.
    KeepVisible,
}

/// Optional sizing and collapse overrides for a control group.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroupLayoutHints {
    /// Minimum visible width. Inferred from control types when omitted.
    pub min_width: Option<f32>,
    /// Preferred visible width. Inferred from control types when omitted.
    pub preferred_width: Option<f32>,
    /// Priority for staying visible, from 0 (first to collapse) to 1.
    pub collapse_priority: f32,
    /// Whether this group may move into overflow.
    pub overflow: GroupOverflow,
}

impl GroupLayoutHints {
    /// Inferred sizing, declaration-order collapse, and automatic overflow.
    pub const fn inferred() -> Self {
        Self {
            min_width: None,
            preferred_width: None,
            collapse_priority: 0.5,
            overflow: GroupOverflow::Auto,
        }
    }

    /// Override inferred minimum and preferred widths.
    pub const fn widths(mut self, min_width: f32, preferred_width: f32) -> Self {
        self.min_width = Some(min_width);
        self.preferred_width = Some(preferred_width);
        self
    }

    /// Override collapse priority.
    pub const fn priority(mut self, priority: f32) -> Self {
        self.collapse_priority = priority;
        self
    }

    /// Keep the group on the primary surface at every width.
    pub const fn keep_visible(mut self) -> Self {
        self.overflow = GroupOverflow::KeepVisible;
        self
    }
}

impl Default for GroupLayoutHints {
    fn default() -> Self {
        Self::inferred()
    }
}

impl ControlGroup {
    /// Define a group with inferred responsive sizing.
    pub const fn new(
        id: &'static str,
        title: &'static str,
        controls: &'static [ControlSpec],
    ) -> Self {
        Self {
            id,
            title,
            controls,
            layout: GroupLayoutHints::inferred(),
        }
    }

    /// Override inferred responsive layout behavior.
    pub const fn with_layout(mut self, layout: GroupLayoutHints) -> Self {
        self.layout = layout;
        self
    }
}

/// A tab section at the bottom of the plugin UI.
#[derive(Debug, Clone, Copy)]
pub struct TabSpec {
    /// Tab label (e.g., "LFE & Bass", "Advanced").
    pub name: &'static str,
    /// Controls displayed when this tab is active.
    pub controls: &'static [ControlSpec],
}

/// Position of a visualization within the layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VizPosition {
    /// Below a specific control group (matched by title).
    BelowGroup(&'static str),
    /// Spans the entire center column.
    FullCenter,
}

/// A visualization slot in the plugin layout.
#[derive(Debug, Clone, Copy)]
pub enum VizSlot {
    /// Input/output transfer curve (compressor, limiter, gate, expander).
    TransferCurve { position: VizPosition },
    /// Frequency response graph (EQ).
    FrequencyResponse { position: VizPosition },
    /// Named slot for platform-specific custom rendering.
    Custom {
        name: &'static str,
        position: VizPosition,
    },
}

/// Stable names for `VizSlot::Custom` viz types. Plugins opt into a custom
/// renderer by referencing one of these constants rather than typing the
/// raw string — typos surface at compile time instead of silently rendering
/// nothing.
pub mod viz_names {
    /// Spatial spider (per-channel SPL / inter-channel correlation).
    /// Recognised by the `app-gpui` layout renderer.
    pub const SPATIAL_SPIDER: &str = "spatial_spider";
}

/// Describes a repeated parameter section (one per band/filter).
///
/// Used by plugins with a variable number of identical parameter groups,
/// such as multiband compressor bands or EQ filter bands.
#[derive(Debug, Clone, Copy)]
pub struct DynamicSection {
    /// Instance label (e.g., "Band", "Filter").
    pub instance_name: &'static str,
    /// Template param indices repeated for each instance (indices into PARAMS).
    pub template_params: &'static [usize],
    /// Control layout for one instance of the template.
    pub template_controls: &'static [ControlSpec],
    /// Allowed range of instance counts (min, max).
    pub count_range: (usize, usize),
    /// PARAMS index that controls the instance count (None = fixed count).
    pub count_param_index: Option<usize>,
    /// Whether to show "Global" as instance 0 (for override-style multiband).
    pub has_global_defaults: bool,
}

/// Complete declarative layout for a plugin UI.
///
/// Each plugin defines a `static LAYOUT: PluginLayout` alongside its `PARAMS` array.
/// The layout is pure data — no rendering code, no framework dependencies.
/// Platform renderers (GPUI, SwiftUI) read this + the solver output to build UI.
#[derive(Debug)]
pub struct PluginLayout {
    /// Left column controls (Config/Setup). Empty slice = no config column.
    pub config: &'static [ControlSpec],
    /// Center area control groups. Groups render side-by-side when wide, stacked when narrow.
    pub main: &'static [ControlGroup],
    /// Right column controls (Output/Meters). Empty slice = no output column.
    pub output: &'static [ControlSpec],
    /// Bottom tab sections. Empty slice = no tabs.
    /// Note: columns that collapse also become tabs (appended by the solver).
    pub tabs: &'static [TabSpec],
    /// Visualization slots embedded in the layout.
    pub visualizations: &'static [VizSlot],
    /// Column constraints for the layout solver.
    pub column_constraints: &'static [ColumnConstraint],
    /// Dynamic (repeated) parameter sections. Empty slice = no dynamic sections.
    pub dynamic_sections: &'static [DynamicSection],
}

impl PluginLayout {
    /// Returns true if this layout has a config (left) column.
    pub fn has_config(&self) -> bool {
        !self.config.is_empty()
    }

    /// Returns true if this layout has an output (right) column.
    pub fn has_output(&self) -> bool {
        !self.output.is_empty()
    }

    /// Returns true if this layout has any tabs.
    pub fn has_tabs(&self) -> bool {
        !self.tabs.is_empty()
    }

    /// Returns true if this layout has any visualizations.
    pub fn has_visualizations(&self) -> bool {
        !self.visualizations.is_empty()
    }

    /// Validate that all param_index values in this layout are within bounds
    /// of the given PARAMS array. Returns a list of errors (empty = valid).
    ///
    /// This catches the class of bug where a layout built for one PARAMS array
    /// (e.g., multiband GLOBAL_PARAMS) is accidentally used with a different
    /// one (e.g., single-band PARAMS) that has different index assignments.
    pub fn validate(&self, params_len: usize, plugin_name: &str) -> Vec<String> {
        let mut errors = Vec::new();
        let check = |idx: usize, section: &str, errors: &mut Vec<String>| {
            if idx >= params_len {
                errors.push(format!(
                    "{plugin_name}: {section} references param index {idx} but PARAMS only has {params_len} entries"
                ));
            }
        };
        let check_controls = |controls: &[ControlSpec], section: &str, errors: &mut Vec<String>| {
            for c in controls {
                if let ControlType::BarMeter { .. } = c.control_type {
                    continue; // meters don't reference params
                }
                check(c.param_index, section, errors);
            }
        };
        check_controls(self.config, "config", &mut errors);
        for group in self.main {
            check_controls(
                group.controls,
                &format!("main/{}", group.title),
                &mut errors,
            );
        }
        check_controls(self.output, "output", &mut errors);
        for tab in self.tabs {
            check_controls(tab.controls, &format!("tab/{}", tab.name), &mut errors);
        }
        for ds in self.dynamic_sections {
            check_controls(
                ds.template_controls,
                &format!("dynamic/{}", ds.instance_name),
                &mut errors,
            );
        }
        errors
    }

    /// Collect all param indices referenced by this layout (config, main, output,
    /// tabs, dynamic_sections). Returns a sorted, deduplicated Vec.
    pub fn referenced_indices(&self) -> Vec<usize> {
        let mut indices = Vec::new();
        let collect = |controls: &[ControlSpec], out: &mut Vec<usize>| {
            for c in controls {
                if let ControlType::BarMeter { .. } = c.control_type {
                    continue;
                }
                out.push(c.param_index);
            }
        };
        collect(self.config, &mut indices);
        for group in self.main {
            collect(group.controls, &mut indices);
        }
        collect(self.output, &mut indices);
        for tab in self.tabs {
            collect(tab.controls, &mut indices);
        }
        for ds in self.dynamic_sections {
            collect(ds.template_controls, &mut indices);
            if let Some(idx) = ds.count_param_index {
                indices.push(idx);
            }
        }
        indices.sort_unstable();
        indices.dedup();
        indices
    }

    /// Validate that every PARAMS index appears in the layout. Returns errors
    /// listing any param indices that are in PARAMS but missing from the layout.
    pub fn validate_coverage(
        &self,
        params: &[crate::param_specs::ParamSpec],
        plugin_name: &str,
    ) -> Vec<String> {
        let referenced = self.referenced_indices();
        let mut errors = Vec::new();
        for (idx, param) in params.iter().enumerate() {
            if !referenced.contains(&idx) {
                errors.push(format!(
                    "{plugin_name}: param index {idx} ({}) is in PARAMS but missing from LAYOUT",
                    param.name
                ));
            }
        }
        errors
    }
}

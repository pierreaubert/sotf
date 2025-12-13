//! GPUI Themes - Theme editor and management for GPUI applications
//!
//! This crate provides:
//! - Serializable theme types with JSON and Rust code export
//! - A color picker component for editing colors
//! - A component showcase for previewing theme changes
//! - A theme editor application

mod color_picker;
mod editor;
mod showcase;
mod theme;

pub use color_picker::ColorPickerView;
pub use editor::ThemeEditor;
pub use showcase::ComponentShowcase;
pub use theme::{
    Color, ColorGroup, EQCurveColors, EditorTheme, GraphColors, MeterColors, PluginColors,
    SpectrumColors,
};

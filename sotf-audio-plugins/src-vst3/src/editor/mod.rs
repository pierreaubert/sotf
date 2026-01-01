//! EQ Plugin Editor
//!
//! Implements the nih-plug Editor trait with egui-based visualization.

mod eq_view;

use crate::eq_params::SotfEqParams;
use eq_view::EqEditorState;
use nih_plug::prelude::*;
use nih_plug_egui::egui;
use nih_plug_egui::{create_egui_editor, EguiState};
use std::sync::Arc;

/// Editor window dimensions
const EDITOR_WIDTH: u32 = 800;
const EDITOR_HEIGHT: u32 = 500;

/// Create the EQ plugin editor
pub fn create_editor(
    params: Arc<SotfEqParams>,
    egui_state: Arc<EguiState>,
) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        egui_state,
        EqEditorState::new(params),
        |ctx, _| {
            // Apply custom fonts and styling
            setup_custom_style(ctx);
        },
        move |ctx, setter, state| {
            state.ui(ctx, setter);
        },
    )
}

/// Create the egui state for persistence
pub fn create_egui_state() -> Arc<EguiState> {
    EguiState::from_size(EDITOR_WIDTH, EDITOR_HEIGHT)
}

/// Set up custom egui styling
fn setup_custom_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // Use a dark theme
    style.visuals = egui::Visuals::dark();

    // Customize colors
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(40, 40, 45);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(50, 50, 55);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(60, 60, 70);

    // Panel background
    style.visuals.panel_fill = egui::Color32::from_rgb(25, 25, 30);
    style.visuals.window_fill = egui::Color32::from_rgb(30, 30, 35);

    // Accent color
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(80, 120, 200);

    ctx.set_style(style);
}

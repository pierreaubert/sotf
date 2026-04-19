//! Recording wizard — Step 4: Bass Anchor (GD-Opt v2 Phase GD-1e).
//!
//! Plays a 20 Hz × 5-cycle Hann-windowed tone burst sequentially on
//! each output channel, records the mic, and extracts the per-channel
//! phase + stability of the burst's fundamental via a single-bin DFT.
//! The result anchors the sweep-derived phase at the first bin,
//! eliminating the 2π wraparound ambiguity that plagues log-sweep
//! bass measurements (§2.6 of `docs/gd_opt_v2_plan.md`).
//!
//! This renderer mirrors the Probe step (`probe.rs`) — display current
//! status, expose a Start/Cancel control, render a per-channel result
//! table when Complete. Live capture wiring (engine spawn +
//! apply_results) lands alongside this step; for review purposes the
//! step is optional and can be skipped via the wizard's Next button.

use crate::app::types::recording::BassAnchorCaptureStatus;
use crate::ui::PlayerView;
use gpui::{Context, IntoElement};
use gpui_ui_kit::{Card, StackSpacing, Text, TextSize, TextWeight, VStack};

impl PlayerView {
    /// Render the BassAnchor step UI.
    pub(crate) fn render_recording_bass_anchor_step(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let rec = &state.app.measurement_state.recording_state;
        let bac = rec.bass_anchor_capture.clone();

        let status_line = match &bac.status {
            BassAnchorCaptureStatus::Idle => {
                format!(
                    "Not started — {:.0} Hz × {} cycles ({:.0} ms / channel)",
                    bac.bass_freq_hz,
                    bac.bass_cycles,
                    1000.0 * bac.bass_cycles as f32 / bac.bass_freq_hz
                )
            }
            BassAnchorCaptureStatus::Running { .. } => "Capturing bass anchor…".to_string(),
            BassAnchorCaptureStatus::Complete => "Complete".to_string(),
            BassAnchorCaptureStatus::Failed(e) => format!("Failed: {e}"),
        };

        let mut column = VStack::new()
            .spacing(StackSpacing::Sm)
            .child(
                Text::new("Bass Anchor")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Md),
            )
            .child(
                Text::new(
                    "Plays a low-frequency tone burst per channel so GD-Opt v2 can anchor the \
                     first bass bin of the sweep-derived phase. Optional — skip with Next if \
                     your system doesn't support sub-bass playback.",
                )
                .size(TextSize::Sm),
            )
            .child(Text::new(status_line).size(TextSize::Sm));

        if let Some(results) = bac.results.as_ref() {
            column = column.child(
                Text::new(format!(
                    "Channels captured: {} @ {} Hz",
                    results.channels.len(),
                    results.sample_rate
                ))
                .size(TextSize::Sm),
            );
            for ch in &results.channels {
                let reliable = ch.bass_anchor_stability_deg < 20.0;
                let line = format!(
                    "  {} — phase {:+.1}°, |mag| {:.3}, stability {:.1}°{}",
                    ch.channel_name,
                    ch.bass_anchor_phase_deg,
                    ch.bass_anchor_magnitude,
                    ch.bass_anchor_stability_deg,
                    if reliable { "" } else { "  ⚠ unreliable (> 20°)" }
                );
                column = column.child(Text::new(line).size(TextSize::Sm));
            }
        }

        Card::new()
            .background(theme.surface)
            .border(theme.border)
            .content(column)
    }
}

use super::hint_id::HintId;
use crate::components::design::Ds;
use gpui::prelude::*;
use gpui::*;

pub(super) struct TutorialScreen {
    pub(super) title: &'static str,
    pub(super) image: &'static str,
    pub(super) content: &'static [&'static str],
}

/// Contextual hint state — shown as a dismissible banner at the top of the relevant screen.
#[derive(Debug, Clone)]
pub struct ContextualHint {
    pub hint_id: HintId,
}

/// Render a contextual hint banner (dismissible callout).
pub fn render_hint_banner(
    hint: &ContextualHint,
    theme: &crate::theme::Theme,
    d: Ds,
) -> impl IntoElement {
    let title = hint.hint_id.title();
    let message = hint.hint_id.message();

    div()
        .flex()
        .items_start()
        .gap(d.gap_md)
        .px(d.card)
        .py(d.pad_x)
        .mx(d.card)
        .mt(d.gap)
        .rounded(d.r_lg)
        .bg(theme.feedback.toast_info_bg)
        .border_1()
        .border_color(theme.info)
        .child(
            div()
                .text_size(d.text_sm)
                .text_color(theme.info)
                .font_weight(FontWeight::BOLD)
                .child("\u{1f4a1}"),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(d.grid)
                .child(
                    div()
                        .text_size(d.text_sm)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(d.text_xs)
                        .text_color(theme.text_secondary)
                        .child(message),
                ),
        )
}

pub(super) struct GuideSection {
    pub(super) heading: &'static str,
    pub(super) bullets: &'static [&'static str],
}

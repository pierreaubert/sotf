use super::hint_id::HintId;
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName, IconSize};
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{IconButton, IconButtonSize, IconButtonVariant};

/// Contextual hint state — shown as a dismissible banner at the top of the relevant screen.
#[derive(Debug, Clone)]
pub struct ContextualHint {
    pub hint_id: HintId,
}

/// Render a contextual hint banner (dismissible callout).
///
/// Single-line layout: bulb, bold title, then the message which truncates
/// with an ellipsis when the window is too narrow. The X button on the right
/// invokes `on_close` (dismiss + persist).
pub fn render_hint_banner(
    hint: &ContextualHint,
    theme: &crate::theme::Theme,
    d: Ds,
    dismiss_label: &'static str,
    on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let title = hint.hint_id.title();
    let message = hint.hint_id.message();

    div()
        .flex()
        .items_center()
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
                .min_w_0()
                .flex()
                .items_baseline()
                .gap(d.grid)
                .child(
                    div()
                        .text_size(d.text_sm)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .whitespace_nowrap()
                        .child(title),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(d.text_xs)
                        .text_color(theme.text_secondary)
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .overflow_hidden()
                        .child(message),
                ),
        )
        .child(
            div()
                .id("hint-dismiss")
                .cursor_pointer()
                .on_click(on_close)
                .child(
                    IconButton::with_child(
                        "hint-dismiss-btn",
                        Icon::new(IconName::X)
                            .size(IconSize::Sm)
                            .color(theme.text_secondary),
                    )
                    .variant(IconButtonVariant::Ghost)
                    .size(IconButtonSize::Sm)
                    .rounded_full()
                    .aria_label(dismiss_label)
                    .theme(theme.to_icon_button_theme()),
                ),
        )
}

pub(super) struct GuideSection {
    pub(super) heading: &'static str,
    pub(super) bullets: &'static [&'static str],
}

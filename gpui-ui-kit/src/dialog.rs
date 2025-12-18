//! Dialog/Modal component
//!
//! A modal dialog with backdrop, title, content, and footer sections.

use crate::theme::{Theme, ThemeExt};
use gpui::prelude::*;
use gpui::*;

/// Theme colors for dialog styling
#[derive(Debug, Clone)]
pub struct DialogTheme {
    /// Backdrop background
    pub backdrop: Rgba,
    /// Dialog background
    pub background: Rgba,
    /// Border color
    pub border: Rgba,
    /// Header border
    pub header_border: Rgba,
    /// Title text color
    pub title: Rgba,
    /// Close button text
    pub close: Rgba,
    /// Close button hover
    pub close_hover: Rgba,
    /// Close button hover background
    pub close_hover_bg: Rgba,
}

impl Default for DialogTheme {
    fn default() -> Self {
        Self {
            backdrop: rgba(0x000000aa),
            background: rgb(0x1e1e1e),
            border: rgb(0x007acc),
            header_border: rgb(0x3a3a3a),
            title: rgb(0xffffff),
            close: rgb(0x888888),
            close_hover: rgb(0xffffff),
            close_hover_bg: rgb(0x3a3a3a),
        }
    }
}

impl From<&Theme> for DialogTheme {
    fn from(theme: &Theme) -> Self {
        Self {
            backdrop: rgba(0x00000088),
            background: theme.surface,
            border: theme.accent,
            header_border: theme.border,
            title: theme.text_primary,
            close: theme.text_muted,
            close_hover: theme.text_primary,
            close_hover_bg: theme.surface_hover,
        }
    }
}

/// Dialog size variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogSize {
    /// Small dialog (320px)
    Sm,
    /// Medium dialog (480px)
    #[default]
    Md,
    /// Large dialog (640px)
    Lg,
    /// Extra large dialog (800px)
    Xl,
    /// Full width (90%)
    Full,
}

impl DialogSize {
    fn width(&self) -> Rems {
        match self {
            DialogSize::Sm => Rems(20.0),
            DialogSize::Md => Rems(30.0),
            DialogSize::Lg => Rems(40.0),
            DialogSize::Xl => Rems(50.0),
            DialogSize::Full => Rems(60.0),
        }
    }
}

/// A modal dialog component
pub struct Dialog {
    id: ElementId,
    title: Option<SharedString>,
    size: DialogSize,
    content: Option<AnyElement>,
    footer: Option<AnyElement>,
    show_close_button: bool,
    close_on_backdrop: bool,
    on_close: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl Dialog {
    /// Create a new dialog
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            title: None,
            size: DialogSize::default(),
            content: None,
            footer: None,
            show_close_button: true,
            close_on_backdrop: true,
            on_close: None,
        }
    }

    /// Set the dialog title
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the dialog size
    pub fn size(mut self, size: DialogSize) -> Self {
        self.size = size;
        self
    }

    /// Set the dialog content
    pub fn content(mut self, element: impl IntoElement) -> Self {
        self.content = Some(element.into_any_element());
        self
    }

    /// Alias for content (matches adabraka-ui API)
    pub fn child(self, element: impl IntoElement) -> Self {
        self.content(element)
    }

    /// Set the dialog footer
    pub fn footer(mut self, element: impl IntoElement) -> Self {
        self.footer = Some(element.into_any_element());
        self
    }

    /// Show or hide the close button
    pub fn show_close_button(mut self, show: bool) -> Self {
        self.show_close_button = show;
        self
    }

    /// Close dialog when clicking backdrop
    pub fn close_on_backdrop(mut self, close: bool) -> Self {
        self.close_on_backdrop = close;
        self
    }

    /// Set the close handler
    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Box::new(handler));
        self
    }

    /// Build the dialog into elements with theme
    pub fn build_with_theme(self, theme: &DialogTheme) -> Div {
        let width = self.size.width();
        let on_close = self.on_close;
        let close_on_backdrop = self.close_on_backdrop;

        // Backdrop
        let mut backdrop = div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.backdrop);

        // Handle backdrop click
        if close_on_backdrop
            && let Some(ref handler) = on_close {
                let handler: *const dyn Fn(&mut Window, &mut App) = handler.as_ref();
                backdrop = backdrop.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                    // Safety: handler is valid for the lifetime of the dialog
                    unsafe {
                        (*handler)(window, cx);
                    }
                });
            }

        // Dialog container
        let mut dialog = div()
            .id(self.id)
            .w(width)
            .max_h(Rems(45.0))
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .rounded_lg()
            .shadow_lg()
            .overflow_hidden()
            .flex()
            .flex_col()
            // Stop propagation so clicking dialog doesn't close it
            .on_mouse_down(MouseButton::Left, |_event, _window, _cx| {
                // Consume the event
            });

        // Header with title and close button
        if self.title.is_some() || self.show_close_button {
            let mut header = div()
                .flex()
                .items_center()
                .justify_between()
                .px_4()
                .py_3()
                .border_b_1()
                .border_color(theme.header_border);

            if let Some(title) = self.title {
                header = header.child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.title)
                        .child(title),
                );
            } else {
                header = header.child(div()); // Spacer
            }

            if self.show_close_button
                && let Some(ref handler) = on_close {
                    let handler: *const dyn Fn(&mut Window, &mut App) = handler.as_ref();
                    let close_color = theme.close;
                    let close_hover = theme.close_hover;
                    let close_hover_bg = theme.close_hover_bg;
                    header = header.child(
                        div()
                            .id("dialog-close-btn")
                            .px_2()
                            .py_1()
                            .rounded(px(3.0))
                            .cursor_pointer()
                            .text_color(close_color)
                            .hover(move |s| s.bg(close_hover_bg).text_color(close_hover))
                            .on_mouse_up(MouseButton::Left, move |_event, window, cx| unsafe {
                                (*handler)(window, cx);
                            })
                            .child("×"),
                    );
                }

            dialog = dialog.child(header);
        }

        // Content
        if let Some(content) = self.content {
            dialog = dialog.child(
                div()
                    .id("dialog-content")
                    .flex_1()
                    .overflow_y_scroll()
                    .px_4()
                    .py_4()
                    .child(content),
            );
        }

        // Footer
        if let Some(footer) = self.footer {
            dialog = dialog.child(
                div()
                    .px_4()
                    .py_3()
                    .border_t_1()
                    .border_color(theme.header_border)
                    .child(footer),
            );
        }

        backdrop.child(dialog)
    }
}

impl RenderOnce for Dialog {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let global_theme = cx.theme();
        let dialog_theme = DialogTheme::from(&global_theme);
        self.build_with_theme(&dialog_theme)
    }
}

impl IntoElement for Dialog {
    type Element = gpui::Component<Self>;

    fn into_element(self) -> Self::Element {
        gpui::Component::new(self)
    }
}

//! Alert component
//!
//! Contextual feedback messages.

use gpui::prelude::*;
use gpui::*;

/// Alert variant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlertVariant {
    /// Informational (default)
    #[default]
    Info,
    /// Success message
    Success,
    /// Warning message
    Warning,
    /// Error message
    Error,
}

impl AlertVariant {
    fn colors(&self) -> (Rgba, Rgba, Rgba) {
        // Returns (background, border, icon_color)
        match self {
            AlertVariant::Info => (rgb(0x1a2a3a), rgb(0x007acc), rgb(0x007acc)),
            AlertVariant::Success => (rgb(0x1a3a1a), rgb(0x2da44e), rgb(0x2da44e)),
            AlertVariant::Warning => (rgb(0x3a3a1a), rgb(0xd29922), rgb(0xd29922)),
            AlertVariant::Error => (rgb(0x3a1a1a), rgb(0xcc3333), rgb(0xcc3333)),
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            AlertVariant::Info => "ℹ",
            AlertVariant::Success => "✓",
            AlertVariant::Warning => "⚠",
            AlertVariant::Error => "✕",
        }
    }
}

/// An alert component
pub struct Alert {
    id: ElementId,
    title: Option<SharedString>,
    message: SharedString,
    variant: AlertVariant,
    closeable: bool,
    icon: Option<SharedString>,
    on_close: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl Alert {
    /// Create a new alert
    pub fn new(id: impl Into<ElementId>, message: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: None,
            message: message.into(),
            variant: AlertVariant::default(),
            closeable: false,
            icon: None,
            on_close: None,
        }
    }

    /// Set title
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set variant
    pub fn variant(mut self, variant: AlertVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Make closeable
    pub fn closeable(mut self, closeable: bool) -> Self {
        self.closeable = closeable;
        self
    }

    /// Set custom icon
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Set close handler
    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Box::new(handler));
        self
    }

    /// Build into element
    pub fn build(self) -> Stateful<Div> {
        let (bg, border, icon_color) = self.variant.colors();
        let default_icon = self.variant.icon();

        let mut alert = div()
            .id(self.id)
            .flex()
            .items_start()
            .gap_3()
            .p_4()
            .bg(bg)
            .border_1()
            .border_color(border)
            .rounded_lg();

        // Icon
        let icon = self.icon.unwrap_or_else(|| default_icon.into());
        alert = alert.child(div().text_lg().text_color(icon_color).child(icon));

        // Content
        let mut content = div().flex_1().flex().flex_col().gap_1();

        if let Some(title) = self.title {
            content = content.child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0xffffff))
                    .child(title),
            );
        }

        content = content.child(
            div()
                .text_sm()
                .text_color(rgb(0xcccccc))
                .child(self.message),
        );

        alert = alert.child(content);

        // Close button
        if self.closeable {
            let mut close_btn = div()
                .id("alert-close")
                .text_sm()
                .text_color(rgb(0x888888))
                .cursor_pointer()
                .hover(|s| s.text_color(rgb(0xffffff)));

            if let Some(handler) = self.on_close {
                let handler_ptr: *const dyn Fn(&mut Window, &mut App) = handler.as_ref();
                close_btn =
                    close_btn.on_mouse_up(MouseButton::Left, move |_event, window, cx| unsafe {
                        (*handler_ptr)(window, cx);
                    });
                std::mem::forget(handler);
            }

            alert = alert.child(close_btn.child("×"));
        }

        alert
    }
}

impl IntoElement for Alert {
    type Element = Stateful<Div>;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}

/// A simple inline alert (no close button)
pub struct InlineAlert {
    message: SharedString,
    variant: AlertVariant,
}

impl InlineAlert {
    /// Create a new inline alert
    pub fn new(message: impl Into<SharedString>) -> Self {
        Self {
            message: message.into(),
            variant: AlertVariant::default(),
        }
    }

    /// Set variant
    pub fn variant(mut self, variant: AlertVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Build into element
    pub fn build(self) -> Div {
        let (_, border, icon_color) = self.variant.colors();
        let icon = self.variant.icon();

        div()
            .flex()
            .items_center()
            .gap_2()
            .text_sm()
            .text_color(icon_color)
            .child(div().child(icon))
            .child(self.message)
    }
}

impl IntoElement for InlineAlert {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}

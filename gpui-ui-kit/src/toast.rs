//! Toast notification component
//!
//! Provides non-blocking notifications that appear temporarily.

use gpui::prelude::*;
use gpui::*;

/// Toast visual variant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastVariant {
    /// Informational message (default)
    #[default]
    Info,
    /// Success message
    Success,
    /// Warning message
    Warning,
    /// Error message
    Error,
}

impl ToastVariant {
    fn icon(&self) -> &'static str {
        match self {
            ToastVariant::Info => "ℹ",
            ToastVariant::Success => "✓",
            ToastVariant::Warning => "⚠",
            ToastVariant::Error => "✕",
        }
    }
}

/// Toast position on screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastPosition {
    /// Top right corner
    TopRight,
    /// Top left corner
    TopLeft,
    /// Bottom right corner (default)
    #[default]
    BottomRight,
    /// Bottom left corner
    BottomLeft,
    /// Top center
    TopCenter,
    /// Bottom center
    BottomCenter,
}

/// A single toast notification
pub struct Toast {
    id: ElementId,
    title: Option<SharedString>,
    message: SharedString,
    variant: ToastVariant,
    closeable: bool,
    on_close: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl Toast {
    /// Create a new toast with a message
    pub fn new(id: impl Into<ElementId>, message: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: None,
            message: message.into(),
            variant: ToastVariant::default(),
            closeable: true,
            on_close: None,
        }
    }

    /// Set the toast title
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the toast variant
    pub fn variant(mut self, variant: ToastVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set whether the toast is closeable
    pub fn closeable(mut self, closeable: bool) -> Self {
        self.closeable = closeable;
        self
    }

    /// Set the close handler
    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Box::new(handler));
        self
    }

    /// Build the toast into an element
    pub fn build(self) -> Stateful<Div> {
        // Get colors based on variant
        let (bg, border, icon_color) = match self.variant {
            ToastVariant::Info => (rgb(0x2a2a2a), rgb(0x007acc), rgb(0x007acc)),
            ToastVariant::Success => (rgb(0x1a3a1a), rgb(0x2da44e), rgb(0x2da44e)),
            ToastVariant::Warning => (rgb(0x3a3a1a), rgb(0xd29922), rgb(0xd29922)),
            ToastVariant::Error => (rgb(0x3a1a1a), rgb(0xcc3333), rgb(0xcc3333)),
        };
        let icon = self.variant.icon();

        let mut toast = div()
            .id(self.id)
            .w(px(320.0))
            .flex()
            .items_start()
            .gap_3()
            .px_4()
            .py_3()
            .bg(bg)
            .border_1()
            .border_color(border)
            .rounded_lg()
            .shadow_lg();

        // Icon
        toast = toast.child(
            div()
                .text_lg()
                .text_color(icon_color)
                .mt(px(2.0))
                .child(icon),
        );

        // Content area
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

        toast = toast.child(content);

        // Close button
        if self.closeable {
            if let Some(handler) = self.on_close {
                let handler_ptr: *const dyn Fn(&mut Window, &mut App) = handler.as_ref();
                toast = toast.child(
                    div()
                        .id("toast-close")
                        .text_sm()
                        .text_color(rgb(0x888888))
                        .cursor_pointer()
                        .hover(|s| s.text_color(rgb(0xffffff)))
                        .on_mouse_up(MouseButton::Left, move |_event, window, cx| unsafe {
                            (*handler_ptr)(window, cx);
                        })
                        .child("×"),
                );
                // Keep handler alive - it will be dropped with the toast
                std::mem::forget(handler);
            }
        }

        toast
    }
}

impl IntoElement for Toast {
    type Element = Stateful<Div>;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}

/// A container for positioning toasts on screen
pub struct ToastContainer {
    position: ToastPosition,
    toasts: Vec<Toast>,
}

impl ToastContainer {
    /// Create a new toast container
    pub fn new(position: ToastPosition) -> Self {
        Self {
            position,
            toasts: Vec::new(),
        }
    }

    /// Add a toast to the container
    pub fn toast(mut self, toast: Toast) -> Self {
        self.toasts.push(toast);
        self
    }

    /// Add multiple toasts
    pub fn toasts(mut self, toasts: impl IntoIterator<Item = Toast>) -> Self {
        self.toasts.extend(toasts);
        self
    }

    /// Build the container into an element
    pub fn build(self) -> Div {
        let mut container = div().absolute().flex().flex_col().gap_2().p_4();

        // Position the container
        match self.position {
            ToastPosition::TopRight => {
                container = container.top_0().right_0();
            }
            ToastPosition::TopLeft => {
                container = container.top_0().left_0();
            }
            ToastPosition::BottomRight => {
                container = container.bottom_0().right_0();
            }
            ToastPosition::BottomLeft => {
                container = container.bottom_0().left_0();
            }
            ToastPosition::TopCenter => {
                container = container.top_0().left_0().right_0().items_center();
            }
            ToastPosition::BottomCenter => {
                container = container.bottom_0().left_0().right_0().items_center();
            }
        }

        for toast in self.toasts {
            container = container.child(toast);
        }

        container
    }
}

impl IntoElement for ToastContainer {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}

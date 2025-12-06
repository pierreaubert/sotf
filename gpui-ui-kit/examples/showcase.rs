//! UI Kit Showcase
//!
//! A comprehensive demonstration of all gpui-ui-kit components.

use gpui::*;
use gpui_ui_kit::*;

struct Showcase {
    // Toggle states
    toggle_on: bool,
    checkbox_checked: bool,
    // Slider value
    slider_value: f32,
    // Tabs state
    selected_tab: usize,
}

impl Showcase {
    fn new() -> Self {
        Self {
            toggle_on: true,
            checkbox_checked: true,
            slider_value: 0.5,
            selected_tab: 0,
        }
    }
}

impl Render for Showcase {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let toggle_on = self.toggle_on;
        let checkbox_checked = self.checkbox_checked;
        let slider_value = self.slider_value;

        div()
            .w_full()
            .min_h_full()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xcccccc))
            .p_8()
            .flex()
            .flex_col()
            .gap_8()
            .child(
                // Header
                Heading::h1("GPUI UI Kit Showcase"),
            )
            .child(Text::new(
                "A comprehensive library of reusable UI components for GPUI applications.",
            ))
            .child(Divider::new().build())
            // Buttons Section
            .child(self.render_buttons_section())
            .child(Divider::new().build())
            // Text Section
            .child(self.render_text_section())
            .child(Divider::new().build())
            // Badges Section
            .child(self.render_badges_section())
            .child(Divider::new().build())
            // Avatars Section
            .child(self.render_avatars_section())
            .child(Divider::new().build())
            // Form Controls Section
            .child(self.render_form_controls_section(toggle_on, checkbox_checked, slider_value))
            .child(Divider::new().build())
            // Progress Section
            .child(self.render_progress_section())
            .child(Divider::new().build())
            // Alerts Section
            .child(self.render_alerts_section())
            .child(Divider::new().build())
            // Tabs Section
            .child(self.render_tabs_section())
            .child(Divider::new().build())
            // Card Section
            .child(self.render_card_section())
            .child(Divider::new().build())
            // Breadcrumbs Section
            .child(self.render_breadcrumbs_section())
            .child(Divider::new().build())
            // Spinners Section
            .child(self.render_spinners_section())
            .child(Divider::new().build())
            // Layout Section
            .child(self.render_layout_section())
    }
}

impl Showcase {
    fn section_header(&self, title: impl Into<SharedString>) -> impl IntoElement {
        Heading::h2(title)
    }

    fn render_buttons_section(&self) -> impl IntoElement {
        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(self.section_header("Buttons"))
            // Button Variants
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Variants").weight(TextWeight::Medium))
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(Button::new("btn-primary", "Primary").variant(ButtonVariant::Primary))
                            .child(Button::new("btn-secondary", "Secondary").variant(ButtonVariant::Secondary))
                            .child(Button::new("btn-destructive", "Destructive").variant(ButtonVariant::Destructive))
                            .child(Button::new("btn-ghost", "Ghost").variant(ButtonVariant::Ghost))
                            .child(Button::new("btn-outline", "Outline").variant(ButtonVariant::Outline)),
                    ),
            )
            // Button Sizes
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Sizes").weight(TextWeight::Medium))
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .align(StackAlign::End)
                            .child(Button::new("btn-xs", "Extra Small").size(ButtonSize::Xs))
                            .child(Button::new("btn-sm", "Small").size(ButtonSize::Sm))
                            .child(Button::new("btn-md", "Medium").size(ButtonSize::Md))
                            .child(Button::new("btn-lg", "Large").size(ButtonSize::Lg)),
                    ),
            )
            // Button States
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("States").weight(TextWeight::Medium))
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(Button::new("btn-disabled", "Disabled").disabled(true))
                            .child(Button::new("btn-selected", "Selected").selected(true))
                            .child(Button::new("btn-icon", "With Icon").icon_left("★")),
                    ),
            )
    }

    fn render_text_section(&self) -> impl IntoElement {
        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(self.section_header("Typography"))
            // Headings
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Heading::h1("Heading 1 (h1)"))
                    .child(Heading::h2("Heading 2 (h2)"))
                    .child(Heading::h3("Heading 3 (h3)"))
                    .child(Heading::h4("Heading 4 (h4)")),
            )
            // Text variants
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Regular text with default styling"))
                    .child(Text::new("Bold text").weight(TextWeight::Bold))
                    .child(Text::new("Medium weight text").weight(TextWeight::Medium))
                    .child(Text::new("Light weight text").weight(TextWeight::Light))
                    .child(Text::new("Small text").size(TextSize::Sm))
                    .child(Text::new("Extra small text").size(TextSize::Xs)),
            )
            // Code and Links
            .child(
                HStack::new()
                    .spacing(StackSpacing::Lg)
                    .child(Code::new("inline_code()"))
                    .child(Link::new("link-1", "Clickable Link")),
            )
    }

    fn render_badges_section(&self) -> impl IntoElement {
        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(self.section_header("Badges"))
            // Badge Variants
            .child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(Badge::new("Default").variant(BadgeVariant::Default))
                    .child(Badge::new("Primary").variant(BadgeVariant::Primary))
                    .child(Badge::new("Success").variant(BadgeVariant::Success))
                    .child(Badge::new("Warning").variant(BadgeVariant::Warning))
                    .child(Badge::new("Error").variant(BadgeVariant::Error))
                    .child(Badge::new("Info").variant(BadgeVariant::Info)),
            )
            // Badge Sizes and Styles
            .child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(Badge::new("Small").size(BadgeSize::Sm))
                    .child(Badge::new("Medium").size(BadgeSize::Md))
                    .child(Badge::new("Large").size(BadgeSize::Lg))
                    .child(Badge::new("Rounded").rounded(true))
                    .child(Badge::new("With Icon").icon("●")),
            )
            // Badge Dots
            .child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(BadgeDot::new().variant(BadgeVariant::Default))
                    .child(BadgeDot::new().variant(BadgeVariant::Primary))
                    .child(BadgeDot::new().variant(BadgeVariant::Success))
                    .child(BadgeDot::new().variant(BadgeVariant::Warning))
                    .child(BadgeDot::new().variant(BadgeVariant::Error)),
            )
    }

    fn render_avatars_section(&self) -> impl IntoElement {
        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(self.section_header("Avatars"))
            // Avatar Sizes
            .child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .align(StackAlign::End)
                    .child(Avatar::new().name("John Doe").size(AvatarSize::Xs))
                    .child(Avatar::new().name("Jane Smith").size(AvatarSize::Sm))
                    .child(Avatar::new().name("Bob Wilson").size(AvatarSize::Md))
                    .child(Avatar::new().name("Alice Brown").size(AvatarSize::Lg))
                    .child(Avatar::new().name("Charlie Davis").size(AvatarSize::Xl)),
            )
            // Avatar Shapes and Status
            .child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(Avatar::new().name("Circle").shape(AvatarShape::Circle))
                    .child(Avatar::new().name("Square").shape(AvatarShape::Square))
                    .child(Avatar::new().name("Online").status(AvatarStatus::Online))
                    .child(Avatar::new().name("Away").status(AvatarStatus::Away))
                    .child(Avatar::new().name("Busy").status(AvatarStatus::Busy))
                    .child(Avatar::new().name("Offline").status(AvatarStatus::Offline)),
            )
            // Avatar Group
            .child(
                AvatarGroup::new()
                    .avatars(vec![
                        Avatar::new().name("User One"),
                        Avatar::new().name("User Two"),
                        Avatar::new().name("User Three"),
                        Avatar::new().name("User Four"),
                        Avatar::new().name("User Five"),
                        Avatar::new().name("User Six"),
                    ])
                    .max_display(4)
                    .size(AvatarSize::Md),
            )
    }

    fn render_form_controls_section(
        &self,
        toggle_on: bool,
        checkbox_checked: bool,
        slider_value: f32,
    ) -> impl IntoElement {
        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(self.section_header("Form Controls"))
            // Toggles
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Toggles").weight(TextWeight::Medium))
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Lg)
                            .child(Toggle::new("toggle-sm").size(ToggleSize::Sm).checked(toggle_on))
                            .child(Toggle::new("toggle-md").size(ToggleSize::Md).checked(toggle_on))
                            .child(Toggle::new("toggle-lg").size(ToggleSize::Lg).checked(!toggle_on))
                            .child(Toggle::new("toggle-disabled").disabled(true).checked(true)),
                    ),
            )
            // Checkboxes
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Checkboxes").weight(TextWeight::Medium))
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Lg)
                            .child(Checkbox::new("cb-sm").label("Small").size(CheckboxSize::Sm).checked(checkbox_checked))
                            .child(Checkbox::new("cb-md").label("Medium").size(CheckboxSize::Md).checked(checkbox_checked))
                            .child(Checkbox::new("cb-lg").label("Large").size(CheckboxSize::Lg).checked(!checkbox_checked))
                            .child(Checkbox::new("cb-disabled").label("Disabled").disabled(true).checked(true)),
                    ),
            )
            // Slider
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new(format!("Slider: {:.0}%", slider_value * 100.0)).weight(TextWeight::Medium))
                    .child(
                        div()
                            .w(px(300.0))
                            .child(Slider::new("slider-demo").value(slider_value).size(SliderSize::Medium)),
                    ),
            )
            // Input
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Input").weight(TextWeight::Medium))
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Input::new("input-default")
                                    .placeholder("Default input...")
                                    .variant(InputVariant::Default),
                            )
                            .child(
                                Input::new("input-filled")
                                    .placeholder("Filled input...")
                                    .variant(InputVariant::Filled),
                            )
                            .child(
                                Input::new("input-disabled")
                                    .placeholder("Disabled...")
                                    .disabled(true),
                            ),
                    ),
            )
    }

    fn render_progress_section(&self) -> impl IntoElement {
        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(self.section_header("Progress Indicators"))
            // Linear Progress
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Linear Progress").weight(TextWeight::Medium))
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                div()
                                    .w(px(300.0))
                                    .child(Progress::new(0.25).size(ProgressSize::Sm)),
                            )
                            .child(
                                div()
                                    .w(px(300.0))
                                    .child(Progress::new(0.50).size(ProgressSize::Md)),
                            )
                            .child(
                                div()
                                    .w(px(300.0))
                                    .child(Progress::new(0.75).size(ProgressSize::Lg)),
                            )
                            .child(
                                div()
                                    .w(px(300.0))
                                    .child(Progress::new(0.90).variant(ProgressVariant::Success)),
                            ),
                    ),
            )
            // Circular Progress
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Circular Progress").weight(TextWeight::Medium))
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Lg)
                            .child(CircularProgress::new(0.25).size(px(32.0)))
                            .child(CircularProgress::new(0.50).size(px(48.0)))
                            .child(CircularProgress::new(0.75).size(px(64.0)))
                            .child(CircularProgress::new(0.90).size(px(48.0)).variant(ProgressVariant::Success)),
                    ),
            )
    }

    fn render_alerts_section(&self) -> impl IntoElement {
        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(self.section_header("Alerts"))
            .child(
                VStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        Alert::new("alert-info", "This is an informational message.")
                            .title("Information")
                            .variant(AlertVariant::Info),
                    )
                    .child(
                        Alert::new("alert-success", "Your changes have been saved successfully.")
                            .title("Success")
                            .variant(AlertVariant::Success),
                    )
                    .child(
                        Alert::new("alert-warning", "Please review your settings before continuing.")
                            .title("Warning")
                            .variant(AlertVariant::Warning),
                    )
                    .child(
                        Alert::new("alert-error", "An error occurred while processing your request.")
                            .title("Error")
                            .variant(AlertVariant::Error),
                    ),
            )
            // Inline Alerts
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Inline Alerts").weight(TextWeight::Medium))
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Lg)
                            .child(InlineAlert::new("Info message").variant(AlertVariant::Info))
                            .child(InlineAlert::new("Success message").variant(AlertVariant::Success))
                            .child(InlineAlert::new("Warning message").variant(AlertVariant::Warning))
                            .child(InlineAlert::new("Error message").variant(AlertVariant::Error)),
                    ),
            )
    }

    fn render_tabs_section(&self) -> impl IntoElement {
        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(self.section_header("Tabs"))
            // Underline Tabs
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Underline Variant").weight(TextWeight::Medium))
                    .child(
                        Tabs::new()
                            .variant(TabVariant::Underline)
                            .selected_index(self.selected_tab)
                            .tabs(vec![
                                TabItem::new("tab-1", "Overview").icon("📊"),
                                TabItem::new("tab-2", "Analytics").icon("📈"),
                                TabItem::new("tab-3", "Reports").icon("📄"),
                                TabItem::new("tab-4", "Settings").icon("⚙"),
                            ]),
                    ),
            )
            // Pills Tabs
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Pills Variant").weight(TextWeight::Medium))
                    .child(
                        Tabs::new()
                            .variant(TabVariant::Pills)
                            .selected_index(1)
                            .tabs(vec![
                                TabItem::new("pill-1", "All"),
                                TabItem::new("pill-2", "Active"),
                                TabItem::new("pill-3", "Completed"),
                            ]),
                    ),
            )
            // Enclosed Tabs
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Enclosed Variant").weight(TextWeight::Medium))
                    .child(
                        Tabs::new()
                            .variant(TabVariant::Enclosed)
                            .selected_index(0)
                            .tabs(vec![
                                TabItem::new("enc-1", "Files"),
                                TabItem::new("enc-2", "Folders"),
                                TabItem::new("enc-3", "Trash").badge("3"),
                            ]),
                    ),
            )
    }

    fn render_card_section(&self) -> impl IntoElement {
        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(self.section_header("Cards"))
            .child(
                HStack::new()
                    .spacing(StackSpacing::Lg)
                    .wrap(true)
                    .child(
                        Card::new()
                            .header(
                                div()
                                    .child(Heading::h3("Card Title"))
                                    .child(Text::new("Card subtitle").size(TextSize::Sm)),
                            )
                            .content(
                                Text::new("This is the card content. Cards can contain any content including text, images, and other components."),
                            )
                            .footer(
                                HStack::new()
                                    .justify(StackJustify::End)
                                    .spacing(StackSpacing::Sm)
                                    .child(Button::new("card-cancel", "Cancel").variant(ButtonVariant::Ghost))
                                    .child(Button::new("card-save", "Save").variant(ButtonVariant::Primary)),
                            )
                            .build()
                            .w(px(300.0)),
                    )
                    .child(
                        Card::new()
                            .header(Heading::h3("Simple Card"))
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(Text::new("• Feature one"))
                                    .child(Text::new("• Feature two"))
                                    .child(Text::new("• Feature three")),
                            )
                            .build()
                            .w(px(250.0)),
                    ),
            )
    }

    fn render_breadcrumbs_section(&self) -> impl IntoElement {
        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(self.section_header("Breadcrumbs"))
            // Different separators
            .child(
                VStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        Breadcrumbs::new()
                            .separator(BreadcrumbSeparator::Slash)
                            .items(vec![
                                BreadcrumbItem::new("home", "Home").icon("🏠"),
                                BreadcrumbItem::new("products", "Products"),
                                BreadcrumbItem::new("category", "Electronics"),
                                BreadcrumbItem::new("item", "Laptop"),
                            ]),
                    )
                    .child(
                        Breadcrumbs::new()
                            .separator(BreadcrumbSeparator::Chevron)
                            .items(vec![
                                BreadcrumbItem::new("root", "Root"),
                                BreadcrumbItem::new("folder", "Folder"),
                                BreadcrumbItem::new("file", "File.txt"),
                            ]),
                    )
                    .child(
                        Breadcrumbs::new()
                            .separator(BreadcrumbSeparator::Dot)
                            .items(vec![
                                BreadcrumbItem::new("app", "App"),
                                BreadcrumbItem::new("settings", "Settings"),
                                BreadcrumbItem::new("account", "Account"),
                            ]),
                    ),
            )
    }

    fn render_spinners_section(&self) -> impl IntoElement {
        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(self.section_header("Loading Indicators"))
            // Spinners
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Spinners").weight(TextWeight::Medium))
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Lg)
                            .align(StackAlign::End)
                            .child(Spinner::new().size(SpinnerSize::Xs))
                            .child(Spinner::new().size(SpinnerSize::Sm))
                            .child(Spinner::new().size(SpinnerSize::Md))
                            .child(Spinner::new().size(SpinnerSize::Lg))
                            .child(Spinner::new().size(SpinnerSize::Xl))
                            .child(Spinner::new().size(SpinnerSize::Md).label("Loading...")),
                    ),
            )
            // Loading Dots
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Loading Dots").weight(TextWeight::Medium))
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Lg)
                            .align(StackAlign::End)
                            .child(LoadingDots::new().size(SpinnerSize::Sm))
                            .child(LoadingDots::new().size(SpinnerSize::Md))
                            .child(LoadingDots::new().size(SpinnerSize::Lg))
                            .child(LoadingDots::new().size(SpinnerSize::Md).color(rgb(0x2da44e))),
                    ),
            )
    }

    fn render_layout_section(&self) -> impl IntoElement {
        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(self.section_header("Layout Components"))
            // HStack and VStack
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("HStack & VStack").weight(TextWeight::Medium))
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Lg)
                            .child(
                                VStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(
                                        div()
                                            .p_2()
                                            .bg(rgb(0x3a3a3a))
                                            .rounded_md()
                                            .child("VStack Item 1"),
                                    )
                                    .child(
                                        div()
                                            .p_2()
                                            .bg(rgb(0x3a3a3a))
                                            .rounded_md()
                                            .child("VStack Item 2"),
                                    )
                                    .child(
                                        div()
                                            .p_2()
                                            .bg(rgb(0x3a3a3a))
                                            .rounded_md()
                                            .child("VStack Item 3"),
                                    ),
                            )
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(
                                        div()
                                            .p_2()
                                            .bg(rgb(0x3a3a3a))
                                            .rounded_md()
                                            .child("H1"),
                                    )
                                    .child(
                                        div()
                                            .p_2()
                                            .bg(rgb(0x3a3a3a))
                                            .rounded_md()
                                            .child("H2"),
                                    )
                                    .child(
                                        div()
                                            .p_2()
                                            .bg(rgb(0x3a3a3a))
                                            .rounded_md()
                                            .child("H3"),
                                    ),
                            ),
                    ),
            )
            // Spacer demonstration
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Spacer").weight(TextWeight::Medium))
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                div()
                                    .w(px(400.0))
                                    .p_3()
                                    .bg(rgb(0x2a2a2a))
                                    .rounded_md()
                                    .flex()
                                    .items_center()
                                    .child(Text::new("Left"))
                                    .child(Spacer::new())
                                    .child(Text::new("Right")),
                            ),
                    ),
            )
            // Dividers
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Dividers").weight(TextWeight::Medium))
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                div()
                                    .w(px(300.0))
                                    .child(Divider::new().color(rgb(0x555555)).build()),
                            )
                            .child(
                                div()
                                    .w(px(300.0))
                                    .child(Divider::new().thickness(px(2.0)).color(rgb(0x007acc)).build()),
                            )
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Md)
                                    .child(Text::new("Left"))
                                    .child(
                                        div()
                                            .h(px(20.0))
                                            .child(Divider::vertical().color(rgb(0x555555)).build()),
                                    )
                                    .child(Text::new("Right")),
                            ),
                    ),
            )
    }
}

fn main() {
    MiniApp::run(
        MiniAppConfig::new("UI Kit Showcase")
            .size(1200.0, 900.0)
            .scrollable(true),
        |cx| cx.new(|_| Showcase::new()),
    );
}

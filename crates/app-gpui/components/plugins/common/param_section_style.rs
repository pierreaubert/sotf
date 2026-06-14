use crate::components::design::Ds;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

/// Trait extension for applying parameter section styling to any Div
pub trait ParamSectionStyle {
    /// Apply base param section styling (rounded, background, border) without padding
    fn param_section_base(self, d: &Ds, theme: &Theme) -> Self;
    /// Apply param section styling with standard padding
    fn param_section_style(self, d: &Ds, theme: &Theme) -> Self;
    /// Apply param section styling with larger padding
    fn param_section_style_lg(self, d: &Ds, theme: &Theme) -> Self;
}

impl ParamSectionStyle for Div {
    fn param_section_base(self, d: &Ds, theme: &Theme) -> Self {
        self.rounded(d.r_xl)
            .bg(theme.background_secondary)
            .border_1()
            .border_color(theme.border)
    }

    fn param_section_style(self, d: &Ds, theme: &Theme) -> Self {
        self.param_section_base(d, theme).p(d.pad_x)
    }

    fn param_section_style_lg(self, d: &Ds, theme: &Theme) -> Self {
        self.param_section_base(d, theme).p(d.card)
    }
}

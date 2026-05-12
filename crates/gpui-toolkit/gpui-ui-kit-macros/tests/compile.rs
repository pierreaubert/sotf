use gpui_ui_kit_macros::ComponentTheme;

mod gpui {
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Rgba(pub u32);

    pub fn rgb(val: u32) -> Rgba {
        Rgba(val)
    }

    pub fn rgba(val: u32) -> Rgba {
        Rgba(val)
    }
}

mod theme {
    use super::gpui::Rgba;

    #[derive(Debug, Clone)]
    pub struct Theme {
        pub accent: Rgba,
        pub surface: Rgba,
        pub transparent: Rgba,
    }
}

#[derive(Debug, Clone, ComponentTheme)]
pub struct BasicTheme {
    #[theme(default = 0x007acc, from = accent)]
    pub primary: gpui::Rgba,
    #[theme(default = 0x3c3c3c, from = surface)]
    pub surface: gpui::Rgba,
}

#[derive(Debug, Clone, ComponentTheme)]
pub struct TransparentTheme {
    #[theme(default = 0x00000000, from = transparent)]
    pub transparent: gpui::Rgba,
}

#[derive(Debug, Clone, ComponentTheme)]
#[theme_path = "theme::Theme"]
#[gpui_path = "gpui"]
pub struct ExplicitPathTheme {
    #[theme(default = 0xff0000, from = accent)]
    pub red: gpui::Rgba,
}

#[test]
fn test_rgb_uses_rgb() {
    let theme = BasicTheme::default();
    assert_eq!(theme.primary, gpui::rgb(0x007acc));
    assert_eq!(theme.surface, gpui::rgb(0x3c3c3c));
}

#[test]
fn test_transparent_uses_rgba() {
    let theme = TransparentTheme::default();
    assert_eq!(theme.transparent, gpui::rgba(0x00000000));
}

#[test]
fn test_from_theme() {
    let global = theme::Theme {
        accent: gpui::rgba(0x12345678),
        surface: gpui::rgb(0xabcdef),
        transparent: gpui::rgba(0x00000000),
    };
    let t = BasicTheme::from(&global);
    assert_eq!(t.primary, global.accent);
    assert_eq!(t.surface, global.surface);
}

#[test]
fn test_explicit_path_theme() {
    let global = theme::Theme {
        accent: gpui::rgb(0xff0000),
        surface: gpui::rgb(0x00ff00),
        transparent: gpui::rgba(0x00000000),
    };
    let t = ExplicitPathTheme::from(&global);
    assert_eq!(t.red, global.accent);
}

//! RGB color representation

use gpui::Rgba;

/// RGB color with alpha channel and interpolation support
///
/// # Example
///
/// ```
/// use d3rs::color::D3Color;
///
/// let red = D3Color::rgb(255, 0, 0);
/// let blue = D3Color::from_hex(0x0000ff);
/// let purple = red.interpolate(&blue, 0.5);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct D3Color {
    /// Red component (0.0 - 1.0)
    pub r: f32,
    /// Green component (0.0 - 1.0)
    pub g: f32,
    /// Blue component (0.0 - 1.0)
    pub b: f32,
    /// Alpha component (0.0 - 1.0)
    pub a: f32,
}

impl D3Color {
    /// Create a color from RGB values (0-255)
    ///
    /// # Example
    ///
    /// ```
    /// use d3rs::color::D3Color;
    ///
    /// let red = D3Color::rgb(255, 0, 0);
    /// ```
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }

    /// Create a color from RGBA values (0-255)
    ///
    /// # Example
    ///
    /// ```
    /// use d3rs::color::D3Color;
    ///
    /// let semi_transparent_red = D3Color::rgba(255, 0, 0, 128);
    /// ```
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }

    /// Create a color from a hex value (0xRRGGBB)
    ///
    /// # Example
    ///
    /// ```
    /// use d3rs::color::D3Color;
    ///
    /// let orange = D3Color::from_hex(0xff5500);
    /// ```
    pub fn from_hex(hex: u32) -> Self {
        Self::rgb(
            ((hex >> 16) & 0xFF) as u8,
            ((hex >> 8) & 0xFF) as u8,
            (hex & 0xFF) as u8,
        )
    }

    /// Create a color from RGB floats (0.0 - 1.0)
    ///
    /// # Example
    ///
    /// ```
    /// use d3rs::color::D3Color;
    ///
    /// let red = D3Color::from_rgb_f32(1.0, 0.0, 0.0);
    /// ```
    pub fn from_rgb_f32(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Create a color from RGBA floats (0.0 - 1.0)
    pub fn from_rgba_f32(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Convert to GPUI Rgba type
    ///
    /// # Example
    ///
    /// ```
    /// use d3rs::color::D3Color;
    ///
    /// let color = D3Color::rgb(255, 128, 0);
    /// let gpui_color = color.to_rgba();
    /// ```
    pub fn to_rgba(&self) -> Rgba {
        Rgba {
            r: self.r,
            g: self.g,
            b: self.b,
            a: self.a,
        }
    }

    /// Create from GPUI Rgba type
    pub fn from_rgba(rgba: Rgba) -> Self {
        Self {
            r: rgba.r,
            g: rgba.g,
            b: rgba.b,
            a: rgba.a,
        }
    }

    /// Set the alpha channel
    ///
    /// # Example
    ///
    /// ```
    /// use d3rs::color::D3Color;
    ///
    /// let semi_transparent = D3Color::rgb(255, 0, 0).with_alpha(0.5);
    /// ```
    pub fn with_alpha(mut self, alpha: f32) -> Self {
        self.a = alpha.clamp(0.0, 1.0);
        self
    }

    /// Linear interpolation between two colors
    ///
    /// `t` should be in the range [0.0, 1.0], where:
    /// - 0.0 returns `self`
    /// - 1.0 returns `other`
    /// - 0.5 returns the midpoint
    ///
    /// # Example
    ///
    /// ```
    /// use d3rs::color::D3Color;
    ///
    /// let red = D3Color::rgb(255, 0, 0);
    /// let blue = D3Color::rgb(0, 0, 255);
    /// let purple = red.interpolate(&blue, 0.5);
    /// ```
    pub fn interpolate(&self, other: &D3Color, t: f32) -> D3Color {
        let t = t.clamp(0.0, 1.0);
        D3Color {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    /// Lighten the color by a factor (0.0 - 1.0)
    pub fn lighten(&self, amount: f32) -> D3Color {
        let white = D3Color::rgb(255, 255, 255);
        self.interpolate(&white, amount.clamp(0.0, 1.0))
    }

    /// Darken the color by a factor (0.0 - 1.0)
    pub fn darken(&self, amount: f32) -> D3Color {
        let black = D3Color::rgb(0, 0, 0);
        self.interpolate(&black, amount.clamp(0.0, 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_rgb_creation() {
        let color = D3Color::rgb(255, 128, 64);
        assert_relative_eq!(color.r, 1.0);
        assert_relative_eq!(color.g, 128.0 / 255.0, epsilon = 1e-6);
        assert_relative_eq!(color.b, 64.0 / 255.0, epsilon = 1e-6);
        assert_relative_eq!(color.a, 1.0);
    }

    #[test]
    fn test_hex_conversion() {
        let color = D3Color::from_hex(0xff8040);
        assert_relative_eq!(color.r, 1.0, epsilon = 1e-6);
        assert_relative_eq!(color.g, 128.0 / 255.0, epsilon = 1e-6);
        assert_relative_eq!(color.b, 64.0 / 255.0, epsilon = 1e-6);
    }

    #[test]
    fn test_interpolation() {
        let red = D3Color::rgb(255, 0, 0);
        let blue = D3Color::rgb(0, 0, 255);

        let mid = red.interpolate(&blue, 0.5);
        assert_relative_eq!(mid.r, 0.5, epsilon = 1e-6);
        assert_relative_eq!(mid.g, 0.0, epsilon = 1e-6);
        assert_relative_eq!(mid.b, 0.5, epsilon = 1e-6);

        let at_start = red.interpolate(&blue, 0.0);
        assert_relative_eq!(at_start.r, red.r, epsilon = 1e-6);

        let at_end = red.interpolate(&blue, 1.0);
        assert_relative_eq!(at_end.b, blue.b, epsilon = 1e-6);
    }

    #[test]
    fn test_alpha_channel() {
        let color = D3Color::rgba(255, 0, 0, 128);
        assert_relative_eq!(color.a, 128.0 / 255.0, epsilon = 1e-6);

        let with_alpha = D3Color::rgb(255, 0, 0).with_alpha(0.5);
        assert_relative_eq!(with_alpha.a, 0.5);
    }

    #[test]
    fn test_lighten_darken() {
        let color = D3Color::rgb(128, 128, 128);

        let lighter = color.lighten(0.5);
        assert!(lighter.r > color.r);
        assert!(lighter.g > color.g);
        assert!(lighter.b > color.b);

        let darker = color.darken(0.5);
        assert!(darker.r < color.r);
        assert!(darker.g < color.g);
        assert!(darker.b < color.b);
    }

    #[test]
    fn test_rgba_conversion() {
        let color = D3Color::rgb(255, 128, 64);
        let rgba = color.to_rgba();

        assert_relative_eq!(rgba.r, 1.0);
        assert_relative_eq!(rgba.g, 128.0 / 255.0, epsilon = 1e-6);
        assert_relative_eq!(rgba.b, 64.0 / 255.0, epsilon = 1e-6);

        let back = D3Color::from_rgba(rgba);
        assert_relative_eq!(back.r, color.r);
        assert_relative_eq!(back.g, color.g);
        assert_relative_eq!(back.b, color.b);
    }
}

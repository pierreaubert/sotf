use gpui::Rgba;

/// Helper function to create Rgba from 24-bit hex value (alpha = 1.0)
#[expect(
    clippy::cast_precision_loss,
    reason = "Presentation colors and image geometry deliberately use floating-point coordinates"
)]
pub(crate) fn rgb(hex: u32) -> Rgba {
    Rgba {
        r: ((hex >> 16) & 0xFF) as f32 / 255.0,
        g: ((hex >> 8) & 0xFF) as f32 / 255.0,
        b: (hex & 0xFF) as f32 / 255.0,
        a: 1.0,
    }
}

/// Helper function to create Rgba from 32-bit hex value (RRGGBBAA)
#[expect(
    clippy::cast_precision_loss,
    reason = "Presentation colors and image geometry deliberately use floating-point coordinates"
)]
pub(crate) fn rgba(hex: u32) -> Rgba {
    Rgba {
        r: ((hex >> 24) & 0xFF) as f32 / 255.0,
        g: ((hex >> 16) & 0xFF) as f32 / 255.0,
        b: ((hex >> 8) & 0xFF) as f32 / 255.0,
        a: (hex & 0xFF) as f32 / 255.0,
    }
}

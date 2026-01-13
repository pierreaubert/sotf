//! Text rendering using Core Text
//!
//! Provides simple text rendering for UI labels using macOS Core Text.

use cocoa::base::{id, nil};
use cocoa::foundation::{NSPoint, NSSize, NSString};
use core_graphics::base::CGFloat;
use core_graphics::color_space::CGColorSpace;
use core_graphics::context::CGContext;
use core_graphics::image::CGImageAlphaInfo;
use metal::foreign_types::ForeignType;
use metal::{Device, MTLPixelFormat, MTLTextureUsage, Texture, TextureDescriptor};
use objc::{class, msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::ffi::c_void;

/// Text alignment
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// A cached text label rendered to a texture
pub struct TextLabel {
    pub texture: Texture,
    pub width: f32,
    pub height: f32,
}

/// Text renderer using Core Text and Core Graphics
pub struct TextRenderer {
    device: Device,
    font_name: String,
    font_size: f32,
    cache: HashMap<String, TextLabel>,
}

impl TextRenderer {
    /// Create a new text renderer
    pub fn new(device: Device, font_name: &str, font_size: f32) -> Self {
        Self {
            device,
            font_name: font_name.to_string(),
            font_size,
            cache: HashMap::new(),
        }
    }

    /// Clear the text cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Render text to a texture
    pub fn render_text(&mut self, text: &str) -> Option<&TextLabel> {
        // Check cache first
        if self.cache.contains_key(text) {
            return self.cache.get(text);
        }

        // Create attributed string and measure
        let (width, height) = self.measure_text(text)?;

        // Add padding
        let tex_width = (width.ceil() as u32).max(1);
        let tex_height = (height.ceil() as u32).max(1);

        // Create texture
        let texture = self.create_texture(tex_width, tex_height)?;

        // Render text to texture via Core Graphics
        self.render_to_texture(&texture, text, tex_width, tex_height)?;

        // Cache and return
        let label = TextLabel {
            texture,
            width,
            height,
        };
        self.cache.insert(text.to_string(), label);
        self.cache.get(text)
    }

    /// Measure text dimensions
    fn measure_text(&self, text: &str) -> Option<(f32, f32)> {
        unsafe {
            // Create NSFont
            let font_name = NSString::alloc(nil).init_str(&self.font_name);
            let font: id =
                msg_send![class!(NSFont), fontWithName:font_name size:self.font_size as CGFloat];
            let _: () = msg_send![font_name, release];

            if font.is_null() {
                // Fallback to system font
                let font: id =
                    msg_send![class!(NSFont), systemFontOfSize:self.font_size as CGFloat];
                if font.is_null() {
                    return None;
                }
            }

            // Create NSString for measurement
            let ns_text = NSString::alloc(nil).init_str(text);

            // Create attributes dictionary
            let font_key: id =
                msg_send![class!(NSString), stringWithUTF8String: b"NSFont\0".as_ptr()];
            let attrs: id =
                msg_send![class!(NSDictionary), dictionaryWithObject:font forKey:font_key];

            // Measure
            let size: NSSize = msg_send![ns_text, sizeWithAttributes:attrs];
            let _: () = msg_send![ns_text, release];

            Some((size.width as f32, size.height as f32))
        }
    }

    /// Create a texture for text rendering
    fn create_texture(&self, width: u32, height: u32) -> Option<Texture> {
        let desc = TextureDescriptor::new();
        desc.set_width(width as u64);
        desc.set_height(height as u64);
        desc.set_pixel_format(MTLPixelFormat::RGBA8Unorm);
        desc.set_usage(MTLTextureUsage::ShaderRead);
        desc.set_storage_mode(metal::MTLStorageMode::Managed);

        Some(self.device.new_texture(&desc))
    }

    /// Render text to a texture using Core Graphics
    fn render_to_texture(
        &self,
        texture: &Texture,
        text: &str,
        width: u32,
        height: u32,
    ) -> Option<()> {
        unsafe {
            // Create bitmap context
            let color_space = CGColorSpace::create_device_rgb();
            let bytes_per_row = width * 4;
            let mut buffer = vec![0u8; (bytes_per_row * height) as usize];

            let context: CGContext = CGContext::create_bitmap_context(
                Some(buffer.as_mut_ptr() as *mut c_void),
                width as usize,
                height as usize,
                8,
                bytes_per_row as usize,
                &color_space,
                CGImageAlphaInfo::CGImageAlphaPremultipliedLast as u32,
            );

            // Clear with transparent
            context.clear_rect(core_graphics::geometry::CGRect::new(
                &core_graphics::geometry::CGPoint::new(0.0, 0.0),
                &core_graphics::geometry::CGSize::new(width as f64, height as f64),
            ));

            // Set up text rendering
            context.set_allows_antialiasing(true);
            context.set_should_antialias(true);
            context.set_allows_font_smoothing(true);
            context.set_should_smooth_fonts(true);

            // Use NSGraphicsContext for text rendering (easier than CTLine)
            let ns_context: id = msg_send![class!(NSGraphicsContext), graphicsContextWithCGContext:context.as_ptr() flipped:false];
            let _: () = msg_send![class!(NSGraphicsContext), setCurrentContext:ns_context];

            // Create font
            let font_name_ns = NSString::alloc(nil).init_str(&self.font_name);
            let mut font: id =
                msg_send![class!(NSFont), fontWithName:font_name_ns size:self.font_size as CGFloat];
            let _: () = msg_send![font_name_ns, release];

            if font.is_null() {
                font = msg_send![class!(NSFont), systemFontOfSize:self.font_size as CGFloat];
            }

            // Create color (white)
            let color: id = msg_send![class!(NSColor), whiteColor];

            // Create attributes
            let font_key: id =
                msg_send![class!(NSString), stringWithUTF8String: b"NSFont\0".as_ptr()];
            let color_key: id =
                msg_send![class!(NSString), stringWithUTF8String: b"NSColor\0".as_ptr()];

            let keys: [id; 2] = [font_key, color_key];
            let values: [id; 2] = [font, color];

            let attrs: id = msg_send![
                class!(NSDictionary),
                dictionaryWithObjects:values.as_ptr()
                forKeys:keys.as_ptr()
                count:2_u64
            ];

            // Draw text
            let ns_text = NSString::alloc(nil).init_str(text);
            let point = NSPoint::new(0.0, 2.0); // Small baseline offset
            let _: () = msg_send![ns_text, drawAtPoint:point withAttributes:attrs];
            let _: () = msg_send![ns_text, release];

            // Restore graphics context
            let _: () = msg_send![class!(NSGraphicsContext), setCurrentContext:nil];

            // Upload to texture
            let region = metal::MTLRegion {
                origin: metal::MTLOrigin { x: 0, y: 0, z: 0 },
                size: metal::MTLSize {
                    width: width as u64,
                    height: height as u64,
                    depth: 1,
                },
            };

            texture.replace_region(
                region,
                0,
                buffer.as_ptr() as *const c_void,
                bytes_per_row as u64,
            );

            Some(())
        }
    }

    /// Get a cached label or render it
    pub fn get_or_render(&mut self, text: &str) -> Option<&TextLabel> {
        if !self.cache.contains_key(text) {
            self.render_text(text)?;
        }
        self.cache.get(text)
    }
}

/// Predefined labels for EQ view
pub struct EQLabels {
    pub freq_labels: Vec<(String, f32)>, // (label, frequency)
    pub db_labels: Vec<(String, f32)>,   // (label, db value)
}

impl Default for EQLabels {
    fn default() -> Self {
        Self {
            freq_labels: vec![
                ("20".to_string(), 20.0),
                ("50".to_string(), 50.0),
                ("100".to_string(), 100.0),
                ("200".to_string(), 200.0),
                ("500".to_string(), 500.0),
                ("1k".to_string(), 1000.0),
                ("2k".to_string(), 2000.0),
                ("5k".to_string(), 5000.0),
                ("10k".to_string(), 10000.0),
                ("20k".to_string(), 20000.0),
            ],
            db_labels: vec![
                ("+24".to_string(), 24.0),
                ("+12".to_string(), 12.0),
                ("0".to_string(), 0.0),
                ("-12".to_string(), -12.0),
                ("-24".to_string(), -24.0),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eq_labels() {
        let labels = EQLabels::default();
        assert_eq!(labels.freq_labels.len(), 10);
        assert_eq!(labels.db_labels.len(), 5);
    }
}

use image::{DynamicImage, RgbaImage};

/// Center-crop decoded artwork to the square shown by `ObjectFit::Cover`, then
/// apply the alpha mask to the pixels that will actually remain visible.
pub(crate) fn prepare_album_art_image(image: DynamicImage, corner_radius_ratio: f32) -> RgbaImage {
    let side = image.width().min(image.height());
    if side == 0 {
        return RgbaImage::new(0, 0);
    }

    let crop_x = (image.width() - side) / 2;
    let crop_y = (image.height() - side) / 2;
    let mut image = image.crop_imm(crop_x, crop_y, side, side).to_rgba8();
    apply_album_art_corner_mask(&mut image, corner_radius_ratio);
    image
}

/// Apply a rounded-corner alpha mask with a one-pixel coverage ramp.
///
/// GPUI's image overflow clip does not consistently mask decoded image
/// content, so album thumbnails carry their own alpha mask. Scaling the
/// source alpha by boundary coverage avoids the jagged binary edge produced
/// by an all-or-nothing mask.
pub(crate) fn apply_album_art_corner_mask(image: &mut RgbaImage, corner_radius_ratio: f32) {
    let min_dimension = image.width().min(image.height());
    if min_dimension == 0 || corner_radius_ratio <= 0.0 {
        return;
    }

    let radius = ((min_dimension as f32) * corner_radius_ratio)
        .round()
        .max(1.0)
        .min(min_dimension as f32 / 2.0);
    let center = radius - 0.5;
    let extent = radius.ceil() as u32;
    let width = image.width();
    let height = image.height();

    for y in 0..extent {
        for x in 0..extent {
            let distance = ((x as f32 - center).powi(2) + (y as f32 - center).powi(2)).sqrt();
            let coverage = (radius + 0.5 - distance).clamp(0.0, 1.0);
            if coverage >= 1.0 {
                continue;
            }

            let pixels = [
                (x, y),
                (width - 1 - x, y),
                (x, height - 1 - y),
                (width - 1 - x, height - 1 - y),
            ];
            for (index, &(pixel_x, pixel_y)) in pixels.iter().enumerate() {
                if pixels[..index].contains(&(pixel_x, pixel_y)) {
                    continue;
                }
                let alpha = &mut image.get_pixel_mut(pixel_x, pixel_y).0[3];
                *alpha = ((*alpha as f32) * coverage).round() as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn rounded_mask_antialiases_boundary_and_preserves_source_alpha() {
        let mut opaque = RgbaImage::from_pixel(16, 16, Rgba([20, 40, 60, 255]));
        let mut translucent = RgbaImage::from_pixel(16, 16, Rgba([20, 40, 60, 200]));

        apply_album_art_corner_mask(&mut opaque, 0.25);
        apply_album_art_corner_mask(&mut translucent, 0.25);

        let coverage =
            (4.5 - ((1.0_f32 - 3.5).powi(2) + (0.0_f32 - 3.5).powi(2)).sqrt()).clamp(0.0, 1.0);
        let opaque_boundary = (255.0 * coverage).round() as u8;
        let translucent_boundary = (200.0 * coverage).round() as u8;

        assert_eq!(opaque.get_pixel(0, 0).0[3], 0);
        assert_eq!(opaque.get_pixel(1, 0).0[3], opaque_boundary);
        assert_eq!(translucent.get_pixel(1, 0).0[3], translucent_boundary);
        assert!(translucent_boundary < opaque_boundary);
        assert_eq!(translucent.get_pixel(4, 4).0[3], 200);
        for (x, y) in [(1, 0), (14, 0), (1, 15), (14, 15)] {
            assert_eq!(translucent.get_pixel(x, y).0[3], translucent_boundary);
        }
    }

    #[test]
    fn rectangular_art_is_center_cropped_before_masking() {
        let mut source = RgbaImage::new(6, 4);
        for (x, _y, pixel) in source.enumerate_pixels_mut() {
            *pixel = Rgba([x as u8, 40, 60, 255]);
        }

        let prepared = prepare_album_art_image(DynamicImage::ImageRgba8(source), 0.25);

        assert_eq!(prepared.dimensions(), (4, 4));
        assert_eq!(prepared.get_pixel(0, 2).0[0], 1);
        assert_eq!(prepared.get_pixel(3, 2).0[0], 4);
        assert!(prepared.get_pixel(0, 0).0[3] < 255);
    }

    #[test]
    fn zero_radius_leaves_image_unchanged() {
        let mut image = RgbaImage::from_pixel(4, 4, Rgba([20, 40, 60, 123]));
        let original = image.clone();

        apply_album_art_corner_mask(&mut image, 0.0);

        assert_eq!(image, original);
    }

    #[test]
    fn empty_and_tiny_images_are_safe_and_symmetric() {
        let empty = prepare_album_art_image(DynamicImage::ImageRgba8(RgbaImage::new(0, 3)), 0.25);
        assert_eq!(empty.dimensions(), (0, 0));

        let mut single = RgbaImage::from_pixel(1, 1, Rgba([20, 40, 60, 123]));
        apply_album_art_corner_mask(&mut single, 0.5);
        assert_eq!(single.get_pixel(0, 0).0[3], 123);

        let mut two_by_two = RgbaImage::from_pixel(2, 2, Rgba([20, 40, 60, 200]));
        apply_album_art_corner_mask(&mut two_by_two, 0.5);
        let alpha = two_by_two.get_pixel(0, 0).0[3];
        assert!((1..200).contains(&alpha));
        assert!(two_by_two.pixels().all(|pixel| pixel.0[3] == alpha));
    }
}

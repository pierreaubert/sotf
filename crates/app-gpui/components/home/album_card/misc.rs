use crate::album_art_mask::prepare_album_art_image;
use gpui::*;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

const ALBUM_ART_IMAGE_CACHE_LIMIT: usize = 512;
const PNG_MAGIC: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const JPEG_MAGIC: &[u8; 3] = b"\xff\xd8\xff";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct AlbumArtImageCacheKey {
    len: usize,
    hash: u64,
    corner_radius_ratio_bits: u32,
}

impl AlbumArtImageCacheKey {
    fn from_bytes(bytes: &[u8], corner_radius_ratio: f32) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut hasher);
        Self {
            len: bytes.len(),
            hash: hasher.finish(),
            corner_radius_ratio_bits: corner_radius_ratio.to_bits(),
        }
    }
}

#[derive(Default)]
struct AlbumArtImageCache {
    images: HashMap<AlbumArtImageCacheKey, Arc<Image>>,
    order: VecDeque<AlbumArtImageCacheKey>,
}

impl AlbumArtImageCache {
    fn get_or_insert_with(
        &mut self,
        key: AlbumArtImageCacheKey,
        build: impl FnOnce() -> Arc<Image>,
    ) -> Arc<Image> {
        if let Some(image) = self.images.get(&key) {
            return Arc::clone(image);
        }

        let image = build();
        self.images.insert(key, Arc::clone(&image));
        self.order.push_back(key);
        self.evict_if_needed();
        image
    }

    fn evict_if_needed(&mut self) {
        while self.images.len() > ALBUM_ART_IMAGE_CACHE_LIMIT {
            if let Some(key) = self.order.pop_front() {
                self.images.remove(&key);
            } else {
                break;
            }
        }
    }
}

std::thread_local! {
    static ALBUM_ART_IMAGE_CACHE: RefCell<AlbumArtImageCache> =
        RefCell::new(AlbumArtImageCache::default());
}

/// Create an image from thumbnail bytes
///
/// Thumbnails are stored as PNG in the database for optimal rendering.
/// This function handles both PNG (new format) and JPEG (legacy format) for backward compatibility.
pub(super) fn image_from_jpeg_bytes(bytes: &[u8], corner_radius_ratio: f32) -> Arc<Image> {
    let key = AlbumArtImageCacheKey::from_bytes(bytes, corner_radius_ratio);
    ALBUM_ART_IMAGE_CACHE.with(|cache| {
        cache
            .borrow_mut()
            .get_or_insert_with(key, || decode_album_art_image(bytes, corner_radius_ratio))
    })
}

fn decode_album_art_image(bytes: &[u8], corner_radius_ratio: f32) -> Arc<Image> {
    if (bytes.starts_with(PNG_MAGIC) || bytes.starts_with(JPEG_MAGIC))
        && let Ok(image) = image::load_from_memory(bytes)
    {
        let rgba = prepare_album_art_image(image, corner_radius_ratio);

        let mut png_bytes = Vec::new();
        if rgba
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .is_ok()
        {
            return Arc::new(Image::from_bytes(ImageFormat::Png, png_bytes));
        }
    }

    // Last resort: pass through as-is
    Arc::new(Image::from_bytes(ImageFormat::Png, bytes.to_vec()))
}

pub(super) fn text_size_at_least(size: Rems, min_font_size_px: f32, effective_rem_px: f32) -> Rems {
    rems(size.0.max(min_font_size_px / effective_rem_px.max(1.0)))
}

/// Get the audio format (e.g., "FLAC", "MP3") from a file path extension
pub fn get_format_from_path(path: &std::path::Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str().map(|s| s.to_uppercase()))
}

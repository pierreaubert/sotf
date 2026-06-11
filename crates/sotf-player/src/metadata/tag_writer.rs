use super::{MetadataError, MetadataPatch};
use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::prelude::{Accessor, TagExt};
use lofty::probe::Probe;
use lofty::tag::{ItemKey, Tag};
use std::path::Path;

pub struct TagWriter;

impl TagWriter {
    pub fn is_supported(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| {
                matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "mp3" | "flac" | "ogg" | "oga" | "opus" | "m4a" | "mp4"
                )
            })
            .unwrap_or(false)
    }

    pub fn unsupported_reason(path: &Path) -> Option<String> {
        (!Self::is_supported(path)).then(|| {
            let ext = path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("unknown");
            format!("metadata writing is not enabled for .{ext} files")
        })
    }

    pub fn write_patch(path: &Path, patch: &MetadataPatch) -> Result<(), MetadataError> {
        if !Self::is_supported(path) {
            return Err(MetadataError::TagWrite(
                Self::unsupported_reason(path).unwrap_or_else(|| "unsupported format".to_string()),
            ));
        }

        let mut tagged_file = Probe::open(path)
            .map_err(|err| MetadataError::TagWrite(err.to_string()))?
            .read()
            .map_err(|err| MetadataError::TagWrite(err.to_string()))?;

        if tagged_file.primary_tag().is_none() {
            let tag_type = tagged_file.primary_tag_type();
            tagged_file.insert_tag(Tag::new(tag_type));
        }

        let tag = tagged_file
            .primary_tag_mut()
            .ok_or_else(|| MetadataError::TagWrite("could not create primary tag".to_string()))?;

        if let Some(value) = &patch.title {
            tag.set_title(value.clone());
        }
        if let Some(value) = &patch.artist {
            tag.set_artist(value.clone());
        }
        if let Some(value) = &patch.album_title {
            tag.set_album(value.clone());
        }
        if let Some(value) = &patch.album_artist {
            tag.insert_text(ItemKey::AlbumArtist, value.clone());
        }
        if let Some(value) = patch.year {
            tag.set_year(value);
        }
        if let Some(value) = &patch.genre {
            tag.set_genre(value.clone());
        }
        if let Some(value) = &patch.composer {
            tag.insert_text(ItemKey::Composer, value.clone());
        }
        if let Some(value) = patch.disc_number {
            tag.insert_text(ItemKey::DiscNumber, value.to_string());
        }
        if let Some(value) = patch.track_number {
            tag.set_track(value);
        }
        if let Some(value) = &patch.conductor {
            tag.insert_text(ItemKey::Conductor, value.clone());
        }
        if let Some(value) = &patch.performer {
            tag.insert_text(ItemKey::Performer, value.clone());
        }
        if let Some(value) = &patch.isrc {
            tag.insert_text(ItemKey::Isrc, value.clone());
        }
        if let Some(value) = &patch.ensemble {
            tag.insert_text(ItemKey::Unknown("ENSEMBLE".to_string()), value.clone());
        }
        if let Some(value) = &patch.edition {
            tag.insert_text(ItemKey::Unknown("VERSION".to_string()), value.clone());
        }

        tag.save_to_path(path, WriteOptions::default())
            .map_err(|err| MetadataError::TagWrite(err.to_string()))
    }
}

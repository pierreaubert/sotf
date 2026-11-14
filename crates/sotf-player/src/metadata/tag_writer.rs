use super::{MetadataError, MetadataPatch};
use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::prelude::{Accessor, TagExt};
use lofty::probe::Probe;
use lofty::tag::items::Timestamp;
use lofty::tag::{ItemKey, Tag, TagType};
use std::path::Path;

#[derive(Debug)]
pub struct TagWriter;

impl TagWriter {
    #[must_use]
    pub fn is_supported(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| {
                matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "mp3" | "flac" | "ogg" | "oga" | "opus" | "m4a" | "mp4"
                )
            })
    }

    #[must_use]
    pub fn unsupported_reason(path: &Path) -> Option<String> {
        (!Self::is_supported(path)).then(|| {
            let ext = path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("unknown");
            format!("metadata writing is not enabled for .{ext} files")
        })
    }

    /// Write changes.
    ///
    /// # Errors
    /// Returns an error if the file format does not support writes or tag writing fails.
    pub fn write_patch(path: &Path, changes: &MetadataPatch) -> Result<(), MetadataError> {
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

        if let Some(value) = &changes.title {
            tag.set_title(value.clone());
        }
        if let Some(value) = &changes.artist {
            tag.set_artist(value.clone());
        }
        if let Some(value) = &changes.album_title {
            tag.set_album(value.clone());
        }
        if let Some(value) = &changes.album_artist {
            tag.insert_text(ItemKey::AlbumArtist, value.clone());
        }
        if let Some(value) = changes.year {
            let year = u16::try_from(value).map_err(|_error| {
                MetadataError::TagWrite(format!("year {value} is out of range"))
            })?;
            tag.set_date(Timestamp {
                year,
                ..Timestamp::default()
            });
        }
        if let Some(value) = &changes.genre {
            tag.set_genre(value.clone());
        }
        if let Some(value) = &changes.composer {
            tag.insert_text(ItemKey::Composer, value.clone());
        }
        if let Some(value) = changes.disc_number {
            tag.insert_text(ItemKey::DiscNumber, value.to_string());
        }
        if let Some(value) = changes.track_number {
            tag.set_track(value);
        }
        if let Some(value) = &changes.conductor {
            tag.insert_text(ItemKey::Conductor, value.clone());
        }
        if let Some(value) = &changes.performer {
            tag.insert_text(ItemKey::Performer, value.clone());
        }
        if let Some(value) = &changes.isrc {
            tag.insert_text(ItemKey::Isrc, value.clone());
        }
        if changes.ensemble.is_some() || changes.edition.is_some() {
            *tag = with_custom_fields(tag.clone(), changes)?;
        }

        // Lofty's generic Vorbis writer omits companion items. Save the native
        // tag so new and previously stored custom fields survive every edit.
        if tag.tag_type() == TagType::VorbisComments {
            return lofty::ogg::tag::VorbisComments::from(tag.clone())
                .save_to_path(path, WriteOptions::default())
                .map_err(|err| MetadataError::TagWrite(err.to_string()));
        }

        tag.save_to_path(path, WriteOptions::default())
            .map_err(|err| MetadataError::TagWrite(err.to_string()))
    }
}

// Lofty preserves unmapped native items in a Tag's companion tag. Use native
// keys for these fields, then convert back so the complete tag is saved once.
fn with_custom_fields(tag: Tag, changes: &MetadataPatch) -> Result<Tag, MetadataError> {
    let fields = [
        ("ENSEMBLE", &changes.ensemble),
        ("VERSION", &changes.edition),
    ];
    match tag.tag_type() {
        TagType::Id3v2 => {
            let mut native = lofty::id3::v2::Id3v2Tag::from(tag);
            for (key, value) in fields {
                if let Some(value) = value {
                    native.insert_user_text(key.to_string(), value.clone());
                }
            }
            Ok(native.into())
        }
        TagType::VorbisComments => {
            let mut native = lofty::ogg::tag::VorbisComments::from(tag);
            for (key, value) in fields {
                if let Some(value) = value {
                    native.insert(key.to_string(), value.clone());
                }
            }
            Ok(native.into())
        }
        TagType::Mp4Ilst => {
            use lofty::mp4::{Atom, AtomData, AtomIdent, Ilst};
            let mut native = Ilst::from(tag);
            for (key, value) in fields {
                if let Some(value) = value {
                    native.insert(Atom::new(
                        AtomIdent::Freeform {
                            mean: "com.apple.iTunes".into(),
                            name: key.into(),
                        },
                        AtomData::UTF8(value.clone()),
                    ));
                }
            }
            Ok(native.into())
        }
        kind => Err(MetadataError::TagWrite(format!(
            "custom fields are unsupported for {kind:?}"
        ))),
    }
}

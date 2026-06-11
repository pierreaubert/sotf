mod controller;
mod musicbrainz;
mod sidecar;
mod tag_writer;
mod types;

pub use controller::MetadataController;
pub use musicbrainz::{MetadataProvider, MusicBrainzProvider};
pub use sidecar::{ALBUM_SIDECAR_FILE, AlbumMetadataSidecar};
pub use tag_writer::TagWriter;
pub use types::*;

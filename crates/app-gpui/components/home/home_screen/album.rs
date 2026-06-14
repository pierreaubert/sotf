use sotf_audio_player::Album;
use std::collections::BTreeSet;

pub(super) fn row_album_keys(albums: &[&Album], limit: usize) -> BTreeSet<String> {
    albums
        .iter()
        .take(limit)
        .map(|album| album_key(album))
        .collect()
}

pub(super) fn album_key(album: &Album) -> String {
    if let Some(id) = album.id {
        return format!("id:{id}");
    }
    if let Some(uuid) = album.uuid.as_ref() {
        return format!("uuid:{uuid}");
    }
    format!(
        "meta:{}:{}:{:?}",
        album.artist().to_lowercase(),
        album.title.to_lowercase(),
        album.year
    )
}

pub(super) fn album_genres(album: &Album) -> Vec<String> {
    let mut genres = BTreeSet::new();
    for track in &album.tracks {
        if let Some(genre) = track.genre.as_ref() {
            let trimmed = genre.trim();
            if !trimmed.is_empty() {
                genres.insert(trimmed.to_string());
            }
        }
    }
    genres.into_iter().collect()
}

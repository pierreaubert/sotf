use super::album::album_genres;
use super::album::album_key;
use super::album::row_album_keys;
use super::misc::arc_album_refs;
use super::misc::prioritize_cover_refs;
use super::misc::sort_album_refs_by_listening;
use super::misc::stable_album_hash;
use super::types::HomeShelf;
use super::types::RemoteHomeShelf;
use super::types::top_genre_shelves;
use sotf_audio_player::{Album, sotf_api_client::SotfApiAlbum};
use std::collections::BTreeSet;

pub(super) fn build_home_shelves(
    albums: &[Album],
    collapsed_limit: usize,
    expanded_limit: usize,
) -> Vec<HomeShelf> {
    let display_limit = expanded_limit.max(collapsed_limit);
    let favorite = prioritize_cover_refs(sort_album_refs_by_listening(
        albums.iter().filter(|album| album.is_favorite).collect(),
    ));
    let top_listened = prioritize_cover_refs(sort_album_refs_by_listening(albums.iter().collect()));
    let favorite_albums = if favorite.is_empty() {
        top_listened
    } else {
        favorite
    };
    let favorite_row = row_album_keys(&favorite_albums, collapsed_limit);
    let recommended = prioritize_cover_refs(build_recommended(albums, &favorite_row));
    let mut first_two_rows = favorite_row.clone();
    first_two_rows.extend(row_album_keys(&recommended, collapsed_limit));
    let discover = prioritize_cover_refs(build_discover(albums, &first_two_rows));

    let mut shelves = vec![
        HomeShelf {
            id: "favorite".to_string(),
            title: "Favorite".to_string(),
            total_count: favorite_albums.len(),
            albums: arc_album_refs(&favorite_albums, display_limit),
        },
        HomeShelf {
            id: "recommended".to_string(),
            title: "Recommended".to_string(),
            total_count: recommended.len(),
            albums: arc_album_refs(&recommended, display_limit),
        },
        HomeShelf {
            id: "discover".to_string(),
            title: "Discover".to_string(),
            total_count: discover.len(),
            albums: arc_album_refs(&discover, display_limit),
        },
    ];

    shelves.extend(top_genre_shelves(albums, display_limit));
    shelves
}

pub(super) fn build_remote_home_shelves(albums: &[SotfApiAlbum]) -> Vec<RemoteHomeShelf> {
    let mut top = albums.to_vec();
    top.sort_by(|a, b| {
        b.play_count
            .cmp(&a.play_count)
            .then_with(|| a.artist.cmp(&b.artist))
            .then_with(|| a.title.cmp(&b.title))
    });

    let mut recent = albums.to_vec();
    recent.sort_by(|a, b| {
        b.year
            .unwrap_or(0)
            .cmp(&a.year.unwrap_or(0))
            .then_with(|| a.artist.cmp(&b.artist))
            .then_with(|| a.title.cmp(&b.title))
    });

    let favorites = top
        .iter()
        .filter(|album| album.is_favorite)
        .cloned()
        .collect::<Vec<_>>();

    let mut shelves = Vec::new();
    if !favorites.is_empty() {
        shelves.push(RemoteHomeShelf {
            id: "remote-favorites".to_string(),
            title: "Favorites".to_string(),
            albums: favorites,
        });
    }

    shelves.push(RemoteHomeShelf {
        id: "remote-albums".to_string(),
        title: if top.iter().any(|album| album.play_count > 0) {
            "Top Albums".to_string()
        } else {
            "Albums".to_string()
        },
        albums: top,
    });

    if recent.iter().any(|album| album.year.is_some()) {
        shelves.push(RemoteHomeShelf {
            id: "remote-recent".to_string(),
            title: "Recently Released".to_string(),
            albums: recent,
        });
    }

    shelves
}

pub(super) fn build_recommended<'a>(
    albums: &'a [Album],
    excluded: &BTreeSet<String>,
) -> Vec<&'a Album> {
    let mut seed_genres = BTreeSet::new();
    let mut seed_artists = BTreeSet::new();

    for album in sort_album_refs_by_listening(albums.iter().collect())
        .into_iter()
        .take(12)
    {
        if album.is_favorite || album.play_count > 0 {
            seed_artists.insert(album.artist().to_lowercase());
            for genre in album_genres(&album) {
                seed_genres.insert(genre.to_lowercase());
            }
        }
    }

    let mut scored = albums
        .iter()
        .filter(|album| !album.is_favorite && !excluded.contains(&album_key(album)))
        .map(|album| {
            let genre_score = album_genres(&album)
                .iter()
                .filter(|genre| seed_genres.contains(&genre.to_lowercase()))
                .count();
            let artist_score = usize::from(seed_artists.contains(&album.artist().to_lowercase()));
            let score = genre_score * 4 + artist_score * 2 + album.play_count.min(4);
            (score, album)
        })
        .filter(|(score, _album)| *score > 0)
        .collect::<Vec<_>>();

    scored.sort_by(|(score_a, a), (score_b, b)| {
        score_b
            .cmp(score_a)
            .then_with(|| b.play_count.cmp(&a.play_count))
            .then_with(|| a.title.cmp(&b.title))
    });

    let recommended = scored
        .into_iter()
        .map(|(_score, album)| album)
        .collect::<Vec<_>>();
    if recommended.is_empty() {
        sort_album_refs_by_listening(
            albums
                .iter()
                .filter(|album| !excluded.contains(&album_key(album)))
                .collect(),
        )
    } else {
        recommended
    }
}

pub(super) fn build_discover<'a>(
    albums: &'a [Album],
    excluded: &BTreeSet<String>,
) -> Vec<&'a Album> {
    let mut albums = albums
        .iter()
        .filter(|album| !excluded.contains(&album_key(album)))
        .collect::<Vec<_>>();
    albums.sort_by_key(|album| stable_album_hash(album));
    albums
}

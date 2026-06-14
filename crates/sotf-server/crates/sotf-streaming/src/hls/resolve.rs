use super::consts::MAX_PLAYLIST_BYTES;
use super::fetch::fetch_text;
use super::hls_byte_range::HlsByteRange;
use super::parse::parse_master_playlist;
use super::parse::parse_media_playlist;
use super::types::PendingByteRange;
use super::types::ResolvedPlaylist;
use reqwest::blocking::Client;
use std::io::{self};
use url::Url;

pub(super) fn resolve_playlist(
    client: &Client,
    playlist_url: &Url,
    playlist_text: &str,
) -> io::Result<ResolvedPlaylist> {
    if let Some(variant) = parse_master_playlist(playlist_url, playlist_text)? {
        let variant_text = fetch_text(client, &variant, MAX_PLAYLIST_BYTES)?;
        parse_media_playlist(&variant, &variant_text)
    } else {
        parse_media_playlist(playlist_url, playlist_text)
    }
}

pub(super) fn resolve_byte_range(
    pending: PendingByteRange,
    last_end: &mut Option<u64>,
) -> io::Result<HlsByteRange> {
    let offset = pending.offset.unwrap_or_else(|| last_end.unwrap_or(0));
    let range = HlsByteRange {
        offset,
        length: pending.length,
    };
    *last_end = Some(range.end_exclusive()?);
    Ok(range)
}

//! Snapshot tests for MPD protocol response strings.

use sotf_mpd::{MpdError, MpdResponse, kv};

#[test]
fn snapshot_empty_ok_response() {
    insta::assert_snapshot!(MpdResponse::ok().format());
}

#[test]
fn snapshot_status_like_response() {
    let resp = MpdResponse::ok_with(vec![
        kv("volume", 75),
        kv("repeat", 0),
        kv("random", 0),
        kv("single", 0),
        kv("consume", 0),
        kv("playlist", 2),
        kv("playlistlength", 1),
        kv("state", "stop"),
    ]);
    insta::assert_snapshot!(resp.format());
}

#[test]
fn snapshot_currentsong_like_response() {
    let resp = MpdResponse::ok_with(vec![
        kv("file", "music/a.flac"),
        kv("Title", "Alpha"),
        kv("Artist", "Artist A"),
        kv("Pos", 0),
        kv("Id", 1),
    ]);
    insta::assert_snapshot!(resp.format());
}

#[test]
fn snapshot_error_response() {
    let err = MpdError::unknown_command("not_a_command");
    let resp = MpdResponse::Error(err);
    insta::assert_snapshot!(resp.format());
}

#[test]
fn snapshot_list_ok_response() {
    insta::assert_snapshot!(MpdResponse::ListOk.format());
}

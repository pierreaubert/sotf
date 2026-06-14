use super::error::MpdError;
use super::mpd_response::MpdResponse;
use super::parse::parse_command;
use super::types::MpdCommand;
use super::types::MpdErrorCode;
use super::types::kv;

use super::*;

#[test]
fn test_parse_simple_commands() {
    assert!(matches!(parse_command("ping"), Ok(MpdCommand::Ping)));
    assert!(matches!(parse_command("stop"), Ok(MpdCommand::Stop)));
    assert!(matches!(parse_command("next"), Ok(MpdCommand::Next)));
    assert!(matches!(
        parse_command("previous"),
        Ok(MpdCommand::Previous)
    ));
    assert!(matches!(parse_command("status"), Ok(MpdCommand::Status)));
    assert!(matches!(
        parse_command("currentsong"),
        Ok(MpdCommand::CurrentSong)
    ));
    assert!(matches!(parse_command("clear"), Ok(MpdCommand::Clear)));
    assert!(matches!(parse_command("close"), Ok(MpdCommand::Close)));
}

#[test]
fn test_parse_play() {
    assert!(matches!(parse_command("play"), Ok(MpdCommand::Play(None))));
    assert!(matches!(
        parse_command("play 5"),
        Ok(MpdCommand::Play(Some(5)))
    ));
}

#[test]
fn test_parse_pause() {
    assert!(matches!(
        parse_command("pause"),
        Ok(MpdCommand::Pause(None))
    ));
    assert!(matches!(
        parse_command("pause 1"),
        Ok(MpdCommand::Pause(Some(true)))
    ));
    assert!(matches!(
        parse_command("pause 0"),
        Ok(MpdCommand::Pause(Some(false)))
    ));
}

#[test]
fn test_parse_setvol() {
    match parse_command("setvol 75") {
        Ok(MpdCommand::SetVol(vol)) => assert_eq!(vol, 75),
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn test_parse_seek() {
    match parse_command("seek 3 120.5") {
        Ok(MpdCommand::Seek(pos, time)) => {
            assert_eq!(pos, 3);
            assert!((time - 120.5).abs() < 0.01);
        }
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn test_parse_seekcur() {
    match parse_command("seekcur 45.2") {
        Ok(MpdCommand::SeekCur(time)) => assert!((time - 45.2).abs() < 0.01),
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn test_parse_add_quoted() {
    match parse_command(r#"add "path/to/song.flac""#) {
        Ok(MpdCommand::Add(uri)) => assert_eq!(uri, "path/to/song.flac"),
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn test_parse_add_unquoted() {
    match parse_command("add path/to/song.flac") {
        Ok(MpdCommand::Add(uri)) => assert_eq!(uri, "path/to/song.flac"),
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn test_parse_search() {
    match parse_command(r#"search artist "Pink Floyd""#) {
        Ok(MpdCommand::Search(filters)) => {
            assert_eq!(filters.len(), 1);
            assert_eq!(filters[0].tag, "artist");
            assert_eq!(filters[0].value, "Pink Floyd");
        }
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn test_parse_list() {
    match parse_command("list album") {
        Ok(MpdCommand::List(tag, filters)) => {
            assert_eq!(tag, "album");
            assert!(filters.is_empty());
        }
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn test_parse_unknown_command() {
    assert!(matches!(
        parse_command("foobar"),
        Err(MpdError {
            code: MpdErrorCode::UnknownCmd,
            ..
        })
    ));
}

#[test]
fn test_parse_case_insensitive() {
    assert!(matches!(parse_command("PLAY"), Ok(MpdCommand::Play(None))));
    assert!(matches!(parse_command("Status"), Ok(MpdCommand::Status)));
}

#[test]
fn test_mpd_error_format() {
    let err = MpdError::new(MpdErrorCode::Arg, "seek", "invalid argument");
    assert_eq!(err.format(), "ACK [2@0] {seek} invalid argument\n");
}

#[test]
fn test_mpd_response_format() {
    let resp = MpdResponse::ok_with(vec![kv("volume", 75), kv("state", "play")]);
    let formatted = resp.format();
    assert!(formatted.contains("volume: 75\n"));
    assert!(formatted.contains("state: play\n"));
    assert!(formatted.ends_with("OK\n"));
}

#[test]
fn test_parse_command_list() {
    assert!(matches!(
        parse_command("command_list_begin"),
        Ok(MpdCommand::CommandListBegin)
    ));
    assert!(matches!(
        parse_command("command_list_ok_begin"),
        Ok(MpdCommand::CommandListOkBegin)
    ));
    assert!(matches!(
        parse_command("command_list_end"),
        Ok(MpdCommand::CommandListEnd)
    ));
}

#[test]
fn test_parse_idle() {
    match parse_command("idle player mixer") {
        Ok(MpdCommand::Idle(subsystems)) => {
            assert_eq!(subsystems, vec!["player", "mixer"]);
        }
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn test_parse_playlistinfo_range() {
    match parse_command("playlistinfo 5:10") {
        Ok(MpdCommand::PlaylistInfo(Some((5, Some(10))))) => {}
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn test_parse_random_repeat() {
    assert!(matches!(
        parse_command("random 1"),
        Ok(MpdCommand::Random(true))
    ));
    assert!(matches!(
        parse_command("repeat 0"),
        Ok(MpdCommand::Repeat(false))
    ));
}

// ----- Regression: `volume` no longer silently wraps via `as i8` -----

#[test]
fn test_parse_volume_in_range() {
    match parse_command("volume -50") {
        Ok(MpdCommand::Volume(d)) => assert_eq!(d, -50),
        other => panic!("unexpected: {:?}", other),
    }
    match parse_command("volume 100") {
        Ok(MpdCommand::Volume(d)) => assert_eq!(d, 100),
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn test_parse_volume_out_of_range_rejected() {
    // `volume 200` used to silently wrap to -56 because of `as i8`.
    // It must now be rejected with ACK_ERROR_ARG instead.
    for input in ["volume 200", "volume -200", "volume 101", "volume -101"] {
        match parse_command(input) {
            Err(MpdError {
                code: MpdErrorCode::Arg,
                ..
            }) => {}
            other => panic!("expected Arg error for {input:?}, got {other:?}"),
        }
    }
}

// ----- Regression: `pause` polarity matches the MPD spec -----
// https://mpd.readthedocs.io/en/latest/protocol.html#command-pause
//   - `pause`   → toggle (None)
//   - `pause 1` → pause   (Some(true))
//   - `pause 0` → resume  (Some(false))
#[test]
fn test_parse_pause_polarity_spec() {
    assert!(matches!(
        parse_command("pause"),
        Ok(MpdCommand::Pause(None))
    ));
    assert!(matches!(
        parse_command("pause 1"),
        Ok(MpdCommand::Pause(Some(true)))
    ));
    assert!(matches!(
        parse_command("pause 0"),
        Ok(MpdCommand::Pause(Some(false)))
    ));
}

// ----- Regression: quoted-token UTF-8 preservation -----

#[test]
fn test_parse_quoted_token_multibyte_utf8() {
    // A multibyte codepoint inside a quoted string must round-trip
    // exactly; the leading byte of a UTF-8 sequence used to be cast
    // through `as char` after an escape, corrupting the codepoint.
    match parse_command("add \"caf\u{00e9}\"") {
        Ok(MpdCommand::Add(uri)) => assert_eq!(uri, "café"),
        other => panic!("unexpected: {:?}", other),
    }
    // Escape immediately before multibyte content.
    match parse_command("add \"a\\\"\u{1f3b5}b\"") {
        Ok(MpdCommand::Add(uri)) => assert_eq!(uri, "a\"\u{1f3b5}b"),
        other => panic!("unexpected: {:?}", other),
    }
    // Multiple escapes — `\"...\"` used to lose every escape after the
    // first because the second branch fell through to `collect_until_quote`.
    match parse_command(r#"add "a\"b\"c""#) {
        Ok(MpdCommand::Add(uri)) => assert_eq!(uri, r#"a"b"c"#),
        other => panic!("unexpected: {:?}", other),
    }
    // Escaped backslash.
    match parse_command(r#"add "a\\b""#) {
        Ok(MpdCommand::Add(uri)) => assert_eq!(uri, r"a\b"),
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn test_parse_unterminated_quote_is_arg_error() {
    // Previously returned `Ok` with the dangling tail; clients would
    // desync silently. Now must be ACK_ERROR_ARG.
    match parse_command(r#"add "unterminated"#) {
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        }) => {}
        other => panic!("expected Arg error, got {other:?}"),
    }
}

#[test]
fn test_parse_single_rejects_unknown_value() {
    // `single 2` is outside the documented {0,1,oneshot} set.
    // Project rule: crash hard for unknown values rather than coerce.
    match parse_command("single 2") {
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        }) => {}
        other => panic!("expected Arg error, got {other:?}"),
    }
}

// ----- Additional command coverage -----

#[test]
fn test_parse_empty_command() {
    match parse_command("") {
        Err(MpdError {
            code: MpdErrorCode::UnknownCmd,
            ..
        }) => {}
        other => panic!("expected UnknownCmd error, got {other:?}"),
    }
    match parse_command("   ") {
        Err(MpdError {
            code: MpdErrorCode::UnknownCmd,
            ..
        }) => {}
        other => panic!("expected UnknownCmd error for whitespace, got {other:?}"),
    }
}

#[test]
fn test_parse_playid() {
    assert!(matches!(
        parse_command("playid"),
        Ok(MpdCommand::PlayId(None))
    ));
    assert!(matches!(
        parse_command("playid 7"),
        Ok(MpdCommand::PlayId(Some(7)))
    ));
}

#[test]
fn test_parse_seekid() {
    match parse_command("seekid 42 88.5") {
        Ok(MpdCommand::SeekId(id, time)) => {
            assert_eq!(id, 42);
            assert!((time - 88.5).abs() < 0.01);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn test_parse_setvol_bounds() {
    match parse_command("setvol 0") {
        Ok(MpdCommand::SetVol(v)) => assert_eq!(v, 0),
        other => panic!("unexpected: {other:?}"),
    }
    match parse_command("setvol 100") {
        Ok(MpdCommand::SetVol(v)) => assert_eq!(v, 100),
        other => panic!("unexpected: {other:?}"),
    }
    match parse_command("setvol 101") {
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        }) => {}
        other => panic!("expected Arg error for setvol 101, got {other:?}"),
    }
}

#[test]
fn test_parse_single_modes() {
    assert!(matches!(
        parse_command("single 0"),
        Ok(MpdCommand::Single(SingleMode::Off))
    ));
    assert!(matches!(
        parse_command("single 1"),
        Ok(MpdCommand::Single(SingleMode::On))
    ));
    assert!(matches!(
        parse_command("single oneshot"),
        Ok(MpdCommand::Single(SingleMode::OneShot))
    ));
}

#[test]
fn test_parse_consume_repeat_random() {
    assert!(matches!(
        parse_command("consume 1"),
        Ok(MpdCommand::Consume(true))
    ));
    assert!(matches!(
        parse_command("consume 0"),
        Ok(MpdCommand::Consume(false))
    ));
    assert!(matches!(
        parse_command("repeat 1"),
        Ok(MpdCommand::Repeat(true))
    ));
    assert!(matches!(
        parse_command("random 0"),
        Ok(MpdCommand::Random(false))
    ));
}

#[test]
fn test_parse_playlistid() {
    assert!(matches!(
        parse_command("playlistid"),
        Ok(MpdCommand::PlaylistId(None))
    ));
    assert!(matches!(
        parse_command("playlistid 12"),
        Ok(MpdCommand::PlaylistId(Some(12)))
    ));
}

#[test]
fn test_parse_delete_and_deleteid() {
    match parse_command("delete 3") {
        Ok(MpdCommand::Delete(pos)) => assert_eq!(pos, 3),
        other => panic!("unexpected: {other:?}"),
    }
    match parse_command("deleteid 99") {
        Ok(MpdCommand::DeleteId(id)) => assert_eq!(id, 99),
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn test_parse_move_and_swap() {
    match parse_command("move 2 5") {
        Ok(MpdCommand::Move(from, to)) => {
            assert_eq!(from, 2);
            assert_eq!(to, 5);
        }
        other => panic!("unexpected: {other:?}"),
    }
    match parse_command("swap 1 4") {
        Ok(MpdCommand::Swap(p1, p2)) => {
            assert_eq!(p1, 1);
            assert_eq!(p2, 4);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn test_parse_addid() {
    match parse_command("addid \"library/song.flac\"") {
        Ok(MpdCommand::AddId(uri, pos)) => {
            assert_eq!(uri, "library/song.flac");
            assert_eq!(pos, None);
        }
        other => panic!("unexpected: {other:?}"),
    }
    match parse_command("addid \"library/song.flac\" 3") {
        Ok(MpdCommand::AddId(uri, pos)) => {
            assert_eq!(uri, "library/song.flac");
            assert_eq!(pos, Some(3));
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn test_parse_listall_lsinfo_update() {
    assert!(matches!(
        parse_command("listall"),
        Ok(MpdCommand::ListAll(None))
    ));
    assert!(matches!(
        parse_command("listall Music/Rock"),
        Ok(MpdCommand::ListAll(Some(p))) if p == "Music/Rock"
    ));
    assert!(matches!(
        parse_command("lsinfo"),
        Ok(MpdCommand::LsInfo(None))
    ));
    assert!(matches!(
        parse_command("lsinfo Music"),
        Ok(MpdCommand::LsInfo(Some(p))) if p == "Music"
    ));
    assert!(matches!(
        parse_command("update"),
        Ok(MpdCommand::Update(None))
    ));
    assert!(matches!(
        parse_command("update Music"),
        Ok(MpdCommand::Update(Some(p))) if p == "Music"
    ));
}

#[test]
fn test_parse_find_search_count() {
    match parse_command(r#"find artist "Radiohead""#) {
        Ok(MpdCommand::Find(filters)) => {
            assert_eq!(filters.len(), 1);
            assert_eq!(filters[0].tag, "artist");
            assert_eq!(filters[0].value, "Radiohead");
        }
        other => panic!("unexpected: {other:?}"),
    }
    match parse_command(r#"count album "OK Computer" artist "Radiohead""#) {
        Ok(MpdCommand::Count(filters)) => {
            assert_eq!(filters.len(), 2);
            assert_eq!(filters[0].tag, "album");
            assert_eq!(filters[0].value, "OK Computer");
            assert_eq!(filters[1].tag, "artist");
            assert_eq!(filters[1].value, "Radiohead");
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn test_parse_list_with_filters() {
    match parse_command(r#"list album artist "Radiohead""#) {
        Ok(MpdCommand::List(tag, filters)) => {
            assert_eq!(tag, "album");
            assert_eq!(filters.len(), 1);
            assert_eq!(filters[0].tag, "artist");
            assert_eq!(filters[0].value, "Radiohead");
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn test_parse_outputs_and_toggle() {
    assert!(matches!(parse_command("outputs"), Ok(MpdCommand::Outputs)));
    match parse_command("enableoutput 0") {
        Ok(MpdCommand::EnableOutput(id)) => assert_eq!(id, 0),
        other => panic!("unexpected: {other:?}"),
    }
    match parse_command("disableoutput 1") {
        Ok(MpdCommand::DisableOutput(id)) => assert_eq!(id, 1),
        other => panic!("unexpected: {other:?}"),
    }
    match parse_command("toggleoutput 2") {
        Ok(MpdCommand::ToggleOutput(id)) => assert_eq!(id, 2),
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn test_parse_reflection_commands() {
    assert!(matches!(
        parse_command("commands"),
        Ok(MpdCommand::Commands)
    ));
    assert!(matches!(
        parse_command("notcommands"),
        Ok(MpdCommand::NotCommands)
    ));
    assert!(matches!(
        parse_command("tagtypes"),
        Ok(MpdCommand::TagTypes)
    ));
    assert!(matches!(
        parse_command("urlhandlers"),
        Ok(MpdCommand::UrlHandlers)
    ));
    assert!(matches!(
        parse_command("decoders"),
        Ok(MpdCommand::Decoders)
    ));
}

#[test]
fn test_parse_noidle() {
    assert!(matches!(parse_command("noidle"), Ok(MpdCommand::NoIdle)));
}

#[test]
fn test_parse_range_u32_max_no_overflow() {
    let mut parts = super::command_tokenizer::CommandTokenizer::new("4294967295");
    assert_eq!(super::parse::parse_range(&mut parts), None);
}

#[test]
fn test_parse_range_formats() {
    // Open range
    match parse_command("playlistinfo 5:") {
        Ok(MpdCommand::PlaylistInfo(Some((start, end)))) => {
            assert_eq!(start, 5);
            assert_eq!(end, None);
        }
        other => panic!("unexpected: {other:?}"),
    }
    // Single position becomes (pos, Some(pos+1))
    match parse_command("playlistinfo 7") {
        Ok(MpdCommand::PlaylistInfo(Some((start, end)))) => {
            assert_eq!(start, 7);
            assert_eq!(end, Some(8));
        }
        other => panic!("unexpected: {other:?}"),
    }
    // Invalid range token is treated as no range
    assert!(matches!(
        parse_command("playlistinfo abc"),
        Ok(MpdCommand::PlaylistInfo(None))
    ));
}

#[test]
fn test_parse_range_u32_max_via_command() {
    // u32::MAX (4294967295) + 1 would overflow; parse_range must return None
    match parse_command("playlistinfo 4294967295") {
        Ok(MpdCommand::PlaylistInfo(None)) => {}
        other => panic!("expected PlaylistInfo(None) for u32::MAX, got {other:?}"),
    }
}

#[test]
fn test_response_list_ok_format() {
    let resp = MpdResponse::ListOk;
    assert_eq!(resp.format(), "list_OK\n");
}

#[test]
fn test_response_error_format() {
    let err = MpdError::new(MpdErrorCode::NoExist, "playid", "No such song");
    let resp = MpdResponse::Error(err);
    assert_eq!(resp.format(), "ACK [50@0] {playid} No such song\n");
}

#[test]
fn test_tokenizer_require_types_reject_invalid() {
    // Missing argument
    assert!(matches!(
        parse_command("seek 3"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    // Bad integer
    assert!(matches!(
        parse_command("delete abc"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    // Bad boolean
    assert!(matches!(
        parse_command("random maybe"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    // Bad f64
    assert!(matches!(
        parse_command("seekcur abc"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
}

// ----- Regression: trailing tokens must be rejected -----

#[test]
fn test_parse_trailing_tokens_rejected() {
    for input in ["play 1 extra", "stop extra", "pause 1 extra", "next extra"] {
        match parse_command(input) {
            Err(MpdError {
                code: MpdErrorCode::Arg,
                ..
            }) => {}
            other => panic!("expected Arg error for trailing tokens in {input:?}, got {other:?}"),
        }
    }
}

#[test]
fn test_parse_no_trailing_tokens_ok() {
    assert!(matches!(
        parse_command("play 1"),
        Ok(MpdCommand::Play(Some(1)))
    ));
    assert!(matches!(parse_command("stop"), Ok(MpdCommand::Stop)));
    assert!(matches!(
        parse_command("pause"),
        Ok(MpdCommand::Pause(None))
    ));
    assert!(matches!(
        parse_command("pause 1"),
        Ok(MpdCommand::Pause(Some(true)))
    ));
}

#[test]
fn test_parse_password() {
    match parse_command("password secret123") {
        Ok(MpdCommand::Password(pw)) => assert_eq!(pw, "secret123"),
        other => panic!("unexpected: {other:?}"),
    }
    // password with no argument
    match parse_command("password") {
        Ok(MpdCommand::Password(pw)) => assert_eq!(pw, ""),
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn test_parse_stats_shuffle_urlhandlers() {
    assert!(matches!(parse_command("stats"), Ok(MpdCommand::Stats)));
    assert!(matches!(parse_command("shuffle"), Ok(MpdCommand::Shuffle)));
    assert!(matches!(
        parse_command("urlhandlers"),
        Ok(MpdCommand::UrlHandlers)
    ));
}

#[test]
fn test_parse_deleteid() {
    match parse_command("deleteid 42") {
        Ok(MpdCommand::DeleteId(id)) => assert_eq!(id, 42),
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn test_parse_idle_no_subsystems() {
    match parse_command("idle") {
        Ok(MpdCommand::Idle(subsystems)) => assert!(subsystems.is_empty()),
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn test_parse_find_search_count_list_no_filters() {
    assert!(matches!(
        parse_command("find"),
        Ok(MpdCommand::Find(filters)) if filters.is_empty()
    ));
    assert!(matches!(
        parse_command("search"),
        Ok(MpdCommand::Search(filters)) if filters.is_empty()
    ));
    assert!(matches!(
        parse_command("count"),
        Ok(MpdCommand::Count(filters)) if filters.is_empty()
    ));
}

#[test]
fn test_parse_missing_required_arguments() {
    // setvol missing volume
    assert!(matches!(
        parse_command("setvol"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    // volume missing delta
    assert!(matches!(
        parse_command("volume"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    // random missing bool
    assert!(matches!(
        parse_command("random"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    // repeat missing bool
    assert!(matches!(
        parse_command("repeat"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    // single missing mode
    assert!(matches!(
        parse_command("single"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    // consume missing bool
    assert!(matches!(
        parse_command("consume"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    // seekid missing args
    assert!(matches!(
        parse_command("seekid"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    assert!(matches!(
        parse_command("seekid 5"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    // seekcur missing arg
    assert!(matches!(
        parse_command("seekcur"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    // add missing uri
    assert!(matches!(
        parse_command("add"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    // addid missing uri
    assert!(matches!(
        parse_command("addid"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    // delete missing pos
    assert!(matches!(
        parse_command("delete"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    // deleteid missing id
    assert!(matches!(
        parse_command("deleteid"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    // move missing args
    assert!(matches!(
        parse_command("move"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    assert!(matches!(
        parse_command("move 1"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    // swap missing args
    assert!(matches!(
        parse_command("swap"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    assert!(matches!(
        parse_command("swap 1"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    // enableoutput missing id
    assert!(matches!(
        parse_command("enableoutput"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    // disableoutput missing id
    assert!(matches!(
        parse_command("disableoutput"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
    // toggleoutput missing id
    assert!(matches!(
        parse_command("toggleoutput"),
        Err(MpdError {
            code: MpdErrorCode::Arg,
            ..
        })
    ));
}

#[test]
fn test_parse_trailing_tokens_more_commands() {
    for input in [
        "status extra",
        "stats extra",
        "currentsong extra",
        "setvol 50 extra",
        "volume 10 extra",
        "random 1 extra",
        "repeat 0 extra",
        "single 0 extra",
        "consume 1 extra",
        "seek 1 10.0 extra",
        "seekid 1 10.0 extra",
        "seekcur 5.0 extra",
        "add uri extra",
        "delete 0 extra",
        "deleteid 0 extra",
        "clear extra",
        "shuffle extra",
        "outputs extra",
        "enableoutput 0 extra",
        "disableoutput 0 extra",
        "toggleoutput 0 extra",
        "commands extra",
        "notcommands extra",
        "tagtypes extra",
        "urlhandlers extra",
        "decoders extra",
        "command_list_begin extra",
        "command_list_ok_begin extra",
        "command_list_end extra",
        "noidle extra",
    ] {
        match parse_command(input) {
            Err(MpdError {
                code: MpdErrorCode::Arg,
                ..
            }) => {}
            other => panic!("expected Arg error for {input:?}, got {other:?}"),
        }
    }
}

#[test]
fn test_parse_setvol_zero_and_max() {
    match parse_command("setvol 0") {
        Ok(MpdCommand::SetVol(v)) => assert_eq!(v, 0),
        other => panic!("unexpected: {other:?}"),
    }
    match parse_command("setvol 100") {
        Ok(MpdCommand::SetVol(v)) => assert_eq!(v, 100),
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn test_parse_filter_exprs_odd_tokens() {
    // Odd number of tokens after command: last tag has no value and is dropped
    match parse_command("find artist \"Pink Floyd\" album") {
        Ok(MpdCommand::Find(filters)) => {
            assert_eq!(filters.len(), 1);
            assert_eq!(filters[0].tag, "artist");
            assert_eq!(filters[0].value, "Pink Floyd");
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn test_parse_range_directly_open_end() {
    let mut parts = super::command_tokenizer::CommandTokenizer::new("5:");
    match super::parse::parse_range(&mut parts) {
        Some((start, end)) => {
            assert_eq!(start, 5);
            assert_eq!(end, None);
        }
        None => panic!("expected Some"),
    }
}

#[test]
fn test_parse_range_directly_closed() {
    let mut parts = super::command_tokenizer::CommandTokenizer::new("3:7");
    match super::parse::parse_range(&mut parts) {
        Some((start, end)) => {
            assert_eq!(start, 3);
            assert_eq!(end, Some(7));
        }
        None => panic!("expected Some"),
    }
}

#[test]
fn test_parse_range_directly_invalid() {
    let mut parts = super::command_tokenizer::CommandTokenizer::new("abc");
    assert_eq!(super::parse::parse_range(&mut parts), None);
    let mut parts = super::command_tokenizer::CommandTokenizer::new("abc:def");
    assert_eq!(super::parse::parse_range(&mut parts), None);
}

// ============================================================================
// Property-Based Tests
// ============================================================================

mod property_tests {
    use super::super::command_tokenizer::CommandTokenizer;
    use super::super::parse_command;
    use super::super::types::{FilterExpr, MpdCommand, SingleMode};
    use proptest::prelude::*;

    fn simple_token_strategy() -> BoxedStrategy<String> {
        proptest::string::string_regex("[a-zA-Z0-9_.:/@-]+")
            .unwrap()
            .boxed()
    }

    fn simple_path_strategy() -> BoxedStrategy<String> {
        proptest::string::string_regex("[a-zA-Z0-9_/.@-]+")
            .unwrap()
            .boxed()
    }

    fn tag_strategy() -> BoxedStrategy<String> {
        proptest::string::string_regex("[a-z]+").unwrap().boxed()
    }

    fn mpd_command_strategy() -> BoxedStrategy<MpdCommand> {
        let path = simple_path_strategy();
        let tag = tag_strategy();

        prop_oneof![
            Just(MpdCommand::Ping),
            Just(MpdCommand::Stop),
            Just(MpdCommand::Next),
            Just(MpdCommand::Previous),
            Just(MpdCommand::Status),
            Just(MpdCommand::Stats),
            Just(MpdCommand::CurrentSong),
            Just(MpdCommand::Clear),
            Just(MpdCommand::Shuffle),
            Just(MpdCommand::Outputs),
            Just(MpdCommand::Commands),
            Just(MpdCommand::NotCommands),
            Just(MpdCommand::TagTypes),
            Just(MpdCommand::UrlHandlers),
            Just(MpdCommand::Decoders),
            Just(MpdCommand::CommandListBegin),
            Just(MpdCommand::CommandListOkBegin),
            Just(MpdCommand::CommandListEnd),
            Just(MpdCommand::NoIdle),
            prop::option::of(0u32..16u32).prop_map(MpdCommand::Play),
            prop::option::of(0u32..16u32).prop_map(MpdCommand::PlayId),
            prop::option::of(prop::bool::ANY).prop_map(MpdCommand::Pause),
            (0u32..101u32).prop_map(|v| MpdCommand::SetVol(v as u8)),
            (-100i32..=100i32).prop_map(|v| MpdCommand::Volume(v as i8)),
            prop::bool::ANY.prop_map(MpdCommand::Random),
            prop::bool::ANY.prop_map(MpdCommand::Repeat),
            prop::bool::ANY.prop_map(MpdCommand::Consume),
            prop_oneof![
                Just(SingleMode::Off),
                Just(SingleMode::On),
                Just(SingleMode::OneShot)
            ]
            .prop_map(MpdCommand::Single),
            (0u32..16u32, 0u32..1000u32).prop_map(|(pos, time)| MpdCommand::Seek(pos, time as f64)),
            (0u32..16u32, 0u32..1000u32).prop_map(|(id, time)| MpdCommand::SeekId(id, time as f64)),
            (0u32..1000u32).prop_map(|time| MpdCommand::SeekCur(time as f64)),
            prop::option::of(0u32..16u32).prop_map(MpdCommand::PlaylistId),
            (0u32..16u32, prop::option::of(0u32..32u32))
                .prop_map(|(start, end)| MpdCommand::PlaylistInfo(Some((start, end)))),
            Just(MpdCommand::PlaylistInfo(None)),
            path.clone().prop_map(MpdCommand::Add),
            (path.clone(), prop::option::of(0u32..16u32))
                .prop_map(|(uri, pos)| MpdCommand::AddId(uri, pos)),
            (0u32..16u32).prop_map(MpdCommand::Delete),
            (0u32..16u32).prop_map(MpdCommand::DeleteId),
            (0u32..16u32, 0u32..16u32).prop_map(|(a, b)| MpdCommand::Move(a, b)),
            (0u32..16u32, 0u32..16u32).prop_map(|(a, b)| MpdCommand::Swap(a, b)),
            prop::option::of(path.clone()).prop_map(MpdCommand::ListAll),
            prop::option::of(path.clone()).prop_map(MpdCommand::LsInfo),
            prop::option::of(path.clone()).prop_map(MpdCommand::Update),
            (tag.clone(), path.clone())
                .prop_map(|(t, v)| MpdCommand::Find(vec![FilterExpr { tag: t, value: v }])),
            (tag.clone(), path.clone())
                .prop_map(|(t, v)| MpdCommand::Search(vec![FilterExpr { tag: t, value: v }])),
            (tag.clone(), tag.clone(), path.clone()).prop_map(|(t, ft, fv)| {
                MpdCommand::List(t, vec![FilterExpr { tag: ft, value: fv }])
            }),
            (tag.clone(), path.clone())
                .prop_map(|(t, v)| MpdCommand::Count(vec![FilterExpr { tag: t, value: v }])),
            (0u32..16u32).prop_map(MpdCommand::EnableOutput),
            (0u32..16u32).prop_map(MpdCommand::DisableOutput),
            (0u32..16u32).prop_map(MpdCommand::ToggleOutput),
            prop::collection::vec(proptest::string::string_regex("[a-z]+").unwrap(), 0..8)
                .prop_map(MpdCommand::Idle),
        ]
        .boxed()
    }

    fn format_command(cmd: &MpdCommand) -> String {
        match cmd {
            MpdCommand::Ping => "ping".into(),
            MpdCommand::Close => "close".into(),
            MpdCommand::Password(s) => format!("password {}", s),
            MpdCommand::Play(None) => "play".into(),
            MpdCommand::Play(Some(p)) => format!("play {}", p),
            MpdCommand::PlayId(None) => "playid".into(),
            MpdCommand::PlayId(Some(p)) => format!("playid {}", p),
            MpdCommand::Pause(None) => "pause".into(),
            MpdCommand::Pause(Some(true)) => "pause 1".into(),
            MpdCommand::Pause(Some(false)) => "pause 0".into(),
            MpdCommand::Stop => "stop".into(),
            MpdCommand::Next => "next".into(),
            MpdCommand::Previous => "previous".into(),
            MpdCommand::Seek(pos, time) => format!("seek {} {}", pos, *time as u64),
            MpdCommand::SeekId(id, time) => format!("seekid {} {}", id, *time as u64),
            MpdCommand::SeekCur(time) => format!("seekcur {}", *time as u64),
            MpdCommand::SetVol(vol) => format!("setvol {}", vol),
            MpdCommand::Volume(delta) => format!("volume {}", delta),
            MpdCommand::Random(b) => format!("random {}", if *b { 1 } else { 0 }),
            MpdCommand::Repeat(b) => format!("repeat {}", if *b { 1 } else { 0 }),
            MpdCommand::Single(mode) => match mode {
                SingleMode::Off => "single 0".into(),
                SingleMode::On => "single 1".into(),
                SingleMode::OneShot => "single oneshot".into(),
            },
            MpdCommand::Consume(b) => format!("consume {}", if *b { 1 } else { 0 }),
            MpdCommand::Status => "status".into(),
            MpdCommand::Stats => "stats".into(),
            MpdCommand::CurrentSong => "currentsong".into(),
            MpdCommand::PlaylistInfo(None) => "playlistinfo".into(),
            MpdCommand::PlaylistInfo(Some((start, None))) => format!("playlistinfo {}:", start),
            MpdCommand::PlaylistInfo(Some((start, Some(end)))) => {
                format!("playlistinfo {}:{}", start, end)
            }
            MpdCommand::PlaylistId(None) => "playlistid".into(),
            MpdCommand::PlaylistId(Some(id)) => format!("playlistid {}", id),
            MpdCommand::Add(uri) => format!("add {}", uri),
            MpdCommand::AddId(uri, None) => format!("addid {}", uri),
            MpdCommand::AddId(uri, Some(pos)) => format!("addid {} {}", uri, pos),
            MpdCommand::Delete(pos) => format!("delete {}", pos),
            MpdCommand::DeleteId(id) => format!("deleteid {}", id),
            MpdCommand::Clear => "clear".into(),
            MpdCommand::Shuffle => "shuffle".into(),
            MpdCommand::Move(from, to) => format!("move {} {}", from, to),
            MpdCommand::Swap(a, b) => format!("swap {} {}", a, b),
            MpdCommand::ListAll(None) => "listall".into(),
            MpdCommand::ListAll(Some(p)) => format!("listall {}", p),
            MpdCommand::LsInfo(None) => "lsinfo".into(),
            MpdCommand::LsInfo(Some(p)) => format!("lsinfo {}", p),
            MpdCommand::Find(filters) => {
                let mut s = "find".to_string();
                for f in filters {
                    s.push_str(&format!(" {} {}", f.tag, f.value));
                }
                s
            }
            MpdCommand::Search(filters) => {
                let mut s = "search".to_string();
                for f in filters {
                    s.push_str(&format!(" {} {}", f.tag, f.value));
                }
                s
            }
            MpdCommand::List(tag, filters) => {
                let mut s = format!("list {}", tag);
                for f in filters {
                    s.push_str(&format!(" {} {}", f.tag, f.value));
                }
                s
            }
            MpdCommand::Count(filters) => {
                let mut s = "count".to_string();
                for f in filters {
                    s.push_str(&format!(" {} {}", f.tag, f.value));
                }
                s
            }
            MpdCommand::Update(None) => "update".into(),
            MpdCommand::Update(Some(p)) => format!("update {}", p),
            MpdCommand::Outputs => "outputs".into(),
            MpdCommand::EnableOutput(id) => format!("enableoutput {}", id),
            MpdCommand::DisableOutput(id) => format!("disableoutput {}", id),
            MpdCommand::ToggleOutput(id) => format!("toggleoutput {}", id),
            MpdCommand::Commands => "commands".into(),
            MpdCommand::NotCommands => "notcommands".into(),
            MpdCommand::TagTypes => "tagtypes".into(),
            MpdCommand::UrlHandlers => "urlhandlers".into(),
            MpdCommand::Decoders => "decoders".into(),
            MpdCommand::CommandListBegin => "command_list_begin".into(),
            MpdCommand::CommandListOkBegin => "command_list_ok_begin".into(),
            MpdCommand::CommandListEnd => "command_list_end".into(),
            MpdCommand::Idle(subsystems) => {
                let mut s = "idle".to_string();
                for sub in subsystems {
                    s.push(' ');
                    s.push_str(sub);
                }
                s
            }
            MpdCommand::NoIdle => "noidle".into(),
        }
    }

    fn filter_exprs_eq(a: &[FilterExpr], b: &[FilterExpr]) -> bool {
        a.len() == b.len()
            && a.iter()
                .zip(b.iter())
                .all(|(x, y)| x.tag == y.tag && x.value == y.value)
    }

    fn commands_eq(a: &MpdCommand, b: &MpdCommand) -> bool {
        match (a, b) {
            (MpdCommand::Ping, MpdCommand::Ping) => true,
            (MpdCommand::Close, MpdCommand::Close) => true,
            (MpdCommand::Password(a), MpdCommand::Password(b)) => a == b,
            (MpdCommand::Play(a), MpdCommand::Play(b)) => a == b,
            (MpdCommand::PlayId(a), MpdCommand::PlayId(b)) => a == b,
            (MpdCommand::Pause(a), MpdCommand::Pause(b)) => a == b,
            (MpdCommand::Stop, MpdCommand::Stop) => true,
            (MpdCommand::Next, MpdCommand::Next) => true,
            (MpdCommand::Previous, MpdCommand::Previous) => true,
            (MpdCommand::Seek(a1, a2), MpdCommand::Seek(b1, b2)) => {
                a1 == b1 && a2.to_bits() == b2.to_bits()
            }
            (MpdCommand::SeekId(a1, a2), MpdCommand::SeekId(b1, b2)) => {
                a1 == b1 && a2.to_bits() == b2.to_bits()
            }
            (MpdCommand::SeekCur(a), MpdCommand::SeekCur(b)) => a.to_bits() == b.to_bits(),
            (MpdCommand::SetVol(a), MpdCommand::SetVol(b)) => a == b,
            (MpdCommand::Volume(a), MpdCommand::Volume(b)) => a == b,
            (MpdCommand::Random(a), MpdCommand::Random(b)) => a == b,
            (MpdCommand::Repeat(a), MpdCommand::Repeat(b)) => a == b,
            (MpdCommand::Single(a), MpdCommand::Single(b)) => {
                matches!(
                    (a, b),
                    (SingleMode::Off, SingleMode::Off)
                        | (SingleMode::On, SingleMode::On)
                        | (SingleMode::OneShot, SingleMode::OneShot)
                )
            }
            (MpdCommand::Consume(a), MpdCommand::Consume(b)) => a == b,
            (MpdCommand::Status, MpdCommand::Status) => true,
            (MpdCommand::Stats, MpdCommand::Stats) => true,
            (MpdCommand::CurrentSong, MpdCommand::CurrentSong) => true,
            (MpdCommand::PlaylistInfo(a), MpdCommand::PlaylistInfo(b)) => a == b,
            (MpdCommand::PlaylistId(a), MpdCommand::PlaylistId(b)) => a == b,
            (MpdCommand::Add(a), MpdCommand::Add(b)) => a == b,
            (MpdCommand::AddId(a1, a2), MpdCommand::AddId(b1, b2)) => a1 == b1 && a2 == b2,
            (MpdCommand::Delete(a), MpdCommand::Delete(b)) => a == b,
            (MpdCommand::DeleteId(a), MpdCommand::DeleteId(b)) => a == b,
            (MpdCommand::Clear, MpdCommand::Clear) => true,
            (MpdCommand::Shuffle, MpdCommand::Shuffle) => true,
            (MpdCommand::Move(a1, a2), MpdCommand::Move(b1, b2)) => a1 == b1 && a2 == b2,
            (MpdCommand::Swap(a1, a2), MpdCommand::Swap(b1, b2)) => a1 == b1 && a2 == b2,
            (MpdCommand::ListAll(a), MpdCommand::ListAll(b)) => a == b,
            (MpdCommand::LsInfo(a), MpdCommand::LsInfo(b)) => a == b,
            (MpdCommand::Find(a), MpdCommand::Find(b)) => filter_exprs_eq(a, b),
            (MpdCommand::Search(a), MpdCommand::Search(b)) => filter_exprs_eq(a, b),
            (MpdCommand::List(a1, a2), MpdCommand::List(b1, b2)) => {
                a1 == b1 && filter_exprs_eq(a2, b2)
            }
            (MpdCommand::Count(a), MpdCommand::Count(b)) => filter_exprs_eq(a, b),
            (MpdCommand::Update(a), MpdCommand::Update(b)) => a == b,
            (MpdCommand::Outputs, MpdCommand::Outputs) => true,
            (MpdCommand::EnableOutput(a), MpdCommand::EnableOutput(b)) => a == b,
            (MpdCommand::DisableOutput(a), MpdCommand::DisableOutput(b)) => a == b,
            (MpdCommand::ToggleOutput(a), MpdCommand::ToggleOutput(b)) => a == b,
            (MpdCommand::Commands, MpdCommand::Commands) => true,
            (MpdCommand::NotCommands, MpdCommand::NotCommands) => true,
            (MpdCommand::TagTypes, MpdCommand::TagTypes) => true,
            (MpdCommand::UrlHandlers, MpdCommand::UrlHandlers) => true,
            (MpdCommand::Decoders, MpdCommand::Decoders) => true,
            (MpdCommand::CommandListBegin, MpdCommand::CommandListBegin) => true,
            (MpdCommand::CommandListOkBegin, MpdCommand::CommandListOkBegin) => true,
            (MpdCommand::CommandListEnd, MpdCommand::CommandListEnd) => true,
            (MpdCommand::Idle(a), MpdCommand::Idle(b)) => a == b,
            (MpdCommand::NoIdle, MpdCommand::NoIdle) => true,
            _ => false,
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

        /// INVARIANT: simple whitespace-separated tokens round-trip through the
        /// command tokenizer.
        #[test]
        fn command_tokenizer_roundtrip(tokens in prop::collection::vec(simple_token_strategy(), 0..8)) {
            let line = tokens.join(" ");
            let mut tokenizer = CommandTokenizer::new(&line);
            let mut out = Vec::new();
            while let Some(t) = tokenizer.next_token() {
                out.push(t);
            }
            prop_assert_eq!(out, tokens);
        }

        /// INVARIANT: the tokenizer never panics on arbitrary UTF-8 input.
        #[test]
        fn random_text_tokenizer_never_panics(input in prop::collection::vec(0u8..255, 0..64)) {
            let line = String::from_utf8_lossy(&input).to_string();
            let line_for_closure = line.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                let mut tokenizer = CommandTokenizer::new(&line_for_closure);
                while tokenizer.next_token().is_some() {}
            }));
            prop_assert!(result.is_ok(), "tokenizer panicked on input: {:?}", line);
        }

        /// INVARIANT: a syntactically valid command line parses to the expected
        /// `MpdCommand` and re-parses to an equal value.
        #[test]
        fn valid_command_roundtrip(cmd in mpd_command_strategy()) {
            let line = format_command(&cmd);
            match parse_command(&line) {
                Ok(parsed) => prop_assert!(
                    commands_eq(&cmd, &parsed),
                    "round-trip failed for line {:?}: expected {:?}, got {:?}",
                    line,
                    cmd,
                    parsed
                ),
                Err(e) => prop_assert!(false, "parse failed for {:?}: {:?}", line, e),
            }
        }

        /// INVARIANT: quoted MPD arguments round-trip through the tokenizer's escape
        /// rules. Values containing backslashes, double quotes, spaces, and multibyte
        /// UTF-8 must survive `format_arg` -> `parse_command("add ...")`.
        #[test]
        fn quoted_argument_roundtrips_with_escaping(
            value in prop::collection::vec(any::<char>(), 0..64)
                .prop_map(|chars| chars.into_iter().collect::<String>()),
        ) {
            fn format_quoted_arg(value: &str) -> String {
                let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
                format!("\"{}\"", escaped)
            }

            let line = format!("add {}", format_quoted_arg(&value));
            match parse_command(&line) {
                Ok(MpdCommand::Add(uri)) => prop_assert_eq!(
                    uri, value,
                    "quoted argument did not round-trip: line={:?}",
                    line
                ),
                Ok(other) => prop_assert!(false, "unexpected command for {:?}: {:?}", line, other),
                Err(e) => prop_assert!(false, "parse failed for {:?}: {:?}", line, e),
            }
        }

    }
}

// ============================================================================
// MPD Protocol - Parser and Types
// ============================================================================
//
// The MPD protocol is line-oriented text over TCP.
// Commands: "command arg1 arg2\n"
// Responses: key-value lines ending with "OK\n" or "ACK [error@pos] {command} message\n"
//
// Reference: https://mpd.readthedocs.io/en/latest/protocol.html

/// MPD protocol version we advertise.
pub const MPD_VERSION: &str = "0.23.5";

/// ACK error codes per MPD spec.
#[derive(Debug, Clone, Copy)]
pub enum MpdErrorCode {
    NotList = 1,
    Arg = 2,
    Password = 3,
    Permission = 4,
    UnknownCmd = 5,
    NoExist = 50,
    PlaylistMax = 51,
    System = 52,
    PlaylistLoad = 53,
    UpdateAlready = 54,
    PlayerSync = 55,
    Exist = 56,
}

/// An MPD error response.
#[derive(Debug, Clone)]
pub struct MpdError {
    pub code: MpdErrorCode,
    pub command_index: usize,
    pub command: String,
    pub message: String,
}

impl MpdError {
    pub fn new(code: MpdErrorCode, command: &str, message: &str) -> Self {
        Self {
            code,
            command_index: 0,
            command: command.to_string(),
            message: message.to_string(),
        }
    }

    pub fn unknown_command(cmd: &str) -> Self {
        Self::new(
            MpdErrorCode::UnknownCmd,
            cmd,
            &format!("unknown command \"{}\"", cmd),
        )
    }

    pub fn format(&self) -> String {
        format!(
            "ACK [{}@{}] {{{}}} {}\n",
            self.code as u32, self.command_index, self.command, self.message
        )
    }
}

/// A single key-value pair in an MPD response.
#[derive(Debug, Clone)]
pub struct MpdKv {
    pub key: String,
    pub value: String,
}

/// MPD response — either success with key-value lines or an error.
#[derive(Debug, Clone)]
pub enum MpdResponse {
    Ok(Vec<MpdKv>),
    Error(MpdError),
    /// For list_ok_begin mode: OK between commands.
    ListOk,
}

impl MpdResponse {
    pub fn ok() -> Self {
        MpdResponse::Ok(vec![])
    }

    pub fn ok_with(kvs: Vec<MpdKv>) -> Self {
        MpdResponse::Ok(kvs)
    }

    pub fn format(&self) -> String {
        match self {
            MpdResponse::Ok(kvs) => {
                let mut out = String::new();
                for kv in kvs {
                    out.push_str(&kv.key);
                    out.push_str(": ");
                    out.push_str(&kv.value);
                    out.push('\n');
                }
                out.push_str("OK\n");
                out
            }
            MpdResponse::Error(err) => err.format(),
            MpdResponse::ListOk => "list_OK\n".to_string(),
        }
    }
}

/// Helper to build kv pairs.
pub fn kv(key: &str, value: impl std::fmt::Display) -> MpdKv {
    MpdKv {
        key: key.to_string(),
        value: value.to_string(),
    }
}

/// Parsed MPD commands.
#[derive(Debug, Clone)]
pub enum MpdCommand {
    // Connection / session
    Ping,
    Close,
    Password(String),

    // Playback control
    Play(Option<u32>),   // position in playlist (optional)
    PlayId(Option<u32>), // song id (optional)
    Pause(Option<bool>), // toggle or explicit
    Stop,
    Next,
    Previous,
    Seek(u32, f64),   // songpos, time_secs
    SeekId(u32, f64), // songid, time_secs
    SeekCur(f64),     // relative or absolute seconds

    // Playback options
    SetVol(u8), // 0-100
    Volume(i8), // relative change
    Random(bool),
    Repeat(bool),
    Single(SingleMode),
    Consume(bool),

    // Status
    Status,
    Stats,
    CurrentSong,

    // Queue / playlist
    PlaylistInfo(Option<(u32, Option<u32>)>), // optional range
    PlaylistId(Option<u32>),
    Add(String),                // URI
    AddId(String, Option<u32>), // URI, optional position
    Delete(u32),                // position
    DeleteId(u32),              // song id
    Clear,
    Shuffle,
    Move(u32, u32), // from, to
    Swap(u32, u32), // pos1, pos2

    // Database / library
    ListAll(Option<String>), // optional path
    LsInfo(Option<String>),  // optional path
    Find(Vec<FilterExpr>),
    Search(Vec<FilterExpr>),
    List(String, Vec<FilterExpr>), // tag, optional filter
    Count(Vec<FilterExpr>),
    Update(Option<String>), // optional path

    // Output
    Outputs,
    EnableOutput(u32),
    DisableOutput(u32),
    ToggleOutput(u32),

    // Reflection
    Commands,
    NotCommands,
    TagTypes,
    UrlHandlers,
    Decoders,

    // Command lists
    CommandListBegin,
    CommandListOkBegin,
    CommandListEnd,

    // Idle / notify
    Idle(Vec<String>), // optional subsystems
    NoIdle,
}

#[derive(Debug, Clone)]
pub enum SingleMode {
    Off,
    On,
    OneShot,
}

#[derive(Debug, Clone)]
pub struct FilterExpr {
    pub tag: String,
    pub value: String,
}

/// Parse a single MPD command line (without the trailing newline).
pub fn parse_command(line: &str) -> Result<MpdCommand, MpdError> {
    let line = line.trim();
    if line.is_empty() {
        return Err(MpdError::new(MpdErrorCode::UnknownCmd, "", "empty command"));
    }

    let mut parts = CommandTokenizer::new(line);
    let cmd = parts.next_token().unwrap_or_default().to_lowercase();

    match cmd.as_str() {
        // Connection
        "ping" => Ok(MpdCommand::Ping),
        "close" => Ok(MpdCommand::Close),
        "password" => Ok(MpdCommand::Password(parts.next_token().unwrap_or_default())),

        // Playback control
        "play" => Ok(MpdCommand::Play(parts.next_u32())),
        "playid" => Ok(MpdCommand::PlayId(parts.next_u32())),
        "pause" => Ok(MpdCommand::Pause(parts.next_bool())),
        "stop" => Ok(MpdCommand::Stop),
        "next" => Ok(MpdCommand::Next),
        "previous" => Ok(MpdCommand::Previous),
        "seek" => {
            let pos = parts.require_u32(&cmd)?;
            let time = parts.require_f64(&cmd)?;
            Ok(MpdCommand::Seek(pos, time))
        }
        "seekid" => {
            let id = parts.require_u32(&cmd)?;
            let time = parts.require_f64(&cmd)?;
            Ok(MpdCommand::SeekId(id, time))
        }
        "seekcur" => {
            let time = parts.require_f64(&cmd)?;
            Ok(MpdCommand::SeekCur(time))
        }

        // Playback options
        "setvol" => {
            let vol = parts.require_u32(&cmd)?;
            if vol > 100 {
                return Err(MpdError::new(
                    MpdErrorCode::Arg,
                    &cmd,
                    "volume must be 0-100",
                ));
            }
            Ok(MpdCommand::SetVol(vol as u8))
        }
        "volume" => {
            // MPD spec: delta is a signed integer; route through i32 explicitly
            // and reject anything outside `-100..=100`. The previous `as i8`
            // wrapped silently (e.g. 200 → -56, 1000 → -24).
            let delta_i32 = parts.require_i32(&cmd)?;
            if !(-100..=100).contains(&delta_i32) {
                return Err(MpdError::new(
                    MpdErrorCode::Arg,
                    &cmd,
                    "volume delta must be -100..=100",
                ));
            }
            // Safe: the range check guarantees the value fits in i8.
            Ok(MpdCommand::Volume(delta_i32 as i8))
        }
        "random" => Ok(MpdCommand::Random(parts.require_bool(&cmd)?)),
        "repeat" => Ok(MpdCommand::Repeat(parts.require_bool(&cmd)?)),
        "single" => {
            // Per project rule: do not silently coerce unknown values to a
            // default. Reject anything outside {`0`, `1`, `oneshot`} with
            // ACK_ERROR_ARG so a misbehaving client gets a clear signal.
            let val = parts.require_string(&cmd)?;
            let mode = match val.as_str() {
                "0" => SingleMode::Off,
                "1" => SingleMode::On,
                "oneshot" => SingleMode::OneShot,
                other => {
                    return Err(MpdError::new(
                        MpdErrorCode::Arg,
                        &cmd,
                        &format!("expected 0/1/oneshot, got \"{other}\""),
                    ));
                }
            };
            Ok(MpdCommand::Single(mode))
        }
        "consume" => Ok(MpdCommand::Consume(parts.require_bool(&cmd)?)),

        // Status
        "status" => Ok(MpdCommand::Status),
        "stats" => Ok(MpdCommand::Stats),
        "currentsong" => Ok(MpdCommand::CurrentSong),

        // Queue
        "playlistinfo" => Ok(MpdCommand::PlaylistInfo(parse_range(&mut parts))),
        "playlistid" => Ok(MpdCommand::PlaylistId(parts.next_u32())),
        "add" => Ok(MpdCommand::Add(parts.require_string(&cmd)?)),
        "addid" => {
            let uri = parts.require_string(&cmd)?;
            let pos = parts.next_u32();
            Ok(MpdCommand::AddId(uri, pos))
        }
        "delete" => Ok(MpdCommand::Delete(parts.require_u32(&cmd)?)),
        "deleteid" => Ok(MpdCommand::DeleteId(parts.require_u32(&cmd)?)),
        "clear" => Ok(MpdCommand::Clear),
        "shuffle" => Ok(MpdCommand::Shuffle),
        "move" => {
            let from = parts.require_u32(&cmd)?;
            let to = parts.require_u32(&cmd)?;
            Ok(MpdCommand::Move(from, to))
        }
        "swap" => {
            let p1 = parts.require_u32(&cmd)?;
            let p2 = parts.require_u32(&cmd)?;
            Ok(MpdCommand::Swap(p1, p2))
        }

        // Database
        "listall" => Ok(MpdCommand::ListAll(parts.next_token())),
        "lsinfo" => Ok(MpdCommand::LsInfo(parts.next_token())),
        "find" => Ok(MpdCommand::Find(parse_filter_exprs(&mut parts))),
        "search" => Ok(MpdCommand::Search(parse_filter_exprs(&mut parts))),
        "list" => {
            let tag = parts.require_string(&cmd)?;
            let filters = parse_filter_exprs(&mut parts);
            Ok(MpdCommand::List(tag, filters))
        }
        "count" => Ok(MpdCommand::Count(parse_filter_exprs(&mut parts))),
        "update" => Ok(MpdCommand::Update(parts.next_token())),

        // Output
        "outputs" => Ok(MpdCommand::Outputs),
        "enableoutput" => Ok(MpdCommand::EnableOutput(parts.require_u32(&cmd)?)),
        "disableoutput" => Ok(MpdCommand::DisableOutput(parts.require_u32(&cmd)?)),
        "toggleoutput" => Ok(MpdCommand::ToggleOutput(parts.require_u32(&cmd)?)),

        // Reflection
        "commands" => Ok(MpdCommand::Commands),
        "notcommands" => Ok(MpdCommand::NotCommands),
        "tagtypes" => Ok(MpdCommand::TagTypes),
        "urlhandlers" => Ok(MpdCommand::UrlHandlers),
        "decoders" => Ok(MpdCommand::Decoders),

        // Command lists
        "command_list_begin" => Ok(MpdCommand::CommandListBegin),
        "command_list_ok_begin" => Ok(MpdCommand::CommandListOkBegin),
        "command_list_end" => Ok(MpdCommand::CommandListEnd),

        // Idle
        "idle" => {
            let mut subsystems = Vec::new();
            while let Some(s) = parts.next_token() {
                subsystems.push(s);
            }
            Ok(MpdCommand::Idle(subsystems))
        }
        "noidle" => Ok(MpdCommand::NoIdle),

        _ => Err(MpdError::unknown_command(&cmd)),
    }
}

fn parse_range(parts: &mut CommandTokenizer) -> Option<(u32, Option<u32>)> {
    let token = parts.next_token()?;
    if let Some((start, end)) = token.split_once(':') {
        let start = start.parse().ok()?;
        let end = if end.is_empty() {
            None
        } else {
            end.parse().ok()
        };
        Some((start, end))
    } else {
        let pos = token.parse().ok()?;
        Some((pos, Some(pos + 1)))
    }
}

fn parse_filter_exprs(parts: &mut CommandTokenizer) -> Vec<FilterExpr> {
    let mut exprs = Vec::new();
    while let Some(tag) = parts.next_token() {
        if let Some(value) = parts.next_token() {
            exprs.push(FilterExpr { tag, value });
        }
    }
    exprs
}

// ============================================================================
// Command tokenizer — handles quoted strings
// ============================================================================

struct CommandTokenizer<'a> {
    input: &'a str,
    pos: usize,
}

/// Errors raised by the quoted-token state machine. Surfaced through
/// `next_token_result`; `require_string` rewrites them as ACK_ERROR_ARG.
#[derive(Debug, Clone, Copy)]
enum TokenizerError {
    /// A quoted token never reached its closing quote.
    UnterminatedQuote,
    /// A quoted token's bytes did not form valid UTF-8 after escape processing.
    InvalidUtf8,
}

impl TokenizerError {
    fn message(self) -> &'static str {
        match self {
            TokenizerError::UnterminatedQuote => "unterminated quoted string",
            TokenizerError::InvalidUtf8 => "invalid UTF-8 in quoted string",
        }
    }
}

impl<'a> CommandTokenizer<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn next_token(&mut self) -> Option<String> {
        // Tokenizer-level errors are dropped at this layer; the public
        // `require_string` API uses `next_token_result` directly when it needs
        // to surface them as ACK errors.
        self.next_token_result().ok().flatten()
    }

    /// Like `next_token`, but reports quoted-string errors instead of silently
    /// returning the dangling tail. Without this, an unterminated quote or a
    /// multibyte UTF-8 corruption would leave the parser desynchronised.
    fn next_token_result(&mut self) -> Result<Option<String>, TokenizerError> {
        self.skip_whitespace();
        if self.pos >= self.input.len() {
            return Ok(None);
        }

        let bytes = self.input.as_bytes();
        if bytes[self.pos] == b'"' {
            // Quoted string. Accumulate as raw bytes and only validate UTF-8
            // once the closing quote is reached, so that multibyte sequences
            // spanning a backslash escape are preserved exactly. The previous
            // implementation pushed `bytes[i] as char` mid-escape, which cast
            // the leading byte of a multibyte UTF-8 sequence to a Latin-1
            // codepoint and corrupted the rest of the token.
            self.pos += 1; // skip opening quote
            let mut buf: Vec<u8> = Vec::new();
            loop {
                if self.pos >= bytes.len() {
                    return Err(TokenizerError::UnterminatedQuote);
                }
                let b = bytes[self.pos];
                match b {
                    b'"' => {
                        self.pos += 1; // consume closing quote
                        return String::from_utf8(buf)
                            .map(Some)
                            .map_err(|_| TokenizerError::InvalidUtf8);
                    }
                    b'\\' => {
                        // Per MPD spec, only `\\` and `\"` are required
                        // escapes; any other escaped byte is taken literally.
                        // Operating on raw bytes keeps multibyte content intact
                        // and lets the post-loop `from_utf8` validation catch
                        // genuine corruption rather than masking it.
                        if self.pos + 1 >= bytes.len() {
                            return Err(TokenizerError::UnterminatedQuote);
                        }
                        buf.push(bytes[self.pos + 1]);
                        self.pos += 2;
                    }
                    other => {
                        buf.push(other);
                        self.pos += 1;
                    }
                }
            }
        } else {
            // Unquoted token
            let start = self.pos;
            while self.pos < bytes.len() && bytes[self.pos] != b' ' && bytes[self.pos] != b'\t' {
                self.pos += 1;
            }
            Ok(Some(self.input[start..self.pos].to_string()))
        }
    }

    fn skip_whitespace(&mut self) {
        let bytes = self.input.as_bytes();
        while self.pos < bytes.len() && (bytes[self.pos] == b' ' || bytes[self.pos] == b'\t') {
            self.pos += 1;
        }
    }

    fn require_string(&mut self, cmd: &str) -> Result<String, MpdError> {
        match self.next_token_result() {
            Ok(Some(tok)) => Ok(tok),
            Ok(None) => Err(MpdError::new(
                MpdErrorCode::Arg,
                cmd,
                "missing required argument",
            )),
            Err(err) => Err(MpdError::new(MpdErrorCode::Arg, cmd, err.message())),
        }
    }

    fn require_u32(&mut self, cmd: &str) -> Result<u32, MpdError> {
        let token = self.require_string(cmd)?;
        token.parse().map_err(|_| {
            MpdError::new(
                MpdErrorCode::Arg,
                cmd,
                &format!("expected integer, got \"{}\"", token),
            )
        })
    }

    fn require_i32(&mut self, cmd: &str) -> Result<i32, MpdError> {
        let token = self.require_string(cmd)?;
        token.parse().map_err(|_| {
            MpdError::new(
                MpdErrorCode::Arg,
                cmd,
                &format!("expected integer, got \"{}\"", token),
            )
        })
    }

    fn require_f64(&mut self, cmd: &str) -> Result<f64, MpdError> {
        let token = self.require_string(cmd)?;
        token.parse().map_err(|_| {
            MpdError::new(
                MpdErrorCode::Arg,
                cmd,
                &format!("expected number, got \"{}\"", token),
            )
        })
    }

    fn require_bool(&mut self, cmd: &str) -> Result<bool, MpdError> {
        let token = self.require_string(cmd)?;
        match token.as_str() {
            "0" => Ok(false),
            "1" => Ok(true),
            _ => Err(MpdError::new(
                MpdErrorCode::Arg,
                cmd,
                &format!("expected 0 or 1, got \"{}\"", token),
            )),
        }
    }

    fn next_u32(&mut self) -> Option<u32> {
        self.next_token().and_then(|t| t.parse().ok())
    }

    fn next_bool(&mut self) -> Option<bool> {
        self.next_token().and_then(|t| match t.as_str() {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
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
}

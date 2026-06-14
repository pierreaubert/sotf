/// Authentication mode for the MPD server.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum MpdAuthMode {
    /// Mutual TLS — client cert fingerprint must be in the trusted set.
    #[default]
    Certificate,
    /// Legacy password-based authentication.
    Password,
}

/// Outcome of [`read_line_bounded`].
pub(super) enum LineRead {
    /// A complete line (without trailing CR/LF).
    Line(String),
    /// EOF before any bytes were read.
    Eof,
    /// The line exceeded `max_bytes` without a newline. Connection should be
    /// terminated after an ACK is sent.
    TooLong,
    /// Bytes that did not form valid UTF-8 once the newline was reached.
    InvalidUtf8,
}

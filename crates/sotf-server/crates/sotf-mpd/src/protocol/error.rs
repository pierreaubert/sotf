use super::types::MpdErrorCode;

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

/// Errors raised by the quoted-token state machine. Surfaced through
/// `next_token_result`; `require_string` rewrites them as ACK_ERROR_ARG.
#[derive(Debug, Clone, Copy)]
pub(super) enum TokenizerError {
    /// A quoted token never reached its closing quote.
    UnterminatedQuote,
    /// A quoted token's bytes did not form valid UTF-8 after escape processing.
    InvalidUtf8,
}

impl TokenizerError {
    pub(super) fn message(self) -> &'static str {
        match self {
            TokenizerError::UnterminatedQuote => "unterminated quoted string",
            TokenizerError::InvalidUtf8 => "invalid UTF-8 in quoted string",
        }
    }
}

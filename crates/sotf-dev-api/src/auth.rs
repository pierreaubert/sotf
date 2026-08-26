use std::fmt;

use thiserror::Error;

pub const RUN_ID_HEADER: &str = "x-sotf-dev-run-id";
pub const MIN_RUN_ID_BYTES: usize = 16;
pub const MAX_RUN_ID_BYTES: usize = 128;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RunId(String);

impl RunId {
    pub fn parse(value: impl Into<String>) -> Result<Self, RunIdError> {
        let value = value.into();
        let bytes = value.as_bytes();
        if !(MIN_RUN_ID_BYTES..=MAX_RUN_ID_BYTES).contains(&bytes.len()) {
            return Err(RunIdError::Length(bytes.len()));
        }
        if !bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(RunIdError::Characters);
        }
        Ok(Self(value))
    }

    pub fn authenticate(&self, candidate: &str) -> bool {
        constant_work_eq(self.0.as_bytes(), candidate.as_bytes())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn redacted_hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(self.0.as_bytes());
        hex::encode(&digest[..8])
    }
}

impl fmt::Debug for RunId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("RunId")
            .field(&format_args!("<redacted:{}>", self.redacted_hash()))
            .finish()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RunIdError {
    #[error("run ID length {0} is outside {MIN_RUN_ID_BYTES}..={MAX_RUN_ID_BYTES}")]
    Length(usize),
    #[error("run ID contains unsupported characters")]
    Characters,
}

fn constant_work_eq(expected: &[u8], candidate: &[u8]) -> bool {
    let compared_len = expected.len().max(candidate.len());
    let mut difference = expected.len() ^ candidate.len();
    for index in 0..compared_len {
        let left = expected.get(index).copied().unwrap_or_default();
        let right = candidate.get(index).copied().unwrap_or_default();
        difference |= usize::from(left ^ right);
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_and_authenticates_run_ids() {
        let id = RunId::parse("0123456789abcdef0123456789abcdef").unwrap();
        assert!(id.authenticate("0123456789abcdef0123456789abcdef"));
        assert!(!id.authenticate("0123456789abcdef0123456789abcdee"));
        assert!(!id.authenticate("short"));
        assert!(!format!("{id:?}").contains(id.as_str()));
    }

    #[test]
    fn rejects_unsafe_run_ids() {
        assert_eq!(RunId::parse("short").unwrap_err(), RunIdError::Length(5));
        assert_eq!(
            RunId::parse("0123456789abcde/").unwrap_err(),
            RunIdError::Characters
        );
    }
}

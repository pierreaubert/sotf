/// Stack-buffered formatter for the short numeric labels rendered next
/// to every meter / LUFS bar. Reusing the same buffer across renders
/// avoids the `format!`-per-channel-per-frame allocation that appeared
/// in flamegraphs of multi-channel meter screens.
pub(crate) struct MeterLabelBuf {
    pub(super) buf: [u8; 32],
    pub(super) len: usize,
}

impl MeterLabelBuf {
    pub(crate) fn new() -> Self {
        Self {
            buf: [0; 32],
            len: 0,
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        // Only ASCII / valid-UTF-8 sequences are ever written via the
        // `write!` macro below, and `len` tracks bytes written.
        std::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }
}

impl std::fmt::Write for MeterLabelBuf {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let remaining = self.buf.len() - self.len;
        let bytes = s.as_bytes();
        if bytes.len() > remaining {
            return Err(std::fmt::Error);
        }
        self.buf[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(())
    }
}

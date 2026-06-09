// ============================================================================
// MSB-first bit reader over a byte slice
// ============================================================================
//
// IAMF bit fields are packed MSB-first within each byte (see IAMF v1.1.0 §3).
// `BitReader` tracks a bit offset into a borrowed `&[u8]` and exposes
// fixed-width reads + byte-aligned skip.
//
// Each read returns `IamfError::ParseError` on truncation. The struct is
// intentionally small (a slice + a bit cursor) so it can be created and
// dropped freely inside parse helpers.

use crate::error::{IamfError, IamfResult};

pub struct BitReader<'a> {
    data: &'a [u8],
    /// Absolute bit position (0..=data.len()*8).
    bit_pos: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, bit_pos: 0 }
    }

    /// Read `n` bits (1..=32) MSB-first as a u32.
    pub fn read_bits(&mut self, n: u32) -> IamfResult<u32> {
        if n == 0 {
            return Ok(0);
        }
        if n > 32 {
            return Err(IamfError::ParseError(format!(
                "BitReader::read_bits n={n} > 32"
            )));
        }
        let end = self.bit_pos + n as usize;
        if end > self.data.len() * 8 {
            return Err(IamfError::ParseError(
                "BitReader: unexpected end of data".into(),
            ));
        }

        let mut value: u32 = 0;
        let mut remaining = n as usize;
        while remaining > 0 {
            let byte_idx = self.bit_pos / 8;
            let bit_in_byte = self.bit_pos % 8;
            let avail = 8 - bit_in_byte;
            let take = avail.min(remaining);
            // Extract `take` bits starting at `bit_in_byte` (MSB-first).
            let shift_right = avail - take;
            let mask = (1u32 << take) - 1;
            let chunk = ((self.data[byte_idx] as u32) >> shift_right) & mask;
            value = (value << take) | chunk;
            self.bit_pos += take;
            remaining -= take;
        }
        Ok(value)
    }

    /// Read a single bit as a bool.
    pub fn read_bool(&mut self) -> IamfResult<bool> {
        Ok(self.read_bits(1)? != 0)
    }

    /// Skip `n` bits.
    pub fn skip_bits(&mut self, n: u32) -> IamfResult<()> {
        let end = self.bit_pos + n as usize;
        if end > self.data.len() * 8 {
            return Err(IamfError::ParseError(
                "BitReader: skip past end of data".into(),
            ));
        }
        self.bit_pos = end;
        Ok(())
    }

    /// Align the cursor up to the next byte boundary.
    pub fn align_to_byte(&mut self) {
        let rem = self.bit_pos % 8;
        if rem != 0 {
            self.bit_pos += 8 - rem;
        }
    }

    /// Byte offset of the cursor, rounded up. After `align_to_byte()` this is
    /// the exact byte offset just past the bits that have been consumed.
    pub fn byte_pos(&self) -> usize {
        self.bit_pos.div_ceil(8)
    }

    /// Whether the cursor is currently byte-aligned.
    pub fn is_byte_aligned(&self) -> bool {
        self.bit_pos.is_multiple_of(8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_bits_basic() {
        // 0b10110010 0b11110000
        let data = [0b1011_0010, 0b1111_0000];
        let mut br = BitReader::new(&data);
        assert_eq!(br.read_bits(3).unwrap(), 0b101);
        assert_eq!(br.read_bits(5).unwrap(), 0b10010);
        assert_eq!(br.read_bits(4).unwrap(), 0b1111);
        assert_eq!(br.read_bits(4).unwrap(), 0b0000);
    }

    #[test]
    fn read_bool_and_skip() {
        // bits: 1 0 0 1 0 0 0 0  (MSB-first)
        // read 1 → true, skip 2 (consume bits 1,2), next bool reads bit 3 = 1.
        let data = [0b1001_0000];
        let mut br = BitReader::new(&data);
        assert!(br.read_bool().unwrap());
        br.skip_bits(2).unwrap();
        assert!(br.read_bool().unwrap());
    }

    #[test]
    fn read_past_end_errors() {
        let data = [0xff];
        let mut br = BitReader::new(&data);
        assert!(br.read_bits(8).is_ok());
        assert!(br.read_bits(1).is_err());
    }

    #[test]
    fn align_to_byte_rounds_up() {
        let data = [0xff, 0x00];
        let mut br = BitReader::new(&data);
        br.read_bits(3).unwrap();
        br.align_to_byte();
        assert!(br.is_byte_aligned());
        assert_eq!(br.byte_pos(), 1);
    }

    #[test]
    fn cross_byte_read() {
        // Top 4 bits of byte 0 = 0b1011, next 8 bits should span bytes.
        let data = [0b1011_0010, 0b1100_0000];
        let mut br = BitReader::new(&data);
        br.skip_bits(4).unwrap();
        // bits 4..12 = 0b0010_1100 = 0x2C
        assert_eq!(br.read_bits(8).unwrap(), 0x2C);
    }

    #[test]
    fn read_bits_zero_returns_zero() {
        let data = [0xFF];
        let mut br = BitReader::new(&data);
        assert_eq!(br.read_bits(0).unwrap(), 0);
        // Cursor should not advance.
        assert_eq!(br.byte_pos(), 0);
    }

    #[test]
    fn read_bits_too_many_errors() {
        let data = [0xFF];
        let mut br = BitReader::new(&data);
        assert!(br.read_bits(33).is_err());
    }

    #[test]
    fn skip_bits_zero_ok() {
        let data = [0xFF];
        let mut br = BitReader::new(&data);
        assert!(br.skip_bits(0).is_ok());
        assert_eq!(br.byte_pos(), 0);
    }

    #[test]
    fn skip_past_end_errors() {
        let data = [0xFF];
        let mut br = BitReader::new(&data);
        assert!(br.skip_bits(9).is_err());
    }

    #[test]
    fn empty_reader_byte_pos_is_zero() {
        let br = BitReader::new(&[]);
        assert_eq!(br.byte_pos(), 0);
        assert!(br.is_byte_aligned());
    }

    #[test]
    fn read_bool_false() {
        // bits: 1 0 1 1 ...
        let data = [0b1011_0000];
        let mut br = BitReader::new(&data);
        assert!(br.read_bool().unwrap());
        assert!(!br.read_bool().unwrap());
    }

    #[test]
    fn byte_pos_exact_after_aligned_reads() {
        let data = [0xFF, 0xFF];
        let mut br = BitReader::new(&data);
        br.read_bits(8).unwrap();
        assert!(br.is_byte_aligned());
        assert_eq!(br.byte_pos(), 1);
        br.read_bits(8).unwrap();
        assert_eq!(br.byte_pos(), 2);
    }

    #[test]
    fn read_all_32_bits() {
        let data = [0x12, 0x34, 0x56, 0x78];
        let mut br = BitReader::new(&data);
        assert_eq!(br.read_bits(32).unwrap(), 0x12345678);
    }
}

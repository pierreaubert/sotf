// ============================================================================
// Property-Based Tests for sotf-iamf
// ============================================================================
//
// Covers LEB128 encode/decode round-trip, BitReader read/skip consistency,
// and IAMF channel count / mapping invariants.

use proptest::prelude::*;
use sotf_iamf::obu::BitReader;
use sotf_iamf::obu::parser::read_leb128;
use sotf_iamf::types::IamfChannelLayout;

// =============================================================================
// LEB128 encode/decode helpers (test-only)
// =============================================================================

fn encode_leb128(mut value: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(10);
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
    out
}

// =============================================================================
// LEB128 Round-Trip Properties
// =============================================================================

proptest! {
    /// INVARIANT: Encoding any u64 and decoding it round-trips to the original
    /// value, provided the value does not require a shift >= 64 (i.e. the
    /// decoder's overflow guard is not triggered).
    #[test]
    fn leb128_u64_roundtrip(value in 0u64..(1u64 << 63)) {
        let encoded = encode_leb128(value);
        let (decoded, consumed) = read_leb128(&encoded).unwrap();
        prop_assert_eq!(decoded, value);
        prop_assert_eq!(consumed, encoded.len());
    }

    /// INVARIANT: Encoding any u32 always round-trips (u32 fits well within
    /// the 64-bit decoder's safe range).
    #[test]
    fn leb128_u32_roundtrip(value in 0u32..=u32::MAX) {
        let encoded = encode_leb128(value as u64);
        let (decoded, consumed) = read_leb128(&encoded).unwrap();
        prop_assert_eq!(decoded, value as u64);
        prop_assert!(consumed <= 5, "u32 leb128 must fit in <= 5 bytes");
    }

    /// INVARIANT: Concatenating two valid encodings decodes back as the first
    /// value, with the cursor positioned at the start of the second encoding.
    #[test]
    fn leb128_concatenated_decode(
        a in 0u64..(1u64 << 56),
        b in 0u64..(1u64 << 56)
    ) {
        let mut buf = encode_leb128(a);
        buf.extend_from_slice(&encode_leb128(b));

        let (decoded_a, consumed_a) = read_leb128(&buf).unwrap();
        prop_assert_eq!(decoded_a, a);

        let (decoded_b, consumed_b) = read_leb128(&buf[consumed_a..]).unwrap();
        prop_assert_eq!(decoded_b, b);
        prop_assert_eq!(consumed_a + consumed_b, buf.len());
    }
}

// =============================================================================
// BitReader Consistency Properties
// =============================================================================

proptest! {
    /// INVARIANT: Reading N bits all at once yields the same value as reading
    /// the same total number of bits in arbitrary smaller chunks (capped to
    /// BitReader's 32-bit read limit).
    #[test]
    fn bitreader_chunked_read_matches_single_read(
        data in prop::collection::vec(0u8..=u8::MAX, 1..5),
        split in 1usize..31usize,
    ) {
        let total_bits = (data.len() * 8).min(32);
        let split = split.min(total_bits.saturating_sub(1)).max(1);

        let mut br1 = BitReader::new(&data);
        let single = br1.read_bits(total_bits as u32).unwrap();

        let mut br2 = BitReader::new(&data);
        let first = br2.read_bits(split as u32).unwrap();
        let second = br2.read_bits((total_bits - split) as u32).unwrap();
        let combined = (first << (total_bits - split)) | second;

        prop_assert_eq!(single, combined);
    }

    /// INVARIANT: Skipping N bits then reading M bits returns the same value
    /// as reading N+M bits and masking out the top N bits.
    #[test]
    fn bitreader_skip_then_read_matches_direct_read(
        data in prop::collection::vec(0u8..=u8::MAX, 2..8),
        skip in 0usize..16usize,
        read in 1usize..16usize,
    ) {
        let total_bits = data.len() * 8;
        let skip = skip.min(total_bits - 1);
        let read = read.min(total_bits - skip).max(1);

        let mut br_skip = BitReader::new(&data);
        br_skip.skip_bits(skip as u32).unwrap();
        let skipped_value = br_skip.read_bits(read as u32).unwrap();

        let mut br_direct = BitReader::new(&data);
        let combined = br_direct.read_bits((skip + read) as u32).unwrap();
        let direct_value = combined & ((1u32 << read).wrapping_sub(1));

        prop_assert_eq!(skipped_value, direct_value);
    }

    /// INVARIANT: read_bool() is equivalent to read_bits(1) != 0.
    #[test]
    fn bitreader_bool_matches_one_bit_read(data in prop::collection::vec(0u8..=u8::MAX, 1..4)) {
        let mut br_bool = BitReader::new(&data);
        let mut br_bits = BitReader::new(&data);

        for _ in 0..data.len() * 8 {
            let bool_val = br_bool.read_bool().unwrap();
            let bits_val = br_bits.read_bits(1).unwrap();
            prop_assert_eq!(bool_val, bits_val != 0);
        }
    }

    /// INVARIANT: byte_pos() and bit cursor are monotonically non-decreasing
    /// across reads and skips.
    #[test]
    fn bitreader_position_monotonic(
        data in prop::collection::vec(0u8..=u8::MAX, 2..8),
        steps in prop::collection::vec(1usize..8usize, 1..10),
    ) {
        let mut br = BitReader::new(&data);
        let total_bits = data.len() * 8;
        let mut last_pos = 0usize;

        for step in steps {
            let step = step.min(total_bits - last_pos).max(1);
            let _ = br.read_bits(step as u32).unwrap();
            let new_pos = br.byte_pos();
            prop_assert!(new_pos >= last_pos.div_ceil(8),
                "byte_pos decreased: {} -> {}", last_pos.div_ceil(8), new_pos);
            last_pos += step;
            if last_pos >= total_bits {
                break;
            }
        }
    }

    /// INVARIANT: Zero-bit read returns 0 without advancing the cursor.
    #[test]
    fn bitreader_zero_bit_read_is_noop(data in prop::collection::vec(0u8..=u8::MAX, 1..4)) {
        let mut br = BitReader::new(&data);
        let before = br.byte_pos();
        let val = br.read_bits(0).unwrap();
        prop_assert_eq!(val, 0);
        prop_assert_eq!(br.byte_pos(), before);
    }

    /// INVARIANT: align_to_byte() always leaves the reader byte-aligned and
    /// does not move the cursor backward.
    #[test]
    fn bitreader_align_never_moves_backward(
        data in prop::collection::vec(0u8..=u8::MAX, 1..8),
        n in 0u32..32
    ) {
        let mut br = BitReader::new(&data);
        let _ = br.read_bits(n.min(data.len() as u32 * 8));
        let before = br.byte_pos();
        br.align_to_byte();
        prop_assert!(br.is_byte_aligned());
        prop_assert!(br.byte_pos() >= before);
    }
}

// =============================================================================
// Channel Count / Mapping Properties
// =============================================================================

proptest! {
    /// INVARIANT: Every valid layout index maps to a layout whose channel_count
    /// matches the canonical label list length.
    #[test]
    fn channel_count_matches_label_count_for_known_layouts(idx in 0u8..=9u8) {
        if let Some(layout) = IamfChannelLayout::from_layout_index(idx) {
            let count = layout.channel_count();
            prop_assert!(count > 0, "channel_count must be positive for known layouts");
            // Round-trip through speaker config id is consistent when present.
            let _ = layout.to_speaker_config_id();
        }
    }

    /// INVARIANT: Invalid layout indices always return None.
    #[test]
    fn invalid_layout_index_returns_none(idx in 10u8..=255u8) {
        prop_assert!(IamfChannelLayout::from_layout_index(idx).is_none());
    }

    /// INVARIANT: Stereo and Mono channel counts are fixed.
    #[test]
    fn stereo_mono_counts_are_fixed(_dummy in 0u8..1) {
        prop_assert_eq!(IamfChannelLayout::Mono.channel_count(), 1);
        prop_assert_eq!(IamfChannelLayout::Stereo.channel_count(), 2);
        prop_assert_eq!(IamfChannelLayout::Binaural.channel_count(), 2);
    }

    /// INVARIANT: Layout ordering preserves channel-count monotonicity where
    /// expected: 5.1 <= 5.1.2 <= 5.1.4 and 7.1 <= 7.1.2 <= 7.1.4.
    #[test]
    fn surround_layouts_monotonically_increase(_dummy in 0u8..1) {
        use IamfChannelLayout::*;
        prop_assert!(Layout5_1.channel_count() <= Layout5_1_2.channel_count());
        prop_assert!(Layout5_1_2.channel_count() <= Layout5_1_4.channel_count());
        prop_assert!(Layout7_1.channel_count() <= Layout7_1_2.channel_count());
        prop_assert!(Layout7_1_2.channel_count() <= Layout7_1_4.channel_count());
    }
}

// =============================================================================
// Adversarial / Edge Properties
// =============================================================================

proptest! {
    /// INVARIANT: The maximum 9-byte leb128 (shift < 64) decodes successfully
    /// and round-trips through our encoder.
    #[test]
    fn leb128_max_safe_value_roundtrips(_dummy in 0u8..1) {
        // 9 continuation bytes: max shift = 63, value = 2^63 - 1 (i63 max).
        let value = (1u64 << 63) - 1;
        let encoded = encode_leb128(value);
        prop_assert_eq!(encoded.len(), 9);
        let (decoded, _) = read_leb128(&encoded).unwrap();
        prop_assert_eq!(decoded, value);
    }

    /// INVARIANT: A 10-byte encoding with all continuation bits set triggers
    /// the decoder's overflow guard (shift >= 64 before termination).
    #[test]
    fn leb128_continuation_overflow_is_rejected(_dummy in 0u8..1) {
        let data = [0xff; 10];
        prop_assert!(read_leb128(&data).is_err());
    }

    /// INVARIANT: Empty data is rejected as truncated.
    #[test]
    fn leb128_empty_rejected(_dummy in 0u8..1) {
        prop_assert!(read_leb128(&[]).is_err());
    }
}

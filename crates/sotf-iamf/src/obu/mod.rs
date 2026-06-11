// ============================================================================
// OBU (Open Bitstream Unit) Module
// ============================================================================

pub mod bitreader;
pub mod parser;

pub use bitreader::BitReader;
pub use parser::{ObuHeader, ObuType, parse_descriptors, parse_temporal_unit};

#[cfg(test)]
mod tests {
    use super::parser::{parse_obu_header, parse_sequence_header, read_leb128};

    #[test]
    fn read_leb128_max_u64_edge() {
        // leb128 encoding of u64::MAX requires 10 bytes, but shift >= 64 triggers
        // overflow before the value is complete. Use 10 continuation bytes.
        let data = [0xff; 10];
        assert!(read_leb128(&data).is_err());
    }

    #[test]
    fn read_leb128_truncated() {
        // All high bits set, no terminating byte.
        let data = [0xff, 0xff, 0xff];
        assert!(read_leb128(&data).is_err());
    }

    #[test]
    fn read_leb128_large_value() {
        // 0x80 = 128 -> leb128 [0x80, 0x01]
        let (val, consumed) = read_leb128(&[0x80, 0x01]).unwrap();
        assert_eq!(val, 128);
        assert_eq!(consumed, 2);
    }

    #[test]
    fn parse_obu_header_empty_errors() {
        assert!(parse_obu_header(&[]).is_err());
    }

    #[test]
    fn parse_obu_header_truncated_payload() {
        // OBU type=0, payload_size=10, but only 2 bytes of payload.
        let data = [0x00, 10, 0x00, 0x00];
        assert!(parse_obu_header(&data).is_err());
    }

    #[test]
    fn parse_obu_header_with_trimming() {
        // OBU type=5 (AudioFrame), trimming_status=1, extension_flag=0
        // Type byte: 5 << 3 | 0b010 = 0x2A
        let data = [0x2A, 4, 2, 1, 0xAA, 0xBB, 0xCC, 0xDD];
        let (header, header_size) = parse_obu_header(&data).unwrap();
        assert_eq!(header.obu_type, super::parser::ObuType::AudioFrame);
        assert!(header.trimming_status);
        assert_eq!(header.trim_end, 2);
        assert_eq!(header.trim_start, 1);
        assert_eq!(header.payload_size, 4);
        assert_eq!(header_size, 4);
    }

    #[test]
    fn parse_obu_header_with_extension() {
        // OBU type=31 (SequenceHeader), extension_flag=1
        // Type byte: 31 << 3 | 0b001 = 0xF9
        // payload_size=6, extension_size=2, ext bytes, payload
        let data = [
            0xF9, 6, // header prefix
            2, 0xAB, 0xCD, // extension
            b'i', b'a', b'm', b'f', 0, 0, // payload
        ];
        let (header, header_size) = parse_obu_header(&data).unwrap();
        assert_eq!(header.obu_type, super::parser::ObuType::SequenceHeader);
        assert!(header.extension_flag);
        assert_eq!(header.payload_size, 6);
        assert_eq!(header_size, 5);
    }

    #[test]
    fn parse_obu_header_audio_frame_id_range() {
        // AudioFrameId(0) and AudioFrameId(17) are valid.
        for id in 0u8..=17 {
            let type_byte = (6 + id) << 3;
            let data = [type_byte, 0];
            let (header, _) = parse_obu_header(&data).unwrap();
            assert_eq!(header.obu_type, super::parser::ObuType::AudioFrameId(id));
        }
    }

    #[test]
    fn parse_sequence_header_too_short_errors() {
        assert!(parse_sequence_header(b"ia").is_err());
    }
}

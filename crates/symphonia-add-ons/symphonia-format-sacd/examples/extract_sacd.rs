use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use rdsd2pcm::{DsdPcmConverter, DsdPcmOptions, PcmOutputEncoding};
use symphonia_codec_dst::DstDsdDecoder;
use symphonia_format_sacd::{
    SACD_SAMPLING_FREQUENCY, SacdAreaKind, SacdError, SacdFrameFormat, SacdIso, SacdPacket,
    SacdPacketReader, write_track_as_dsf,
};

type ExampleResult<T> = Result<T, Box<dyn Error>>;
const DEFAULT_WAV_RATE: u32 = 176_400;
const DST_DECODE_CHUNK_FRAMES: usize = 512;

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> ExampleResult<()> {
    let args = Args::parse()?;
    fs::create_dir_all(&args.output_dir)?;
    let iso = SacdIso::open(&args.input)?;
    let mut extracted = 0usize;

    for area in &iso.disc().areas {
        if !args.include_area(area.kind) {
            continue;
        }

        for track in &area.tracks {
            if let Some(selected) = args.track
                && track.number != selected
            {
                continue;
            }

            let filename = format!(
                "{:02}-{}-{}.{}",
                track.number,
                area_label(area.kind),
                sanitize(track.metadata.title.as_deref().unwrap_or("track")),
                args.format.extension(),
            );
            let path = args.output_dir.join(filename);
            let reader = iso.packet_reader(area.index, track.index)?;
            let output = BufWriter::new(File::create(&path)?);
            let frames = match args.format {
                OutputFormat::Dsf => write_track_as_dsf(output, reader)?,
                OutputFormat::WavPcm24 | OutputFormat::WavF32 => {
                    let encoding = args
                        .format
                        .wav_encoding()
                        .ok_or(SacdError::InvalidIso("selected format is not WAV"))?;
                    write_track_as_wav(output, reader, encoding, args.wav_rate)?
                }
            };
            println!("wrote {} ({} SACD frames)", path.display(), frames);
            extracted += 1;
        }
    }

    if extracted == 0 {
        return Err(SacdError::InvalidIso("no matching tracks were extracted").into());
    }

    Ok(())
}

#[derive(Debug)]
struct Args {
    input: PathBuf,
    output_dir: PathBuf,
    stereo: bool,
    multichannel: bool,
    track: Option<u8>,
    format: OutputFormat,
    wav_rate: u32,
}

impl Args {
    fn parse() -> ExampleResult<Self> {
        Self::parse_from(env::args().skip(1))
    }

    fn parse_from<I>(args: I) -> ExampleResult<Self>
    where
        I: IntoIterator<Item = String>,
    {
        let mut input = None;
        let mut output_dir = None;
        let mut stereo = false;
        let mut multichannel = false;
        let mut track = None;
        let mut format = OutputFormat::default();
        let mut wav_rate = DEFAULT_WAV_RATE;
        let mut iter = args.into_iter();

        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--stereo" => stereo = true,
                "--multichannel" => multichannel = true,
                "--all" => {
                    stereo = true;
                    multichannel = true;
                }
                "--track" => {
                    let Some(value) = iter.next() else {
                        return Err(SacdError::InvalidIso("--track requires a track number").into());
                    };
                    track = Some(
                        value
                            .parse::<u8>()
                            .map_err(|_| SacdError::InvalidIso("invalid --track value"))?,
                    );
                }
                "--format" | "--output-format" => {
                    let Some(value) = iter.next() else {
                        return Err(SacdError::InvalidIso("--format requires a value").into());
                    };
                    format = OutputFormat::parse(&value)?;
                }
                "--wav-rate" => {
                    let Some(value) = iter.next() else {
                        return Err(SacdError::InvalidIso("--wav-rate requires a value").into());
                    };
                    wav_rate = value
                        .parse::<u32>()
                        .map_err(|_| SacdError::InvalidIso("invalid --wav-rate value"))?;
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                _ if input.is_none() => input = Some(PathBuf::from(arg)),
                _ if output_dir.is_none() => output_dir = Some(PathBuf::from(arg)),
                _ => return Err(SacdError::InvalidIso("unexpected argument").into()),
            }
        }

        let Some(input) = input else {
            print_usage();
            return Err(SacdError::InvalidIso("missing input SACD ISO").into());
        };
        let output_dir = output_dir.unwrap_or_else(|| Path::new(".").to_path_buf());

        if !stereo && !multichannel {
            stereo = true;
            multichannel = true;
        }

        Ok(Self {
            input,
            output_dir,
            stereo,
            multichannel,
            track,
            format,
            wav_rate,
        })
    }

    fn include_area(&self, kind: SacdAreaKind) -> bool {
        match kind {
            SacdAreaKind::Stereo => self.stereo,
            SacdAreaKind::Multichannel => self.multichannel,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum OutputFormat {
    #[default]
    Dsf,
    WavPcm24,
    WavF32,
}

impl OutputFormat {
    fn parse(value: &str) -> ExampleResult<Self> {
        match value.to_ascii_lowercase().as_str() {
            "dsf" => Ok(Self::Dsf),
            "wav" | "wave" | "wav-pcm" | "wav-pcm24" | "wav24" => Ok(Self::WavPcm24),
            "wav-f32" | "wav-float" | "wave-f32" | "wave-float" => Ok(Self::WavF32),
            _ => Err(SacdError::InvalidIso("invalid --format value").into()),
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Dsf => "dsf",
            Self::WavPcm24 | Self::WavF32 => "wav",
        }
    }

    fn supported_values() -> &'static str {
        "dsf, wav, wav-f32"
    }

    fn wav_encoding(self) -> Option<WavEncoding> {
        match self {
            Self::Dsf => None,
            Self::WavPcm24 => Some(WavEncoding::Pcm24),
            Self::WavF32 => Some(WavEncoding::Float32),
        }
    }
}

fn write_track_as_wav<W: Write + Seek, R: std::io::Read + Seek>(
    writer: W,
    mut packets: SacdPacketReader<R>,
    encoding: WavEncoding,
    output_sample_rate: u32,
) -> ExampleResult<u64> {
    let Some(first_packet) = packets.next_packet()? else {
        return Err(SacdError::InvalidIso("selected track contains no packets").into());
    };

    let channel_count = usize::from(first_packet.channel_count);
    let mut converter = DsdPcmConverter::new(DsdPcmOptions {
        output_sample_rate,
        ..DsdPcmOptions::sacd(channel_count)
    })?;
    let mut wav = WavWriter::new(
        writer,
        first_packet.channel_count,
        converter.output_sample_rate(),
        encoding,
    )?;
    let mut scratch = Vec::with_capacity(
        converter.max_output_frames(first_packet.data.len())
            * channel_count
            * encoding.bytes_per_sample(),
    );
    let mut encoded_chunk = Vec::with_capacity(DST_DECODE_CHUNK_FRAMES);
    let mut frames = 0u64;

    match first_packet.frame_format {
        SacdFrameFormat::Dst => {
            encoded_chunk.push(first_packet.data);
            while let Some(packet) = packets.next_packet()? {
                encoded_chunk.push(packet.data);
                if encoded_chunk.len() == DST_DECODE_CHUNK_FRAMES {
                    frames += encoded_chunk.len() as u64;
                    decode_dst_chunk_to_wav(
                        &encoded_chunk,
                        channel_count,
                        &mut converter,
                        &mut wav,
                        &mut scratch,
                    )?;
                    encoded_chunk.clear();
                }
            }
            if !encoded_chunk.is_empty() {
                frames += encoded_chunk.len() as u64;
                decode_dst_chunk_to_wav(
                    &encoded_chunk,
                    channel_count,
                    &mut converter,
                    &mut wav,
                    &mut scratch,
                )?;
            }
        }
        SacdFrameFormat::Dsd3In14 | SacdFrameFormat::Dsd3In16 => {
            write_packet_as_wav(first_packet, &mut converter, &mut wav, &mut scratch)?;
            frames += 1;
            while let Some(packet) = packets.next_packet()? {
                write_packet_as_wav(packet, &mut converter, &mut wav, &mut scratch)?;
                frames += 1;
            }
        }
    }

    let _ = wav.finalize()?;
    Ok(frames)
}

fn decode_dst_chunk_to_wav<W: Write + Seek>(
    encoded_packets: &[Vec<u8>],
    channel_count: usize,
    converter: &mut DsdPcmConverter,
    wav: &mut WavWriter<W>,
    scratch: &mut Vec<u8>,
) -> ExampleResult<()> {
    let probe_decoder = DstDsdDecoder::new(channel_count, SACD_SAMPLING_FREQUENCY as usize)
        .map_err(|err| SacdError::DstDecode(err.to_string()))?;
    let decoded_frame_bytes = probe_decoder.dsd_frame_bytes();
    let decoded_frames: Result<Vec<Vec<u8>>, SacdError> = encoded_packets
        .par_iter()
        .map_init(
            || {
                DstDsdDecoder::new(channel_count, SACD_SAMPLING_FREQUENCY as usize)
                    .map_err(|err| err.to_string())
            },
            |decoder, encoded| {
                let decoder = decoder
                    .as_mut()
                    .map_err(|err| SacdError::DstDecode(err.clone()))?;
                let mut decoded = vec![0u8; decoded_frame_bytes];
                let written = decoder
                    .decode_frame(encoded, &mut decoded)
                    .map_err(|err| SacdError::DstDecode(err.to_string()))?;
                if written % channel_count != 0 {
                    return Err(SacdError::DstDecode(
                        "decoded frame is not channel aligned".to_string(),
                    ));
                }
                decoded.truncate(written);
                Ok(decoded)
            },
        )
        .collect();

    for decoded in decoded_frames? {
        write_dsd_packet_as_wav(converter, &decoded, wav, scratch)?;
    }
    Ok(())
}

fn write_packet_as_wav<W: Write + Seek>(
    packet: SacdPacket,
    converter: &mut DsdPcmConverter,
    wav: &mut WavWriter<W>,
    scratch: &mut Vec<u8>,
) -> ExampleResult<()> {
    write_dsd_packet_as_wav(converter, &packet.data, wav, scratch)
}

fn write_dsd_packet_as_wav<W: Write + Seek>(
    converter: &mut DsdPcmConverter,
    dsd: &[u8],
    wav: &mut WavWriter<W>,
    scratch: &mut Vec<u8>,
) -> ExampleResult<()> {
    converter.convert_interleaved_to_bytes(dsd, wav.pcm_encoding(), scratch)?;
    wav.write_pcm_bytes(scratch)?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WavEncoding {
    Pcm24,
    Float32,
}

impl WavEncoding {
    fn audio_format(self) -> u16 {
        match self {
            Self::Pcm24 => 0xfffe,
            Self::Float32 => 3,
        }
    }

    fn bits_per_sample(self) -> u16 {
        match self {
            Self::Pcm24 => 24,
            Self::Float32 => 32,
        }
    }

    fn bytes_per_sample(self) -> usize {
        usize::from(self.bits_per_sample() / 8)
    }

    fn fmt_len(self) -> u32 {
        match self {
            Self::Pcm24 => 40,
            Self::Float32 => 16,
        }
    }

    fn riff_overhead(self) -> u32 {
        20 + self.fmt_len()
    }

    fn pcm_output_encoding(self) -> PcmOutputEncoding {
        match self {
            Self::Pcm24 => PcmOutputEncoding::Pcm24Le,
            Self::Float32 => PcmOutputEncoding::Float32Le,
        }
    }
}

struct WavWriter<W> {
    writer: W,
    channels: u8,
    sample_rate: u32,
    encoding: WavEncoding,
    data_len: u64,
}

impl<W: Write + Seek> WavWriter<W> {
    fn new(
        mut writer: W,
        channels: u8,
        sample_rate: u32,
        encoding: WavEncoding,
    ) -> ExampleResult<Self> {
        write_wav_header(&mut writer, channels, sample_rate, encoding, 0)?;
        Ok(Self {
            writer,
            channels,
            sample_rate,
            encoding,
            data_len: 0,
        })
    }

    fn write_pcm_bytes(&mut self, bytes: &[u8]) -> ExampleResult<()> {
        let new_data_len = self
            .data_len
            .checked_add(bytes.len() as u64)
            .ok_or(SacdError::InvalidIso("WAV data size overflows"))?;
        if new_data_len > u64::from(u32::MAX - self.encoding.riff_overhead()) {
            return Err(SacdError::InvalidIso("WAV output exceeds the RIFF size limit").into());
        }
        self.writer.write_all(bytes)?;
        self.data_len = new_data_len;
        Ok(())
    }

    fn pcm_encoding(&self) -> PcmOutputEncoding {
        self.encoding.pcm_output_encoding()
    }

    fn finalize(mut self) -> ExampleResult<W> {
        self.writer.seek(SeekFrom::Start(0))?;
        let data_len = u32::try_from(self.data_len)
            .map_err(|_| SacdError::InvalidIso("WAV output exceeds the RIFF size limit"))?;
        write_wav_header(
            &mut self.writer,
            self.channels,
            self.sample_rate,
            self.encoding,
            data_len,
        )?;
        self.writer.flush()?;
        Ok(self.writer)
    }
}

fn write_wav_header<W: Write>(
    writer: &mut W,
    channels: u8,
    sample_rate: u32,
    encoding: WavEncoding,
    data_len: u32,
) -> std::io::Result<()> {
    let channels = channels.max(1);
    let sample_rate = sample_rate.max(1);
    let byte_rate = sample_rate * u32::from(channels) * encoding.bytes_per_sample() as u32;
    let block_align = u16::from(channels) * encoding.bytes_per_sample() as u16;
    let fmt_len = encoding.fmt_len();
    let riff_len = encoding.riff_overhead().saturating_add(data_len);

    writer.write_all(b"RIFF")?;
    writer.write_all(&riff_len.to_le_bytes())?;
    writer.write_all(b"WAVE")?;
    writer.write_all(b"fmt ")?;
    writer.write_all(&fmt_len.to_le_bytes())?;
    writer.write_all(&encoding.audio_format().to_le_bytes())?;
    writer.write_all(&u16::from(channels).to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&encoding.bits_per_sample().to_le_bytes())?;
    if encoding == WavEncoding::Pcm24 {
        writer.write_all(&22u16.to_le_bytes())?;
        writer.write_all(&encoding.bits_per_sample().to_le_bytes())?;
        writer.write_all(&wav_channel_mask(channels).to_le_bytes())?;
        writer.write_all(&[
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38,
            0x9b, 0x71,
        ])?;
    }
    writer.write_all(b"data")?;
    writer.write_all(&data_len.to_le_bytes())?;
    Ok(())
}

fn wav_channel_mask(channels: u8) -> u32 {
    match channels {
        1 => 0x0004,
        2 => 0x0003,
        5 => 0x0037,
        6 => 0x003f,
        _ => 0,
    }
}

fn print_usage() {
    eprintln!(
        "Usage: extract_sacd <input.iso> [output-dir] [--format FORMAT] [--wav-rate HZ] [--stereo] [--multichannel] [--all] [--track N]\n\nFormats: {}\nDefault WAV rate: {}",
        OutputFormat::supported_values(),
        DEFAULT_WAV_RATE
    );
}

fn area_label(kind: SacdAreaKind) -> &'static str {
    match kind {
        SacdAreaKind::Stereo => "stereo",
        SacdAreaKind::Multichannel => "multichannel",
    }
}

fn sanitize(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            out.push(ch);
        } else if ch.is_whitespace() {
            out.push('_');
        }
    }
    if out.is_empty() {
        "track".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_defaults_to_dsf() {
        let args = Args::parse_from(vec!["disc.iso".to_string()]).unwrap();
        assert_eq!(args.format, OutputFormat::Dsf);
        assert_eq!(args.wav_rate, DEFAULT_WAV_RATE);
    }

    #[test]
    fn parse_wav_output_format() {
        let args = Args::parse_from(vec![
            "disc.iso".to_string(),
            "out".to_string(),
            "--format".to_string(),
            "wav".to_string(),
        ])
        .unwrap();
        assert_eq!(args.format, OutputFormat::WavPcm24);
        assert_eq!(args.output_dir, PathBuf::from("out"));
    }

    #[test]
    fn parse_wav_rate() {
        let args = Args::parse_from(vec![
            "disc.iso".to_string(),
            "--format".to_string(),
            "wav".to_string(),
            "--wav-rate".to_string(),
            "88200".to_string(),
        ])
        .unwrap();
        assert_eq!(args.wav_rate, 88_200);
    }

    #[test]
    fn parse_float_wav_output_format() {
        let args = Args::parse_from(vec![
            "disc.iso".to_string(),
            "--format".to_string(),
            "wav-f32".to_string(),
        ])
        .unwrap();
        assert_eq!(args.format, OutputFormat::WavF32);
    }

    #[test]
    fn wav_writer_defaults_to_pcm24_header() {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = WavWriter::new(cursor, 2, 176_400, WavEncoding::Pcm24).unwrap();
        writer.write_pcm_bytes(&[0; 12]).unwrap();
        let cursor = writer.finalize().unwrap();
        let data = cursor.into_inner();

        assert_eq!(&data[0..4], b"RIFF");
        assert_eq!(
            u32::from_le_bytes([data[16], data[17], data[18], data[19]]),
            40
        );
        assert_eq!(u16::from_le_bytes([data[20], data[21]]), 0xfffe);
        assert_eq!(u16::from_le_bytes([data[22], data[23]]), 2);
        assert_eq!(
            u32::from_le_bytes([data[24], data[25], data[26], data[27]]),
            176_400
        );
        assert_eq!(u16::from_le_bytes([data[34], data[35]]), 24);
        assert_eq!(u16::from_le_bytes([data[38], data[39]]), 24);
        assert_eq!(
            u32::from_le_bytes([data[40], data[41], data[42], data[43]]),
            3
        );
        let data_chunk = find_chunk(&data, b"data").unwrap();
        assert_eq!(
            u32::from_le_bytes([
                data[data_chunk + 4],
                data[data_chunk + 5],
                data[data_chunk + 6],
                data[data_chunk + 7]
            ]),
            12
        );
    }

    #[test]
    fn wav_writer_can_emit_float32_header() {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = WavWriter::new(cursor, 2, 176_400, WavEncoding::Float32).unwrap();
        writer.write_pcm_bytes(&[0; 8]).unwrap();
        let cursor = writer.finalize().unwrap();
        let data = cursor.into_inner();

        assert_eq!(u16::from_le_bytes([data[20], data[21]]), 3);
        assert_eq!(u16::from_le_bytes([data[34], data[35]]), 32);
        let data_chunk = find_chunk(&data, b"data").unwrap();
        assert_eq!(
            u32::from_le_bytes([
                data[data_chunk + 4],
                data[data_chunk + 5],
                data[data_chunk + 6],
                data[data_chunk + 7]
            ]),
            8
        );
    }

    fn find_chunk(data: &[u8], marker: &[u8; 4]) -> Option<usize> {
        data.windows(marker.len())
            .position(|window| window == marker)
    }
}

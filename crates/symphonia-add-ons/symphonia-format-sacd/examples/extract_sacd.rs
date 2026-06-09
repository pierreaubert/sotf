use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use rdsd2pcm::{DsdPcmConverter, DsdPcmOptions};
use symphonia_codec_dst::DstDsdDecoder;
use symphonia_format_sacd::{
    SACD_SAMPLING_FREQUENCY, SacdAreaKind, SacdError, SacdFrameFormat, SacdIso, SacdPacket,
    SacdPacketReader, write_track_as_dsf,
};

type ExampleResult<T> = Result<T, Box<dyn Error>>;

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
                OutputFormat::Wav => write_track_as_wav(output, reader)?,
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
    Wav,
}

impl OutputFormat {
    fn parse(value: &str) -> ExampleResult<Self> {
        match value.to_ascii_lowercase().as_str() {
            "dsf" => Ok(Self::Dsf),
            "wav" | "wave" => Ok(Self::Wav),
            _ => Err(SacdError::InvalidIso("invalid --format value").into()),
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Dsf => "dsf",
            Self::Wav => "wav",
        }
    }

    fn supported_values() -> &'static str {
        "dsf, wav"
    }
}

fn write_track_as_wav<W: Write + Seek, R: std::io::Read + Seek>(
    writer: W,
    mut packets: SacdPacketReader<R>,
) -> ExampleResult<u64> {
    let Some(first_packet) = packets.next_packet()? else {
        return Err(SacdError::InvalidIso("selected track contains no packets").into());
    };

    let channel_count = usize::from(first_packet.channel_count);
    let mut converter = DsdPcmConverter::new(DsdPcmOptions::sacd(channel_count))?;
    let mut wav = FloatWavWriter::new(
        writer,
        first_packet.channel_count,
        converter.output_sample_rate(),
    )?;
    let mut scratch = Vec::with_capacity(
        converter.max_output_frames(first_packet.data.len()) * channel_count * size_of::<f32>(),
    );
    let mut frames = 0u64;

    match first_packet.frame_format {
        SacdFrameFormat::Dst => {
            let mut encoded_packets = vec![first_packet.data];
            while let Some(packet) = packets.next_packet()? {
                encoded_packets.push(packet.data);
            }
            frames = encoded_packets.len() as u64;

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
                write_dsd_packet_as_wav(&mut converter, &decoded, &mut wav, &mut scratch)?;
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

fn write_packet_as_wav<W: Write + Seek>(
    packet: SacdPacket,
    converter: &mut DsdPcmConverter,
    wav: &mut FloatWavWriter<W>,
    scratch: &mut Vec<u8>,
) -> ExampleResult<()> {
    write_dsd_packet_as_wav(converter, &packet.data, wav, scratch)
}

fn write_dsd_packet_as_wav<W: Write + Seek>(
    converter: &mut DsdPcmConverter,
    dsd: &[u8],
    wav: &mut FloatWavWriter<W>,
    scratch: &mut Vec<u8>,
) -> ExampleResult<()> {
    let pcm = converter.convert_interleaved(dsd)?;
    wav.write_planar_f32(pcm, scratch)?;
    Ok(())
}

struct FloatWavWriter<W> {
    writer: W,
    channels: u8,
    sample_rate: u32,
    data_len: u64,
}

impl<W: Write + Seek> FloatWavWriter<W> {
    fn new(mut writer: W, channels: u8, sample_rate: u32) -> ExampleResult<Self> {
        write_wav_header(&mut writer, channels, sample_rate, 0)?;
        Ok(Self {
            writer,
            channels,
            sample_rate,
            data_len: 0,
        })
    }

    fn write_planar_f32(&mut self, pcm: &[Vec<f32>], scratch: &mut Vec<u8>) -> ExampleResult<()> {
        let Some(first_channel) = pcm.first() else {
            return Err(SacdError::InvalidIso("decoded PCM has no channels").into());
        };
        let frames = first_channel.len();
        let channels = pcm.len();
        if pcm.iter().any(|channel| channel.len() != frames) {
            return Err(SacdError::InvalidIso("decoded PCM channel length mismatch").into());
        }
        scratch.clear();
        scratch.reserve(frames * channels * size_of::<f32>());

        for frame in 0..frames {
            for channel in pcm {
                let sample = channel[frame].clamp(-1.0, 1.0);
                scratch.extend_from_slice(&sample.to_le_bytes());
            }
        }

        let new_data_len = self
            .data_len
            .checked_add(scratch.len() as u64)
            .ok_or(SacdError::InvalidIso("WAV data size overflows"))?;
        if new_data_len > u64::from(u32::MAX) - 36 {
            return Err(SacdError::InvalidIso("WAV output exceeds the RIFF size limit").into());
        }
        self.writer.write_all(scratch)?;
        self.data_len = new_data_len;
        Ok(())
    }

    fn finalize(mut self) -> ExampleResult<W> {
        self.writer.seek(SeekFrom::Start(0))?;
        let data_len = u32::try_from(self.data_len)
            .map_err(|_| SacdError::InvalidIso("WAV output exceeds the RIFF size limit"))?;
        write_wav_header(&mut self.writer, self.channels, self.sample_rate, data_len)?;
        self.writer.flush()?;
        Ok(self.writer)
    }
}

fn write_wav_header<W: Write>(
    writer: &mut W,
    channels: u8,
    sample_rate: u32,
    data_len: u32,
) -> std::io::Result<()> {
    let channels = channels.max(1);
    let sample_rate = sample_rate.max(1);
    let byte_rate = sample_rate * u32::from(channels) * size_of::<f32>() as u32;
    let block_align = u16::from(channels) * size_of::<f32>() as u16;
    let riff_len = 36u32.saturating_add(data_len);

    writer.write_all(b"RIFF")?;
    writer.write_all(&riff_len.to_le_bytes())?;
    writer.write_all(b"WAVE")?;
    writer.write_all(b"fmt ")?;
    writer.write_all(&16u32.to_le_bytes())?;
    writer.write_all(&3u16.to_le_bytes())?;
    writer.write_all(&u16::from(channels).to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&32u16.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&data_len.to_le_bytes())?;
    Ok(())
}

fn print_usage() {
    eprintln!(
        "Usage: extract_sacd <input.iso> [output-dir] [--format FORMAT] [--stereo] [--multichannel] [--all] [--track N]\n\nFormats: {}",
        OutputFormat::supported_values()
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
        assert_eq!(args.format, OutputFormat::Wav);
        assert_eq!(args.output_dir, PathBuf::from("out"));
    }

    #[test]
    fn wav_writer_preserves_header_fields() {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = FloatWavWriter::new(cursor, 2, 176_400).unwrap();
        let pcm = vec![vec![0.0, 0.5], vec![-0.5, 1.5]];
        let mut scratch = Vec::new();
        writer.write_planar_f32(&pcm, &mut scratch).unwrap();
        let cursor = writer.finalize().unwrap();
        let data = cursor.into_inner();

        assert_eq!(&data[0..4], b"RIFF");
        assert_eq!(u16::from_le_bytes([data[22], data[23]]), 2);
        assert_eq!(
            u32::from_le_bytes([data[24], data[25], data[26], data[27]]),
            176_400
        );
        assert_eq!(
            u32::from_le_bytes([data[40], data[41], data[42], data[43]]),
            16
        );
    }
}

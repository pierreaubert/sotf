use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use encoding_rs::{Encoding, ISO_8859_10, SHIFT_JIS, UTF_8};
use rayon::prelude::*;
pub use symphonia_codec_dsd::{
    CODEC_ID_DSD, DsdPcmAudioDecoder, DsdPcmAudioDecoder as SacdDsdPcmAudioDecoder,
    register_decoders as register_dsd_decoders,
};
use symphonia_codec_dst::DstDsdDecoder;
pub use symphonia_codec_dst::{
    CODEC_ID_DST, DstAudioDecoder, DstAudioDecoder as SacdDstAudioDecoder,
    register_decoders as register_dst_decoders,
};
use symphonia_core::audio::Channels;
use symphonia_core::codecs::CodecParameters;
use symphonia_core::codecs::audio::{AudioCodecId, AudioCodecParameters};
use symphonia_core::codecs::registry::CodecRegistry;
use symphonia_core::common::FourCc;
use symphonia_core::errors::{
    Error as SymphoniaError, Result as SymphoniaResult, SeekErrorKind, seek_error,
};
use symphonia_core::formats::TrackFlags;
use symphonia_core::formats::prelude::*;
use symphonia_core::formats::probe::{Probe, ProbeFormatData, ProbeableFormat, Score, Scoreable};
use symphonia_core::io::{MediaSourceStream, ScopedStream};
use symphonia_core::meta::{
    Metadata, MetadataBuilder, MetadataId, MetadataInfo, MetadataLog, PerTrackMetadataBuilder,
    StandardTag, Tag,
};
use symphonia_core::packet::Packet;
use symphonia_core::support_format;
use symphonia_core::units::Time;
use thiserror::Error;

pub const SACD_LSN_SIZE: usize = 2048;
pub const SACD_SAMPLING_FREQUENCY: u32 = 2_822_400;
pub const SACD_FRAME_RATE: u32 = 75;
pub const SAMPLES_PER_FRAME: u64 = 588;
pub const FRAME_SIZE_64: usize = 4704;
pub const START_OF_MASTER_TOC: u32 = 510;
const MASTER_TOC_LEN: u32 = 10;
const MAX_AREA_TOC_SIZE_LSN: u16 = 96;
const MAX_PACKET_SIZE: usize = 2045;
const MAX_DST_SIZE: usize = 1024 * 64;
const SUPPORTED_VERSION_MAJOR: u8 = 1;
const SUPPORTED_VERSION_MINOR: u8 = 20;

const FORMAT_INFO: FormatInfo = FormatInfo {
    format: FormatId::new(FourCc::new(*b"SACD")),
    short_name: "sacd",
    long_name: "Super Audio CD ISO",
};

const METADATA_INFO: MetadataInfo = MetadataInfo {
    metadata: MetadataId::new(FourCc::new(*b"SACD")),
    short_name: "sacd",
    long_name: "Super Audio CD metadata",
};

#[derive(Debug, Error)]
pub enum SacdError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("invalid SACD ISO: {0}")]
    InvalidIso(&'static str),
    #[error("unsupported SACD version {major}.{minor:02}")]
    UnsupportedVersion { major: u8, minor: u8 },
    #[error("SACD ISO contains no supported stereo or multichannel area")]
    MissingArea,
    #[error("corrupt SACD track table: {0}")]
    CorruptTrackTable(&'static str),
    #[error("track selection is out of range")]
    TrackOutOfRange,
    #[error("SACD seek is unsupported for this request")]
    UnsupportedSeek,
    #[error("DST-to-DSD decode failed: {0}")]
    DstDecode(String),
}

pub type SacdResult<T> = Result<T, SacdError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SacdAreaKind {
    Stereo,
    Multichannel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SacdFrameFormat {
    Dst,
    Dsd3In14,
    Dsd3In16,
}

impl SacdFrameFormat {
    pub fn is_dst(self) -> bool {
        matches!(self, Self::Dst)
    }

    fn codec_id(self) -> AudioCodecId {
        if self.is_dst() {
            CODEC_ID_DST
        } else {
            CODEC_ID_DSD
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SacdMetadata {
    pub album_title: Option<String>,
    pub album_artist: Option<String>,
    pub disc_title: Option<String>,
    pub disc_artist: Option<String>,
}

impl SacdMetadata {
    fn display_title(&self) -> Option<&str> {
        self.disc_title.as_deref().or(self.album_title.as_deref())
    }

    fn display_artist(&self) -> Option<&str> {
        self.disc_artist.as_deref().or(self.album_artist.as_deref())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SacdTrackMetadata {
    pub title: Option<String>,
    pub performer: Option<String>,
    pub composer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SacdTrack {
    pub index: usize,
    pub number: u8,
    pub start_lsn: u32,
    pub length_lsn: u32,
    pub start_frames: u32,
    pub duration_frames: u32,
    pub metadata: SacdTrackMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SacdArea {
    pub index: usize,
    pub kind: SacdAreaKind,
    pub frame_format: SacdFrameFormat,
    pub channel_count: u8,
    pub loudspeaker_config: u8,
    pub extra_settings: u8,
    pub track_start_lsn: u32,
    pub track_end_lsn: u32,
    pub total_frames: u32,
    pub tracks: Vec<SacdTrack>,
}

impl SacdArea {
    pub fn track(&self, index: usize) -> SacdResult<&SacdTrack> {
        self.tracks.get(index).ok_or(SacdError::TrackOutOfRange)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SacdDisc {
    pub metadata: SacdMetadata,
    pub areas: Vec<SacdArea>,
}

impl SacdDisc {
    pub fn stereo_area(&self) -> Option<&SacdArea> {
        self.areas
            .iter()
            .find(|area| area.kind == SacdAreaKind::Stereo)
    }

    pub fn multichannel_area(&self) -> Option<&SacdArea> {
        self.areas
            .iter()
            .find(|area| area.kind == SacdAreaKind::Multichannel)
    }

    pub fn area(&self, kind: SacdAreaKind) -> Option<&SacdArea> {
        self.areas.iter().find(|area| area.kind == kind)
    }
}

#[derive(Debug, Clone)]
pub struct SacdIso {
    path: PathBuf,
    disc: SacdDisc,
}

impl SacdIso {
    pub fn open<P: AsRef<Path>>(path: P) -> SacdResult<Self> {
        let path = path.as_ref().to_path_buf();
        let mut file = File::open(&path)?;
        let disc = SacdDisc::read_from(&mut file)?;
        Ok(Self { path, disc })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn disc(&self) -> &SacdDisc {
        &self.disc
    }

    pub fn packet_reader(
        &self,
        area_index: usize,
        track_index: usize,
    ) -> SacdResult<SacdPacketReader<File>> {
        let file = File::open(&self.path)?;
        SacdPacketReader::new(file, self.disc.clone(), area_index, track_index)
    }
}

impl SacdDisc {
    pub fn read_from<R: Read + Seek>(reader: &mut R) -> SacdResult<Self> {
        parse_disc(reader)
    }
}

#[derive(Debug, Clone)]
pub struct SacdPacket {
    pub area_index: usize,
    pub track_index: usize,
    pub pts: u64,
    pub duration: u64,
    pub frame_format: SacdFrameFormat,
    pub channel_count: u8,
    pub data: Vec<u8>,
}

pub struct SacdPacketReader<R> {
    reader: R,
    area: SacdArea,
    track: SacdTrack,
    current_lsn: u32,
    end_lsn: u32,
    pending: FrameAccumulator,
    packet_index: u64,
}

impl<R: Read + Seek> SacdPacketReader<R> {
    pub fn new(
        reader: R,
        disc: SacdDisc,
        area_index: usize,
        track_index: usize,
    ) -> SacdResult<Self> {
        let area = disc
            .areas
            .get(area_index)
            .cloned()
            .ok_or(SacdError::TrackOutOfRange)?;
        let track = area
            .tracks
            .get(track_index)
            .cloned()
            .ok_or(SacdError::TrackOutOfRange)?;
        let current_lsn = track.start_lsn;
        let end_lsn = track
            .start_lsn
            .checked_add(track.length_lsn)
            .ok_or(SacdError::CorruptTrackTable("track LSN range overflows"))?;

        Ok(Self {
            reader,
            area,
            track,
            current_lsn,
            end_lsn,
            pending: FrameAccumulator::default(),
            packet_index: 0,
        })
    }

    pub fn next_packet(&mut self) -> SacdResult<Option<SacdPacket>> {
        let mut sector = [0u8; SACD_LSN_SIZE];

        while self.current_lsn < self.end_lsn {
            read_sector(&mut self.reader, self.current_lsn, &mut sector)?;
            self.current_lsn += 1;
            let is_last = self.current_lsn >= self.end_lsn;
            let frames =
                process_audio_sector(&sector, &self.area, &self.track, &mut self.pending, is_last)?;

            if let Some(frame) = frames.into_iter().next() {
                let packet = self.frame_to_packet(frame);
                return Ok(Some(packet));
            }
        }

        if let Some(frame) = self.pending.take_if_complete(&self.area) {
            return Ok(Some(self.frame_to_packet(frame)));
        }

        Ok(None)
    }

    fn frame_to_packet(&mut self, frame: CompleteFrame) -> SacdPacket {
        let pts = frame.timecode_frames.unwrap_or_else(|| {
            self.track
                .start_frames
                .saturating_add(self.packet_index as u32)
        }) as u64;
        self.packet_index += 1;
        SacdPacket {
            area_index: self.area.index,
            track_index: self.track.index,
            pts,
            duration: 1,
            frame_format: self.area.frame_format,
            channel_count: self.area.channel_count,
            data: frame.data,
        }
    }
}

pub fn write_track_as_dsf<W: Write, R: Read + Seek>(
    mut writer: W,
    mut packets: SacdPacketReader<R>,
) -> SacdResult<u64> {
    let channel_count = packets.area.channel_count;
    let mut audio = Vec::new();
    let mut frames = 0u64;

    if packets.area.frame_format.is_dst() {
        let mut encoded_packets = Vec::new();
        while let Some(packet) = packets.next_packet()? {
            encoded_packets.push(packet.data);
        }
        frames = encoded_packets.len() as u64;

        let probe_decoder =
            DstDsdDecoder::new(usize::from(channel_count), SACD_SAMPLING_FREQUENCY as usize)
                .map_err(|err| SacdError::DstDecode(err.to_string()))?;
        let decoded_frame_bytes = probe_decoder.dsd_frame_bytes();
        let decoded_frames: SacdResult<Vec<Vec<u8>>> = encoded_packets
            .par_iter()
            .map_init(
                || {
                    DstDsdDecoder::new(usize::from(channel_count), SACD_SAMPLING_FREQUENCY as usize)
                        .map_err(|err| err.to_string())
                },
                |decoder, encoded| {
                    let decoder = decoder
                        .as_mut()
                        .map_err(|err| SacdError::DstDecode(err.clone()))?;
                    let mut decoded = vec![0u8; decoded_frame_bytes];
                    let written = decode_dst_packet(decoder, encoded, &mut decoded)?;
                    if written % usize::from(channel_count) != 0 {
                        return Err(SacdError::DstDecode(
                            "decoded frame is not channel aligned".to_string(),
                        ));
                    }
                    decoded.truncate(written);
                    Ok(decoded)
                },
            )
            .collect();

        let decoded_frames = decoded_frames?;
        audio.reserve(decoded_frames.iter().map(Vec::len).sum());
        for decoded in decoded_frames {
            audio.extend_from_slice(&decoded);
        }
    } else {
        while let Some(packet) = packets.next_packet()? {
            audio.extend_from_slice(&packet.data);
            frames += 1;
        }
    }

    let sample_count_per_channel = (audio.len() as u64 / u64::from(channel_count)) * 8;
    write_dsf_header(
        &mut writer,
        channel_count,
        sample_count_per_channel,
        audio.len() as u64,
    )?;
    writer.write_all(&audio)?;
    Ok(frames)
}

fn decode_dst_packet(
    decoder: &mut DstDsdDecoder,
    encoded: &[u8],
    decoded: &mut Vec<u8>,
) -> SacdResult<usize> {
    let required = decoder.dsd_frame_bytes();
    if decoded.len() < required {
        decoded.resize(required, 0);
    }
    decoder
        .decode_frame(encoded, decoded)
        .map_err(|err| SacdError::DstDecode(err.to_string()))
}

fn write_dsf_header<W: Write>(
    writer: &mut W,
    channel_count: u8,
    sample_count_per_channel: u64,
    audio_len: u64,
) -> io::Result<()> {
    let total_size = 28 + 52 + 12 + audio_len;
    writer.write_all(b"DSD ")?;
    writer.write_all(&28u64.to_le_bytes())?;
    writer.write_all(&total_size.to_le_bytes())?;
    writer.write_all(&0u64.to_le_bytes())?;

    writer.write_all(b"fmt ")?;
    writer.write_all(&52u64.to_le_bytes())?;
    writer.write_all(&1u32.to_le_bytes())?;
    writer.write_all(&0u32.to_le_bytes())?;
    writer.write_all(&dsf_channel_type(channel_count).to_le_bytes())?;
    writer.write_all(&(channel_count as u32).to_le_bytes())?;
    writer.write_all(&SACD_SAMPLING_FREQUENCY.to_le_bytes())?;
    writer.write_all(&1u32.to_le_bytes())?;
    writer.write_all(&sample_count_per_channel.to_le_bytes())?;
    writer.write_all(&4096u32.to_le_bytes())?;
    writer.write_all(&0u32.to_le_bytes())?;

    writer.write_all(b"data")?;
    writer.write_all(&(audio_len + 12).to_le_bytes())?;
    Ok(())
}

fn dsf_channel_type(channel_count: u8) -> u32 {
    match channel_count {
        1 => 1,
        2 => 2,
        5 => 5,
        6 => 6,
        _ => 0,
    }
}

#[derive(Debug, Clone, Default)]
struct FrameAccumulator {
    data: Vec<u8>,
    started: bool,
    dst_encoded: bool,
    sector_count: i32,
    channel_count: u8,
    timecode_frames: Option<u32>,
}

impl FrameAccumulator {
    fn begin(
        &mut self,
        dst_encoded: bool,
        sector_count: u8,
        channel_count: u8,
        timecode_frames: Option<u32>,
    ) {
        self.data.clear();
        self.started = true;
        self.dst_encoded = dst_encoded;
        self.sector_count = i32::from(sector_count);
        self.channel_count = channel_count;
        self.timecode_frames = timecode_frames;
    }

    fn append(&mut self, data: &[u8]) -> SacdResult<()> {
        if self.data.len() + data.len() > MAX_DST_SIZE {
            self.started = false;
            return Err(SacdError::CorruptTrackTable(
                "audio frame exceeds maximum packet size",
            ));
        }
        self.data.extend_from_slice(data);
        if self.dst_encoded {
            self.sector_count -= 1;
        }
        Ok(())
    }

    fn take_if_complete(&mut self, area: &SacdArea) -> Option<CompleteFrame> {
        if !self.started || self.data.is_empty() {
            return None;
        }

        let complete = if self.dst_encoded {
            self.sector_count <= 0
        } else {
            self.data.len() == usize::from(area.channel_count) * FRAME_SIZE_64
        };

        if complete {
            self.started = false;
            Some(CompleteFrame {
                data: std::mem::take(&mut self.data),
                timecode_frames: self.timecode_frames,
            })
        } else {
            None
        }
    }
}

#[derive(Debug)]
struct CompleteFrame {
    data: Vec<u8>,
    timecode_frames: Option<u32>,
}

fn process_audio_sector(
    sector: &[u8; SACD_LSN_SIZE],
    area: &SacdArea,
    track: &SacdTrack,
    pending: &mut FrameAccumulator,
    last_sector: bool,
) -> SacdResult<Vec<CompleteFrame>> {
    let header = sector[0];
    let dst_encoded = (header & 0x01) != 0;
    let frame_info_count = (header >> 2) & 0x07;
    let packet_info_count = (header >> 5) & 0x07;

    if packet_info_count > 7 {
        return Err(SacdError::CorruptTrackTable(
            "audio sector has too many packets",
        ));
    }

    let mut cursor = 1usize;
    let mut packet_infos = Vec::with_capacity(packet_info_count as usize);
    for _ in 0..packet_info_count {
        if cursor + 2 > sector.len() {
            return Err(SacdError::CorruptTrackTable(
                "audio sector packet table is truncated",
            ));
        }
        let b0 = sector[cursor];
        let b1 = sector[cursor + 1];
        cursor += 2;
        let info = PacketInfo {
            frame_start: (b0 & 0x80) != 0,
            data_type: (b0 >> 3) & 0x07,
            packet_length: (((b0 & 0x07) as usize) << 8) | b1 as usize,
        };
        if info.packet_length > MAX_PACKET_SIZE {
            return Err(SacdError::CorruptTrackTable(
                "audio packet exceeds SACD packet limit",
            ));
        }
        packet_infos.push(info);
    }

    let mut frame_infos = Vec::with_capacity(frame_info_count as usize);
    for _ in 0..frame_info_count {
        let info_len = if dst_encoded { 4 } else { 3 };
        if cursor + info_len > sector.len() {
            return Err(SacdError::CorruptTrackTable(
                "audio sector frame table is truncated",
            ));
        }
        let minutes = sector[cursor];
        let seconds = sector[cursor + 1];
        let frames = sector[cursor + 2];
        let flags = if dst_encoded { sector[cursor + 3] } else { 0 };
        cursor += info_len;
        frame_infos.push(FrameInfo {
            timecode_frames: time_to_frames(minutes, seconds, frames),
            sector_count: (flags >> 2) & 0x1f,
            channel_count: dst_channel_count(flags).unwrap_or(area.channel_count),
        });
    }

    let mut completed = Vec::new();
    let mut frame_info_idx = 0usize;

    for info in packet_infos {
        if cursor + info.packet_length > sector.len() {
            return Err(SacdError::CorruptTrackTable(
                "audio packet data is truncated",
            ));
        }
        let packet_data = &sector[cursor..cursor + info.packet_length];
        cursor += info.packet_length;

        if info.data_type != 2 {
            continue;
        }

        if info.frame_start {
            if let Some(frame) = pending.take_if_complete(area) {
                completed.push(frame);
            }
            let frame_info = frame_infos
                .get(frame_info_idx)
                .copied()
                .unwrap_or(FrameInfo {
                    timecode_frames: Some(
                        track.start_frames.saturating_add(completed.len() as u32),
                    ),
                    sector_count: 0,
                    channel_count: area.channel_count,
                });
            frame_info_idx += 1;
            pending.begin(
                dst_encoded,
                frame_info.sector_count,
                frame_info.channel_count,
                frame_info.timecode_frames,
            );
        }

        if pending.started {
            pending.append(packet_data)?;
        }
    }

    if last_sector && let Some(frame) = pending.take_if_complete(area) {
        completed.push(frame);
    }

    Ok(completed)
}

#[derive(Debug, Clone, Copy)]
struct PacketInfo {
    frame_start: bool,
    data_type: u8,
    packet_length: usize,
}

#[derive(Debug, Clone, Copy)]
struct FrameInfo {
    timecode_frames: Option<u32>,
    sector_count: u8,
    channel_count: u8,
}

fn dst_channel_count(flags: u8) -> Option<u8> {
    let bit_2 = (flags & 0x02) != 0;
    let bit_3 = (flags & 0x01) != 0;
    match (bit_2, bit_3) {
        (true, false) => Some(6),
        (false, true) => Some(5),
        _ => None,
    }
}

fn parse_disc<R: Read + Seek>(reader: &mut R) -> SacdResult<SacdDisc> {
    let master = read_master_toc(reader)?;
    let metadata = read_master_text(reader).unwrap_or_default();
    let mut areas = Vec::new();

    if master.area_1_toc_1_start > 0
        && let Some(area) = read_area_with_backup(
            reader,
            areas.len(),
            master.area_1_toc_1_start,
            master.area_1_toc_2_start,
            master.area_1_toc_size,
        )?
    {
        areas.push(area);
    }

    if master.area_2_toc_1_start > 0
        && let Some(area) = read_area_with_backup(
            reader,
            areas.len(),
            master.area_2_toc_1_start,
            master.area_2_toc_2_start,
            master.area_2_toc_size,
        )?
    {
        areas.push(area);
    }

    if areas.is_empty() {
        return Err(SacdError::MissingArea);
    }

    Ok(SacdDisc { metadata, areas })
}

#[derive(Debug, Clone, Copy)]
struct MasterToc {
    area_1_toc_1_start: u32,
    area_1_toc_2_start: u32,
    area_2_toc_1_start: u32,
    area_2_toc_2_start: u32,
    area_1_toc_size: u16,
    area_2_toc_size: u16,
}

fn read_master_toc<R: Read + Seek>(reader: &mut R) -> SacdResult<MasterToc> {
    let mut data = vec![0u8; MASTER_TOC_LEN as usize * SACD_LSN_SIZE];
    read_sectors(reader, START_OF_MASTER_TOC, MASTER_TOC_LEN, &mut data)?;

    if &data[0..8] != b"SACDMTOC" {
        return Err(SacdError::InvalidIso("missing SACDMTOC marker"));
    }

    let major = data[8];
    let minor = data[9];
    check_version(major, minor)?;

    Ok(MasterToc {
        area_1_toc_1_start: be_u32(&data, 64)?,
        area_1_toc_2_start: be_u32(&data, 68)?,
        area_2_toc_1_start: be_u32(&data, 72)?,
        area_2_toc_2_start: be_u32(&data, 76)?,
        area_1_toc_size: be_u16(&data, 84)?,
        area_2_toc_size: be_u16(&data, 86)?,
    })
}

fn read_master_text<R: Read + Seek>(reader: &mut R) -> SacdResult<SacdMetadata> {
    let mut sector = [0u8; SACD_LSN_SIZE];
    read_sector(reader, START_OF_MASTER_TOC + 1, &mut sector)?;
    if &sector[0..8] != b"SACDText" {
        return Ok(SacdMetadata::default());
    }

    Ok(SacdMetadata {
        album_title: read_text_at(&sector, be_u16(&sector, 16)? as usize),
        album_artist: read_text_at(&sector, be_u16(&sector, 18)? as usize),
        disc_title: read_text_at(&sector, be_u16(&sector, 32)? as usize),
        disc_artist: read_text_at(&sector, be_u16(&sector, 34)? as usize),
    })
}

fn read_area_with_backup<R: Read + Seek>(
    reader: &mut R,
    index: usize,
    primary_lsn: u32,
    backup_lsn: u32,
    toc_size: u16,
) -> SacdResult<Option<SacdArea>> {
    if toc_size == 0 {
        return Ok(None);
    }
    if toc_size > MAX_AREA_TOC_SIZE_LSN {
        return Err(SacdError::InvalidIso(
            "area TOC is larger than the SACD limit",
        ));
    }

    match read_area(reader, index, primary_lsn, toc_size) {
        Ok(area) => Ok(Some(area)),
        Err(primary_err) if backup_lsn > 0 => {
            match read_area(reader, index, backup_lsn, toc_size) {
                Ok(area) => Ok(Some(area)),
                Err(_) => Err(primary_err),
            }
        }
        Err(err) => Err(err),
    }
}

fn read_area<R: Read + Seek>(
    reader: &mut R,
    index: usize,
    toc_lsn: u32,
    toc_size: u16,
) -> SacdResult<SacdArea> {
    let mut data = vec![0u8; toc_size as usize * SACD_LSN_SIZE];
    read_sectors(reader, toc_lsn, toc_size as u32, &mut data)?;

    let id = &data[0..8];
    let kind = match id {
        b"TWOCHTOC" => SacdAreaKind::Stereo,
        b"MULCHTOC" => SacdAreaKind::Multichannel,
        _ => return Err(SacdError::InvalidIso("missing area TOC marker")),
    };

    check_version(data[8], data[9])?;

    let declared_size = be_u16(&data, 10)?;
    if declared_size == 0 || declared_size > toc_size {
        return Err(SacdError::InvalidIso("invalid area TOC size"));
    }

    let frame_format = match data[21] & 0x0f {
        0 => SacdFrameFormat::Dst,
        2 => SacdFrameFormat::Dsd3In14,
        3 => SacdFrameFormat::Dsd3In16,
        _ => return Err(SacdError::InvalidIso("unsupported SACD frame format")),
    };

    let channel_count = data[32];
    let config = data[33];
    let extra_settings = config & 0x07;
    let loudspeaker_config = config >> 3;
    let total_frames = time_to_frames(data[64], data[65], data[66]).unwrap_or(0);
    let track_count = data[69] as usize;
    let track_start_lsn = be_u32(&data, 72)?;
    let track_end_lsn = be_u32(&data, 76)?;

    if track_count == 0 {
        return Err(SacdError::CorruptTrackTable("area contains no tracks"));
    }

    let track_offsets = find_chunk(&data, b"SACDTRL1").ok_or(SacdError::CorruptTrackTable(
        "missing SACDTRL1 track LSN table",
    ))?;
    let track_times = find_chunk(&data, b"SACDTRL2").ok_or(SacdError::CorruptTrackTable(
        "missing SACDTRL2 track time table",
    ))?;
    let track_text = find_chunk(&data, b"SACDTTxt");

    let mut tracks = Vec::with_capacity(track_count);
    for track_idx in 0..track_count {
        let start = be_u32(&data, track_offsets + 8 + track_idx * 4)?;
        let declared_len = be_u32(&data, track_offsets + 8 + 255 * 4 + track_idx * 4)?;
        let next_start = if track_idx + 1 < track_count {
            be_u32(&data, track_offsets + 8 + (track_idx + 1) * 4)?
        } else {
            track_end_lsn
                .checked_add(1)
                .ok_or(SacdError::CorruptTrackTable("track end LSN overflows"))?
        };
        let length = next_start
            .checked_sub(start)
            .ok_or(SacdError::CorruptTrackTable(
                "track starts are not monotonic",
            ))?;
        let length = if declared_len > 0 {
            declared_len.min(length)
        } else {
            length
        };

        if start < track_start_lsn || start > track_end_lsn || length == 0 {
            return Err(SacdError::CorruptTrackTable(
                "track LSN range is outside area",
            ));
        }

        let start_time_off = track_times + 8 + track_idx * 4;
        let duration_time_off = track_times + 8 + 255 * 4 + track_idx * 4;
        let start_frames = time_to_frames(
            data[start_time_off],
            data[start_time_off + 1],
            data[start_time_off + 2],
        )
        .unwrap_or(0);
        let duration_frames = time_to_frames(
            data[duration_time_off],
            data[duration_time_off + 1],
            data[duration_time_off + 2],
        )
        .unwrap_or(0);

        tracks.push(SacdTrack {
            index: track_idx,
            number: track_idx.saturating_add(1).min(u8::MAX as usize) as u8,
            start_lsn: start,
            length_lsn: length,
            start_frames,
            duration_frames,
            metadata: track_text
                .and_then(|offset| read_track_text(&data, offset, track_idx).ok())
                .unwrap_or_default(),
        });
    }

    Ok(SacdArea {
        index,
        kind,
        frame_format,
        channel_count,
        loudspeaker_config,
        extra_settings,
        track_start_lsn,
        track_end_lsn,
        total_frames,
        tracks,
    })
}

fn read_track_text(
    data: &[u8],
    chunk_offset: usize,
    track_idx: usize,
) -> SacdResult<SacdTrackMetadata> {
    let pos_off = chunk_offset + 8 + track_idx * 2;
    if pos_off + 2 > data.len() {
        return Ok(SacdTrackMetadata::default());
    }
    let rel = be_u16(data, pos_off)? as usize;
    if rel == 0 {
        return Ok(SacdTrackMetadata::default());
    }
    let mut cursor = chunk_offset + rel;
    if cursor + 4 > data.len() {
        return Ok(SacdTrackMetadata::default());
    }

    let count = data[cursor] as usize;
    cursor += 4;
    let mut metadata = SacdTrackMetadata::default();

    for _ in 0..count {
        if cursor + 2 > data.len() {
            break;
        }
        let text_type = data[cursor];
        cursor += 2;
        let Some((text, next)) = read_c_string(data, cursor) else {
            break;
        };
        cursor = next;
        match text_type {
            0x01 => metadata.title = Some(text),
            0x02 => metadata.performer = Some(text),
            0x04 => metadata.composer = Some(text),
            _ => {}
        }
    }

    Ok(metadata)
}

fn read_text_at(data: &[u8], offset: usize) -> Option<String> {
    if offset == 0 || offset >= data.len() {
        return None;
    }
    read_c_string(data, offset).map(|(text, _)| text)
}

fn read_c_string(data: &[u8], offset: usize) -> Option<(String, usize)> {
    let end = data[offset..]
        .iter()
        .position(|byte| *byte == 0)
        .map(|pos| offset + pos)
        .unwrap_or(data.len());
    if end <= offset {
        return None;
    }
    let raw = &data[offset..end];
    let encoding = detect_encoding(raw);
    let (text, _, _) = encoding.decode(raw);
    let text = text.trim_matches(char::from(0)).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some((text, end.saturating_add(1)))
    }
}

fn detect_encoding(raw: &[u8]) -> &'static Encoding {
    if std::str::from_utf8(raw).is_ok() {
        UTF_8
    } else if raw.iter().any(|byte| *byte >= 0x80) {
        SHIFT_JIS
    } else {
        ISO_8859_10
    }
}

fn find_chunk(data: &[u8], marker: &[u8; 8]) -> Option<usize> {
    data.windows(marker.len())
        .position(|window| window == marker)
}

fn check_version(major: u8, minor: u8) -> SacdResult<()> {
    if major > SUPPORTED_VERSION_MAJOR
        || (major == SUPPORTED_VERSION_MAJOR && minor > SUPPORTED_VERSION_MINOR)
    {
        Err(SacdError::UnsupportedVersion { major, minor })
    } else {
        Ok(())
    }
}

fn time_to_frames(minutes: u8, seconds: u8, frames: u8) -> Option<u32> {
    if seconds >= 60 || frames >= SACD_FRAME_RATE as u8 {
        return None;
    }
    Some(minutes as u32 * 60 * SACD_FRAME_RATE + seconds as u32 * SACD_FRAME_RATE + frames as u32)
}

fn be_u16(data: &[u8], offset: usize) -> SacdResult<u16> {
    if offset + 2 > data.len() {
        return Err(SacdError::InvalidIso("unexpected end of structure"));
    }
    Ok(u16::from_be_bytes([data[offset], data[offset + 1]]))
}

fn be_u32(data: &[u8], offset: usize) -> SacdResult<u32> {
    if offset + 4 > data.len() {
        return Err(SacdError::InvalidIso("unexpected end of structure"));
    }
    Ok(u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]))
}

fn read_sector<R: Read + Seek>(
    reader: &mut R,
    lsn: u32,
    dest: &mut [u8; SACD_LSN_SIZE],
) -> io::Result<()> {
    reader.seek(SeekFrom::Start(u64::from(lsn) * SACD_LSN_SIZE as u64))?;
    reader.read_exact(dest)?;
    Ok(())
}

fn read_sectors<R: Read + Seek>(
    reader: &mut R,
    start_lsn: u32,
    count: u32,
    dest: &mut [u8],
) -> io::Result<()> {
    let expected = count as usize * SACD_LSN_SIZE;
    if dest.len() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "destination length does not match sector count",
        ));
    }
    reader.seek(SeekFrom::Start(u64::from(start_lsn) * SACD_LSN_SIZE as u64))?;
    reader.read_exact(dest)?;
    Ok(())
}

pub fn register_all(probe: &mut Probe) {
    probe.register_format::<SacdFormatReader<'_>>();
}

pub fn register_decoders(registry: &mut CodecRegistry) {
    register_dsd_decoders(registry);
    register_dst_decoders(registry);
}

pub struct SacdFormatReader<'s> {
    reader: MediaSourceStream<'s>,
    disc: SacdDisc,
    media_info: MediaInfo,
    tracks: Vec<Track>,
    packet_tracks: Vec<PacketTrack>,
    packet_track_index: usize,
    pending: FrameAccumulator,
    metadata: MetadataLog,
}

#[derive(Debug, Clone)]
struct PacketTrack {
    symphonia_track_id: u32,
    area_index: usize,
    track_index: usize,
    current_lsn: u32,
    end_lsn: u32,
    packet_index: u64,
}

impl<'s> SacdFormatReader<'s> {
    pub fn try_new(
        mut reader: MediaSourceStream<'s>,
        _opts: FormatOptions,
    ) -> SymphoniaResult<Self> {
        let disc = SacdDisc::read_from(&mut reader).map_err(to_symphonia_error)?;
        let mut tracks = Vec::new();
        let mut packet_tracks = Vec::new();

        for area in &disc.areas {
            for track in &area.tracks {
                let sym_id = symphonia_track_id(area.index, track.index);
                let mut codec_params = AudioCodecParameters::new();
                codec_params
                    .for_codec(area.frame_format.codec_id())
                    .with_sample_rate(SACD_SAMPLING_FREQUENCY)
                    .with_bits_per_sample(1)
                    .with_bits_per_coded_sample(1)
                    .with_channels(Channels::Discrete(u16::from(area.channel_count)))
                    .with_max_frames_per_packet(SAMPLES_PER_FRAME)
                    .with_frames_per_block(SAMPLES_PER_FRAME);

                let mut sym_track = Track::new(sym_id);
                sym_track
                    .with_codec_params(CodecParameters::Audio(codec_params))
                    .with_time_base(sacd_time_base())
                    .with_duration(Duration::new(track.duration_frames as u64))
                    .with_num_frames(track.duration_frames as u64)
                    .with_start_ts(Timestamp::new(track.start_frames as i64));
                if tracks.is_empty() {
                    sym_track.with_flags(TrackFlags::DEFAULT);
                }
                tracks.push(sym_track);

                packet_tracks.push(PacketTrack {
                    symphonia_track_id: sym_id,
                    area_index: area.index,
                    track_index: track.index,
                    current_lsn: track.start_lsn,
                    end_lsn: track.start_lsn.saturating_add(track.length_lsn),
                    packet_index: 0,
                });
            }
        }

        let media_info = MediaInfo::from_tracks(&tracks);
        let metadata = build_metadata_log(&disc);

        Ok(Self {
            reader,
            disc,
            media_info,
            tracks,
            packet_tracks,
            packet_track_index: 0,
            pending: FrameAccumulator::default(),
            metadata,
        })
    }
}

impl Scoreable for SacdFormatReader<'_> {
    fn score(_src: ScopedStream<&mut MediaSourceStream<'_>>) -> SymphoniaResult<Score> {
        Ok(Score::Supported(1))
    }
}

impl ProbeableFormat<'_> for SacdFormatReader<'_> {
    fn try_probe_new(
        mss: MediaSourceStream<'_>,
        opts: FormatOptions,
    ) -> SymphoniaResult<Box<dyn FormatReader + '_>> {
        Ok(Box::new(SacdFormatReader::try_new(mss, opts)?))
    }

    fn probe_data() -> &'static [ProbeFormatData] {
        &[support_format!(
            FORMAT_INFO,
            &["iso"],
            &["application/x-sacd-iso"],
            &[b"SACD"]
        )]
    }
}

impl FormatReader for SacdFormatReader<'_> {
    fn format_info(&self) -> &FormatInfo {
        &FORMAT_INFO
    }

    fn media_info(&self) -> &MediaInfo {
        &self.media_info
    }

    fn metadata(&mut self) -> Metadata<'_> {
        self.metadata.metadata()
    }

    fn seek(&mut self, _mode: SeekMode, to: SeekTo) -> SymphoniaResult<SeekedTo> {
        let (track_id, required_ts) = match to {
            SeekTo::Time { time, track_id } => {
                if time != Time::ZERO {
                    return seek_error(SeekErrorKind::OutOfRange);
                }
                (
                    track_id.unwrap_or_else(|| self.tracks.first().map(|t| t.id).unwrap_or(0)),
                    Timestamp::ZERO,
                )
            }
            SeekTo::Timestamp { ts, track_id } => {
                if ts != Timestamp::ZERO {
                    return seek_error(SeekErrorKind::OutOfRange);
                }
                (track_id, ts)
            }
        };

        self.packet_track_index = 0;
        self.pending = FrameAccumulator::default();
        for packet_track in &mut self.packet_tracks {
            let area = self
                .disc
                .areas
                .get(packet_track.area_index)
                .ok_or(SymphoniaError::SeekError(SeekErrorKind::InvalidTrack))?;
            let track = area
                .tracks
                .get(packet_track.track_index)
                .ok_or(SymphoniaError::SeekError(SeekErrorKind::InvalidTrack))?;
            packet_track.current_lsn = track.start_lsn;
            packet_track.packet_index = 0;
        }

        Ok(SeekedTo {
            track_id,
            required_ts,
            actual_ts: Timestamp::ZERO,
        })
    }

    fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    fn next_packet(&mut self) -> SymphoniaResult<Option<Packet>> {
        let mut sector = [0u8; SACD_LSN_SIZE];

        loop {
            let Some(packet_track) = self.packet_tracks.get_mut(self.packet_track_index) else {
                return Ok(None);
            };
            let area = self
                .disc
                .areas
                .get(packet_track.area_index)
                .ok_or(SymphoniaError::DecodeError("sacd: area index is invalid"))?;
            let track = area
                .tracks
                .get(packet_track.track_index)
                .ok_or(SymphoniaError::DecodeError("sacd: track index is invalid"))?;

            if packet_track.current_lsn >= packet_track.end_lsn {
                if let Some(frame) = self.pending.take_if_complete(area) {
                    let packet = make_symphonia_packet(packet_track, track, frame);
                    return Ok(Some(packet));
                }
                self.packet_track_index += 1;
                self.pending = FrameAccumulator::default();
                continue;
            }

            read_sector(&mut self.reader, packet_track.current_lsn, &mut sector)?;
            packet_track.current_lsn += 1;
            let last = packet_track.current_lsn >= packet_track.end_lsn;
            let mut frames = process_audio_sector(&sector, area, track, &mut self.pending, last)
                .map_err(to_symphonia_error)?;
            if let Some(frame) = frames.drain(..).next() {
                let packet = make_symphonia_packet(packet_track, track, frame);
                return Ok(Some(packet));
            }
        }
    }

    fn into_inner<'s>(self: Box<Self>) -> MediaSourceStream<'s>
    where
        Self: 's,
    {
        self.reader
    }
}

fn make_symphonia_packet(
    packet_track: &mut PacketTrack,
    track: &SacdTrack,
    frame: CompleteFrame,
) -> Packet {
    let pts = frame.timecode_frames.unwrap_or_else(|| {
        track
            .start_frames
            .saturating_add(packet_track.packet_index as u32)
    });
    packet_track.packet_index += 1;
    Packet::new(
        packet_track.symphonia_track_id,
        Timestamp::new(pts as i64),
        Duration::new(1),
        frame.data,
    )
}

fn symphonia_track_id(area_index: usize, track_index: usize) -> u32 {
    (area_index as u32) * 1000 + track_index as u32 + 1
}

fn sacd_time_base() -> TimeBase {
    TimeBase::new(
        NonZero::new(1).unwrap(),
        NonZero::new(SACD_FRAME_RATE).unwrap(),
    )
}

fn build_metadata_log(disc: &SacdDisc) -> MetadataLog {
    let mut log = MetadataLog::default();
    let mut builder = MetadataBuilder::new(METADATA_INFO);
    if let Some(title) = disc.metadata.display_title() {
        builder.add_tag(Tag::new_from_parts(
            "TITLE",
            title,
            Some(StandardTag::Album(Arc::new(title.to_string()))),
        ));
    }
    if let Some(artist) = disc.metadata.display_artist() {
        builder.add_tag(Tag::new_from_parts(
            "ARTIST",
            artist,
            Some(StandardTag::AlbumArtist(Arc::new(artist.to_string()))),
        ));
    }

    for area in &disc.areas {
        for track in &area.tracks {
            let mut per_track = PerTrackMetadataBuilder::new(u64::from(symphonia_track_id(
                area.index,
                track.index,
            )));
            if let Some(title) = &track.metadata.title {
                per_track.add_tag(Tag::new_from_parts(
                    "TITLE",
                    title.as_str(),
                    Some(StandardTag::TrackTitle(Arc::new(title.clone()))),
                ));
            }
            if let Some(performer) = &track.metadata.performer {
                per_track.add_tag(Tag::new_from_parts(
                    "PERFORMER",
                    performer.as_str(),
                    Some(StandardTag::Artist(Arc::new(performer.clone()))),
                ));
            }
            builder.add_track(per_track.build());
        }
    }

    log.push(builder.build());
    log
}

fn to_symphonia_error(error: SacdError) -> SymphoniaError {
    match error {
        SacdError::Io(err) => SymphoniaError::IoError(err),
        SacdError::UnsupportedSeek => SymphoniaError::SeekError(SeekErrorKind::OutOfRange),
        SacdError::DstDecode(_) => SymphoniaError::DecodeError("sacd: DST-to-DSD decode failed"),
        _ => SymphoniaError::DecodeError("sacd: invalid or unsupported SACD ISO"),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use symphonia_core::audio::{Audio, GenericAudioBufferRef};
    use symphonia_core::codecs::audio::AudioDecoderOptions;
    use symphonia_core::codecs::registry::CodecRegistry;
    use symphonia_core::formats::probe::Hint;
    use symphonia_core::io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions};

    use super::*;

    #[test]
    fn rejects_bad_master_magic() {
        let mut image =
            vec![0u8; (START_OF_MASTER_TOC as usize + MASTER_TOC_LEN as usize) * SACD_LSN_SIZE];
        let err = SacdDisc::read_from(&mut Cursor::new(&mut image)).unwrap_err();
        assert!(matches!(err, SacdError::InvalidIso(_)));
    }

    #[test]
    fn parses_stereo_only_disc() {
        let image = fixture(false, false);
        let disc = SacdDisc::read_from(&mut Cursor::new(image.clone())).unwrap();
        assert_eq!(disc.areas.len(), 1);
        let area = disc.stereo_area().unwrap();
        assert_eq!(area.kind, SacdAreaKind::Stereo);
        assert_eq!(area.channel_count, 2);
        assert_eq!(area.tracks[0].start_lsn, 600);
        assert_eq!(area.tracks[0].length_lsn, 5);
        assert_eq!(area.tracks[0].duration_frames, 2);
    }

    #[test]
    fn parses_dual_area_disc() {
        let image = fixture(true, false);
        let disc = SacdDisc::read_from(&mut Cursor::new(image.clone())).unwrap();
        assert!(disc.stereo_area().is_some());
        assert!(disc.multichannel_area().is_some());
        assert_eq!(disc.multichannel_area().unwrap().channel_count, 6);
    }

    #[test]
    fn uses_backup_area_toc() {
        let mut image = fixture(false, false);
        let area1 = 520 * SACD_LSN_SIZE;
        image[area1..area1 + 8].copy_from_slice(b"BAD-TOC!");
        let backup = 525 * SACD_LSN_SIZE;
        write_area_toc(&mut image, 525, b"TWOCHTOC", 2, 3, 2, 600, 604, false);
        write_master(&mut image, 520, 525, 3, 0, 0, 0);
        let disc = SacdDisc::read_from(&mut Cursor::new(image.clone())).unwrap();
        assert_eq!(disc.stereo_area().unwrap().tracks[0].start_lsn, 600);
        assert_eq!(&image[backup..backup + 8], b"TWOCHTOC");
    }

    #[test]
    fn dst_area_reports_preserved_dst_format() {
        let image = fixture(false, true);
        let disc = SacdDisc::read_from(&mut Cursor::new(image)).unwrap();
        let area = disc.stereo_area().unwrap();
        assert_eq!(area.frame_format, SacdFrameFormat::Dst);
        assert_eq!(area.frame_format.codec_id(), CODEC_ID_DST);
    }

    #[test]
    fn packet_reader_extracts_uncompressed_frame() {
        let image = fixture(false, false);
        let disc = SacdDisc::read_from(&mut Cursor::new(image.clone())).unwrap();
        let mut packets = SacdPacketReader::new(Cursor::new(image), disc, 0, 0).unwrap();
        let packet = packets.next_packet().unwrap().unwrap();
        assert_eq!(packet.channel_count, 2);
        assert_eq!(packet.frame_format, SacdFrameFormat::Dsd3In16);
        assert_eq!(packet.data.len(), 2 * FRAME_SIZE_64);
        assert!(packets.next_packet().unwrap().is_none());
    }

    #[test]
    fn symphonia_probe_reads_first_packet() {
        let image = fixture(false, false);
        let source = TestSource::new(image);
        let mss = MediaSourceStream::new(Box::new(source), MediaSourceStreamOptions::default());
        let mut probe = Probe::new();
        register_all(&mut probe);
        let mut hint = Hint::new();
        hint.with_extension("iso");
        let mut format = probe
            .probe(&hint, mss, Default::default(), Default::default())
            .unwrap();
        assert_eq!(format.tracks().len(), 1);
        let track_id = format.tracks()[0].id;
        let params = match &format.tracks()[0].codec_params {
            Some(CodecParameters::Audio(params)) => params,
            _ => panic!("missing audio params"),
        };
        assert_eq!(params.codec, CODEC_ID_DSD);
        assert_eq!(params.channels, Some(Channels::Discrete(2u16)));
        let packet = format.next_packet().unwrap().unwrap();
        assert_eq!(packet.track_id, track_id);
        assert_eq!(packet.dur, Duration::new(1));
        assert_eq!(packet.data.len(), 2 * FRAME_SIZE_64);
    }

    #[test]
    fn symphonia_decoder_converts_uncompressed_sacd_packet_to_pcm() {
        let image = fixture(false, false);
        let source = TestSource::new(image);
        let mss = MediaSourceStream::new(Box::new(source), MediaSourceStreamOptions::default());
        let mut probe = Probe::new();
        register_all(&mut probe);
        let mut hint = Hint::new();
        hint.with_extension("iso");
        let mut format = probe
            .probe(&hint, mss, Default::default(), Default::default())
            .unwrap();
        let params = match &format.tracks()[0].codec_params {
            Some(CodecParameters::Audio(params)) => params.clone(),
            _ => panic!("missing audio params"),
        };
        let packet = format.next_packet().unwrap().unwrap();

        let mut registry = CodecRegistry::new();
        register_decoders(&mut registry);
        let mut decoder = registry
            .make_audio_decoder(&params, &AudioDecoderOptions::default())
            .unwrap();
        let decoded = decoder.decode_ref(&packet.as_packet_ref()).unwrap();
        let GenericAudioBufferRef::F32(buffer) = decoded else {
            panic!("expected decoded f32 PCM");
        };
        assert_eq!(buffer.spec().rate(), 176_400);
        assert_eq!(buffer.spec().channels().count(), 2);
        assert_eq!(buffer.frames(), 2352);
    }

    #[test]
    fn dsf_writer_reports_dst_decode_errors() {
        let image = fixture(false, true);
        let disc = SacdDisc::read_from(&mut Cursor::new(image.clone())).unwrap();
        let packets = SacdPacketReader::new(Cursor::new(image), disc, 0, 0).unwrap();
        let mut output = Vec::new();
        let err = write_track_as_dsf(&mut output, packets).unwrap_err();
        assert!(matches!(err, SacdError::DstDecode(_)));
    }

    #[test]
    fn dsf_writer_emits_header_for_uncompressed_track() {
        let image = fixture(false, false);
        let disc = SacdDisc::read_from(&mut Cursor::new(image.clone())).unwrap();
        let packets = SacdPacketReader::new(Cursor::new(image), disc, 0, 0).unwrap();
        let mut output = Vec::new();
        let frames = write_track_as_dsf(&mut output, packets).unwrap();
        assert_eq!(frames, 1);
        assert_eq!(&output[0..4], b"DSD ");
        assert_eq!(&output[28..32], b"fmt ");
        assert_eq!(&output[80..84], b"data");
        assert_eq!(u32::from_le_bytes(output[52..56].try_into().unwrap()), 2);
    }

    #[derive(Debug)]
    struct TestSource {
        inner: Cursor<Vec<u8>>,
    }

    impl TestSource {
        fn new(data: Vec<u8>) -> Self {
            Self {
                inner: Cursor::new(data),
            }
        }
    }

    impl Read for TestSource {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.inner.read(buf)
        }
    }

    impl Seek for TestSource {
        fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
            self.inner.seek(pos)
        }
    }

    impl MediaSource for TestSource {
        fn is_seekable(&self) -> bool {
            true
        }

        fn byte_len(&self) -> Option<u64> {
            Some(self.inner.get_ref().len() as u64)
        }
    }

    fn fixture(include_multichannel: bool, dst: bool) -> Vec<u8> {
        let max_lsn = if include_multichannel { 640 } else { 610 };
        let mut image = vec![0u8; (max_lsn + 1) * SACD_LSN_SIZE];
        write_master(
            &mut image,
            520,
            0,
            3,
            if include_multichannel { 540 } else { 0 },
            0,
            if include_multichannel { 3 } else { 0 },
        );
        write_master_text(&mut image);
        write_area_toc(
            &mut image,
            520,
            b"TWOCHTOC",
            2,
            if dst { 0 } else { 3 },
            2,
            600,
            604,
            dst,
        );
        if dst {
            write_dst_sector(&mut image, 600, 2);
        } else {
            write_dsd_frame_sectors(&mut image, 600, 2);
        }
        if include_multichannel {
            write_area_toc(&mut image, 540, b"MULCHTOC", 6, 3, 6, 610, 614, false);
            write_dsd_frame_sectors(&mut image, 610, 6);
        }
        image
    }

    fn write_master(
        image: &mut [u8],
        area1_start: u32,
        area1_backup: u32,
        area1_size: u16,
        area2_start: u32,
        area2_backup: u32,
        area2_size: u16,
    ) {
        let off = START_OF_MASTER_TOC as usize * SACD_LSN_SIZE;
        image[off..off + 8].copy_from_slice(b"SACDMTOC");
        image[off + 8] = 1;
        image[off + 9] = 20;
        put_be_u32(image, off + 64, area1_start);
        put_be_u32(image, off + 68, area1_backup);
        put_be_u32(image, off + 72, area2_start);
        put_be_u32(image, off + 76, area2_backup);
        put_be_u16(image, off + 84, area1_size);
        put_be_u16(image, off + 86, area2_size);
    }

    fn write_master_text(image: &mut [u8]) {
        let off = (START_OF_MASTER_TOC as usize + 1) * SACD_LSN_SIZE;
        image[off..off + 8].copy_from_slice(b"SACDText");
        put_be_u16(image, off + 16, 64);
        put_be_u16(image, off + 18, 80);
        image[off + 64..off + 74].copy_from_slice(b"Test Album");
        image[off + 80..off + 91].copy_from_slice(b"Test Artist");
    }

    fn write_area_toc(
        image: &mut [u8],
        lsn: u32,
        id: &[u8; 8],
        channels: u8,
        frame_format: u8,
        duration_frames: u8,
        track_start: u32,
        track_end: u32,
        dst: bool,
    ) {
        let off = lsn as usize * SACD_LSN_SIZE;
        image[off..off + 8].copy_from_slice(id);
        image[off + 8] = 1;
        image[off + 9] = 20;
        put_be_u16(image, off + 10, 3);
        image[off + 20] = 4;
        image[off + 21] = frame_format;
        image[off + 32] = channels;
        image[off + 33] = if channels == 6 { 4 } else { 0 };
        image[off + 66] = duration_frames;
        image[off + 69] = 1;
        put_be_u32(image, off + 72, track_start);
        put_be_u32(image, off + 76, track_end);

        let trl1 = (lsn as usize + 1) * SACD_LSN_SIZE;
        image[trl1..trl1 + 8].copy_from_slice(b"SACDTRL1");
        put_be_u32(image, trl1 + 8, track_start);
        put_be_u32(image, trl1 + 8 + 255 * 4, track_end - track_start + 1);

        let trl2 = (lsn as usize + 2) * SACD_LSN_SIZE;
        image[trl2..trl2 + 8].copy_from_slice(b"SACDTRL2");
        image[trl2 + 8 + 255 * 4 + 2] = if dst { 1 } else { duration_frames };
    }

    fn write_dsd_frame_sectors(image: &mut [u8], start_lsn: usize, channels: u8) {
        let total = usize::from(channels) * FRAME_SIZE_64;
        let mut remaining = total;
        let mut written = 0usize;
        let mut sector_idx = 0usize;
        while remaining > 0 {
            let off = (start_lsn + sector_idx) * SACD_LSN_SIZE;
            let len = remaining.min(2000);
            image[off] = (1 << 5) | if sector_idx == 0 { 1 << 2 } else { 0 };
            let packet_info = ((2u16 << 3) << 8) | len as u16;
            image[off + 1] = if sector_idx == 0 {
                0x80 | ((packet_info >> 8) as u8 & 0x7f)
            } else {
                (packet_info >> 8) as u8 & 0x7f
            };
            image[off + 2] = packet_info as u8;
            let data_off = if sector_idx == 0 {
                image[off + 3] = 0;
                image[off + 4] = 0;
                image[off + 5] = 0;
                off + 6
            } else {
                off + 3
            };
            for byte in &mut image[data_off..data_off + len] {
                *byte = (written % 251) as u8;
                written += 1;
            }
            remaining -= len;
            sector_idx += 1;
        }
    }

    fn write_dst_sector(image: &mut [u8], lsn: usize, channels: u8) {
        let off = lsn * SACD_LSN_SIZE;
        let len = 12usize;
        image[off] = 1 | (1 << 2) | (1 << 5);
        let packet_info = ((2u16 << 3) << 8) | len as u16;
        image[off + 1] = 0x80 | ((packet_info >> 8) as u8 & 0x7f);
        image[off + 2] = packet_info as u8;
        image[off + 3] = 0;
        image[off + 4] = 0;
        image[off + 5] = 0;
        image[off + 6] = (1 << 2) | if channels == 5 { 1 } else { 0 };
        image[off + 7..off + 7 + len].copy_from_slice(b"DST-FRAME-01");
    }

    fn put_be_u16(data: &mut [u8], offset: usize, value: u16) {
        data[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
    }

    fn put_be_u32(data: &mut [u8], offset: usize, value: u32) {
        data[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }
}

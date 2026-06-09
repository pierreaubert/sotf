use dsf_meta::DSF_SAMPLE_DATA_OFFSET;
use id3::Tag;
use log::warn;
use std::{
    fs::File,
    path::{Path, PathBuf},
};

// Strongly typed container format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DsdFileFormat {
    Dsdiff,
    Dsf,
    Raw,
}

pub trait FormatExtensions {
    fn is_container(&self) -> bool;
}
impl FormatExtensions for DsdFileFormat {
    fn is_container(&self) -> bool {
        match self {
            DsdFileFormat::Dsf | DsdFileFormat::Dsdiff => true,
            DsdFileFormat::Raw => false,
        }
    }
}

impl From<&PathBuf> for DsdFileFormat {
    fn from(path: &PathBuf) -> Self {
        if let Some(ext) = path.extension() {
            match ext.to_ascii_lowercase().to_string_lossy().as_ref() {
                "dsf" => DsdFileFormat::Dsf,
                "dff" => DsdFileFormat::Dsdiff,
                _ => DsdFileFormat::Raw,
            }
        } else {
            DsdFileFormat::Raw
        }
    }
}

pub const DSD_64_RATE: u32 = 2822400;
pub const DFF_BLOCK_SIZE: u32 = 1;
pub const DSF_BLOCK_SIZE: u32 = 4096;

pub struct DsdFile {
    audio_length: u64,
    audio_pos: u64,
    channel_count: Option<usize>,
    is_lsb: Option<bool>,
    block_size: Option<u32>,
    sample_rate: Option<u32>,
    container_format: DsdFileFormat,
    file: File,
    tag: Option<Tag>,
}

impl DsdFile {
    pub fn audio_length(&self) -> u64 {
        self.audio_length
    }
    pub fn tag(&self) -> Option<&Tag> {
        self.tag.as_ref()
    }
    pub fn file(&self) -> &File {
        &self.file
    }
    pub fn audio_pos(&self) -> u64 {
        self.audio_pos
    }
    pub fn channel_count(&self) -> Option<usize> {
        self.channel_count
    }
    pub fn is_lsb(&self) -> Option<bool> {
        self.is_lsb
    }
    pub fn block_size(&self) -> Option<u32> {
        self.block_size
    }
    pub fn sample_rate(&self) -> Option<u32> {
        self.sample_rate
    }
    pub fn container_format(&self) -> DsdFileFormat {
        self.container_format
    }

    pub fn new(
        path: &PathBuf,
        file_format: DsdFileFormat,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if file_format == DsdFileFormat::Dsf {
            use dsf_meta::DsfFile;
            let file_path = Path::new(&path);
            let dsf_file = DsfFile::open(file_path)?;
            if let Some(e) = dsf_file.tag_read_err() {
                warn!(
                    "Attempted read of ID3 tag failed. Partial read attempted: {}",
                    e
                );
            }
            let file = dsf_file.file().try_clone()?;
            Ok(Self {
                sample_rate: Some(
                    dsf_file.fmt_chunk().sampling_frequency(),
                ),
                container_format: DsdFileFormat::Dsf,
                channel_count: Some(
                    dsf_file.fmt_chunk().channel_num() as usize
                ),
                is_lsb: Some(dsf_file.fmt_chunk().bits_per_sample() == 1),
                block_size: Some(DSF_BLOCK_SIZE), // Should always be this value for DSF
                audio_length: dsf_file.fmt_chunk().sample_count() / 8
                    * dsf_file.fmt_chunk().channel_num() as u64,
                audio_pos: DSF_SAMPLE_DATA_OFFSET,
                file,
                tag: dsf_file.id3_tag().clone(),
            })
        } else if file_format == DsdFileFormat::Dsdiff {
            use dff_meta::DffFile;
            use dff_meta::model::*;
            let file_path = Path::new(&path);
            let dff_file = match DffFile::open(file_path) {
                Ok(dff) => dff,
                Err(Error::Id3Error(e, dff_file)) => {
                    warn!(
                        "Attempted read of ID3 tag failed. Partial read attempted: {}",
                        e
                    );
                    dff_file
                }
                Err(e) => {
                    return Err(e.into());
                }
            };
            let file = dff_file.file().try_clone()?;
            Ok(Self {
                sample_rate: Some(dff_file.get_sample_rate()?),
                container_format: DsdFileFormat::Dsdiff,
                channel_count: Some(dff_file.get_num_channels()?),
                is_lsb: Some(false),
                block_size: Some(DFF_BLOCK_SIZE), // Should always be 1 for DFF
                audio_length: dff_file.get_audio_length(),
                audio_pos: dff_file.get_dsd_data_offset(),
                file,
                tag: dff_file.id3_tag().clone(),
            })
        } else if file_format == DsdFileFormat::Raw {
            let Ok(meta) = std::fs::metadata(path) else {
                return Err("Failed to read input file metadata".into());
            };
            Ok(Self {
                sample_rate: None,
                container_format: DsdFileFormat::Raw,
                channel_count: None,
                is_lsb: None,
                block_size: None,
                audio_length: meta.len(),
                audio_pos: 0,
                file: File::open(path)?,
                tag: None,
            })
        } else {
            Err("Unsupported file type; only dsf, dff, and raw dsd files are supported"
                .into())
        }
    }
}

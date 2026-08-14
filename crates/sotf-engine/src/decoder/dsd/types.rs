#[cfg(test)]
pub(super) struct ParsedDsf {
    pub(super) sample_rate: u32,
    pub(super) channels: u16,
    pub(super) sample_count: u64,
    pub(super) block_size_per_channel: usize,
    pub(super) lsb_first: bool,
    pub(super) data: Vec<u8>,
}

#[cfg(test)]
pub(super) struct ParsedDff {
    pub(super) sample_rate: u32,
    pub(super) channels: u16,
    pub(super) sample_count: u64,
    pub(super) data: Vec<u8>,
}

pub(super) struct DsfFileMetadata {
    pub(super) sample_rate: u32,
    pub(super) channels: u16,
    pub(super) sample_count: u64,
    pub(super) block_size_per_channel: usize,
    pub(super) lsb_first: bool,
    pub(super) data_offset: u64,
    pub(super) data_len: u64,
}

pub(super) struct DffFileMetadata {
    pub(super) sample_rate: u32,
    pub(super) channels: u16,
    pub(super) sample_count: u64,
    pub(super) data_offset: u64,
    pub(super) data_len: u64,
}

pub(super) struct DffSoundProperties {
    pub(super) sample_rate: Option<u32>,
    pub(super) channels: Option<u16>,
    pub(super) compression: Option<[u8; 4]>,
}

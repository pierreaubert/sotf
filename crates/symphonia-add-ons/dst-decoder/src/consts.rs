pub const DSD64_SAMPLE_RATE: u32 = 2_822_400;
pub const DSD128_SAMPLE_RATE: u32 = 5_644_800;
pub const DSD256_SAMPLE_RATE: u32 = 11_289_600;

pub const RESOL: i64 = 8;

pub const SIZE_CODEDPREDORDER: usize = 7;
pub const SIZE_PREDCOEF: usize = 9;

pub const AC_BITS: usize = 8;
pub const AC_PROBS: i32 = 1 << AC_BITS;
pub const AC_HISBITS: usize = 6;
pub const AC_HISMAX: usize = 1 << AC_HISBITS;
pub const AC_QSTEP: usize = SIZE_PREDCOEF - AC_HISBITS; // 3

pub const NROFFRICEMETHODS: usize = 3;
pub const NROFPRICEMETHODS: usize = 3;
pub const MAXCPREDORDER: usize = 3;
pub const SIZE_RICEMETHOD: usize = 2;
pub const SIZE_RICEM: usize = 3;

pub const MAXNROF_FSEGS: i32 = 4;
pub const MAXNROF_PSEGS: i32 = 8;
pub const MIN_FSEG_LEN: i32 = 1024;
pub const MIN_PSEG_LEN: i32 = 32;

pub const MAX_CHANNELS: usize = 6;
pub const MAXNROF_SEGS: usize = 8;

// AC encoder register sizes
pub const PBITS: usize = AC_BITS;
pub const NBITS: usize = 4;
pub const ABITS: usize = PBITS + NBITS; // 12
pub const ONE: u32 = 1 << ABITS;
pub const HALF: u32 = 1 << (ABITS - 1);

/*

Copyright 2009, 2011 Sebastian Gesemann. All rights reserved.

Redistribution and use in source and binary forms, with or without modification, are
permitted provided that the following conditions are met:

   1. Redistributions of source code must retain the above copyright notice, this list of
      conditions and the following disclaimer.

   2. Redistributions in binary form must reproduce the above copyright notice, this list
      of conditions and the following disclaimer in the documentation and/or other materials
      provided with the distribution.

THIS SOFTWARE IS PROVIDED BY SEBASTIAN GESEMANN ''AS IS'' AND ANY EXPRESS OR IMPLIED
WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND
FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL SEBASTIAN GESEMANN OR
CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON
ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF
ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

The views and conclusions contained in the software and documentation are those of the
authors and should not be interpreted as representing official policies, either expressed
or implied, of Sebastian Gesemann.

 */

// ============================================================================
// BytePrecalcDecimator
// Minimal, safe, idiomatic Rust adaptation of the dsd2pcm precalc() approach,
// used ONLY for dense bipolar DSD bitstreams with straight integer decimation
// (no zero stuffing). You precompute 256-entry tables for 8-bit windows of the
// HALF (right) filter taps. At each decimated output boundary we sum mirrored
// table contributions, reproducing the full linear‑phase FIR result with
// far fewer operations (O(numTables) vs O(taps)).
//
// Assumptions:
// - Filter specified by right-half taps (second_half_taps), even full length = 2 * len.
// - decim is an integer multiple of 8 (16, 32, 64, etc).
// - Produces one output per 'decim' input bits (decim/8 bytes).
// ============================================================================

use log::trace;

use crate::{
    FilterType,
    filters::{
        HTAPS_16TO1_XLD, HTAPS_32TO1, HTAPS_D2P, HTAPS_DDR_8TO1_EQ,
        HTAPS_DDR_16TO1_CHEB, HTAPS_DDR_16TO1_EQ, HTAPS_DDR_32TO1_CHEB,
        HTAPS_DDR_32TO1_EQ, HTAPS_DDR_64TO1_CHEB, HTAPS_DDR_64TO1_EQ,
        HTAPS_DSD64_8TO1_EQ, HTAPS_DSD64_16TO1_EQ, HTAPS_DSD64_32TO1_EQ,
        HTAPS_DSD256_8TO1_EQ, HTAPS_DSD256_16TO1_EQ,
        HTAPS_DSD256_32TO1_EQ, HTAPS_DSD256_64TO1_EQ,
        HTAPS_DSD256_128TO1_EQ, HTAPS_DSD512_64TO1_EQ, HTAPS_XLD,
    },
};

pub struct BytePrecalcDecimator {
    // Precomputed tables: tables[i][byte] gives partial sum for segment i
    tables: Vec<Box<[f64]>>,
    num_tables: usize,
    bytes_per_out: u32,
    fifo: Vec<u8>,
    fifo_pos: usize,
    // Cached for mirror addressing
    table_span: usize, // num_tables * 2 - 1
}

impl BytePrecalcDecimator {
    /// Build from right-half taps (second_half_taps) and integer decimation factor.
    pub fn new(second_half_taps: &[f64], decim: u32) -> Option<Self> {
        if decim % 8 != 0 {
            return None;
        } // requires byte alignment
        let half = second_half_taps.len();
        if half == 0 {
            return None;
        }
        // Number of 8-bit windows covering half the filter
        let num_tables = (half + 7) / 8;

        let dec = Self {
            tables: (0..num_tables)
                // Reverse to align with C's tableIdx = numTables - 1 - t
                .rev()
                .map(|t| t * 8)
                .map(|t| {
                    // Table index is reversed order (ctx->numTables-1 - t) in C; we can mimic
                    // by pushing and later indexing appropriately. Simpler: store in reverse now.
                    (0..256i16)
                        .map(|dsd_seq| {
                            (0..half.saturating_sub(t).min(8)).fold(
                                0.0f64,
                                |acc, bit|
                                    // Map 0 -> -1, 1 -> +1 and multiply/accumulate
                                    acc + (((dsd_seq >> bit) & 1) * 2 - 1)
                                        as f64
                                        * second_half_taps[t + bit],
                            )
                        })
                        .collect::<Vec<f64>>()
                        .into_boxed_slice()
                })
                .collect(),
            num_tables,
            bytes_per_out: decim / 8,
            fifo: vec![0x69u8; (num_tables * 2 + 8).next_power_of_two()], // simple ring
            fifo_pos: 0,
            table_span: num_tables * 2 - 1,
        };
        trace!(
            "BytePrecalcDecimator initialized with {} tables, {} bytes per output, fifo size {}",
            dec.num_tables,
            dec.bytes_per_out,
            dec.fifo.len()
        );
        Some(dec)
    }

    /// Feed a block of DSD bytes; produce decimated PCM outputs.
    /// Returns number of PCM samples written to `out`.
    pub fn process_bytes(
        &mut self,
        bytes: &[u8],
        out: &mut [f64],
    ) -> usize {
        if self.num_tables == 0 || self.bytes_per_out == 0 {
            return 0;
        }
        let mask = self.fifo.len() - 1; // fifo len is power-of-two
        let mut byte_count_in_frame = 0u32;
        let mut produced = 0usize;

        for &b in bytes {
            // Push newest byte
            self.fifo[self.fifo_pos & mask] = b;
            self.fifo_pos = (self.fifo_pos + 1) & mask;
            byte_count_in_frame += 1;

            if byte_count_in_frame == self.bytes_per_out {
                byte_count_in_frame = 0;

                if produced < out.len() {
                    out[produced] =
                        (0..self.num_tables).fold(0.0f64, |acc, i| {
                            // Recent window i
                            let idx1 =
                                self.fifo_pos.wrapping_sub(1 + i) & mask;
                            // Mirrored window
                            let idx2 = self
                                .fifo_pos
                                .wrapping_sub(1 + (self.table_span - i))
                                & mask;
                            let byte1 = self.fifo[idx1];
                            let byte2 = self.fifo[idx2];
                            // Table i corresponds to tables[i]; mirrored byte must be bit-reversed
                            acc + self.tables[i][byte1 as usize]
                                + self.tables[i]
                                    [byte2.reverse_bits() as usize]
                        });
                    produced += 1;
                }
                if produced == out.len() {
                    break;
                }
            }
        }
        produced
    }

    /// Feed one channel from an interleaved DSD byte stream without first
    /// deinterleaving it into a temporary channel buffer.
    pub fn process_interleaved_bytes(
        &mut self,
        bytes: &[u8],
        channel: usize,
        channels: usize,
        reverse_bits: bool,
        out: &mut [f64],
    ) -> usize {
        if self.num_tables == 0 || self.bytes_per_out == 0 || channels == 0
        {
            return 0;
        }
        let mask = self.fifo.len() - 1;
        let mut byte_count_in_frame = 0u32;
        let mut produced = 0usize;

        let mut pos = channel;
        while pos < bytes.len() {
            let mut b = bytes[pos];
            if reverse_bits {
                b = b.reverse_bits();
            }
            self.fifo[self.fifo_pos & mask] = b;
            self.fifo_pos = (self.fifo_pos + 1) & mask;
            byte_count_in_frame += 1;

            if byte_count_in_frame == self.bytes_per_out {
                byte_count_in_frame = 0;

                if produced < out.len() {
                    out[produced] =
                        (0..self.num_tables).fold(0.0f64, |acc, i| {
                            let idx1 =
                                self.fifo_pos.wrapping_sub(1 + i) & mask;
                            let idx2 = self
                                .fifo_pos
                                .wrapping_sub(1 + (self.table_span - i))
                                & mask;
                            let byte1 = self.fifo[idx1];
                            let byte2 = self.fifo[idx2];
                            acc + self.tables[i][byte1 as usize]
                                + self.tables[i]
                                    [byte2.reverse_bits() as usize]
                        });
                    produced += 1;
                }
                if produced == out.len() {
                    break;
                }
            }

            pos += channels;
        }

        produced
    }
}

// Central mapping from (filter type, dsd_rate, decimation ratio) to half-tap tables.
// Returns Some(&half_taps) if we can drive a single-stage BytePrecalcDecimator; otherwise None.
pub fn select_precalc_taps(
    filt_type: FilterType,
    dsd_rate: i32,
    decim_ratio: i32,
) -> Option<&'static [f64]> {
    match decim_ratio {
        // 128:1 (DSD256 -> 88.2 kHz), Equiripple only
        128 => {
            if filt_type == FilterType::Equiripple && dsd_rate == 4 {
                Some(&HTAPS_DSD256_128TO1_EQ)
            } else {
                None
            }
        }
        // 8:1 (DSD64/128 only) – 'D' uses HTAPS_D2P, 'X' uses HTAPS_XLD, 'E' uses new equiripple, others fallback
        8 => {
            if dsd_rate == 1 {
                match filt_type {
                    FilterType::Dsd2Pcm => Some(&HTAPS_D2P),
                    FilterType::XLD => Some(&HTAPS_XLD),
                    FilterType::Equiripple => Some(&HTAPS_DSD64_8TO1_EQ),
                    _ => None,
                }
            } else if dsd_rate == 2 {
                match filt_type {
                    FilterType::Equiripple => Some(&HTAPS_DDR_8TO1_EQ),
                    _ => None,
                }
            } else if dsd_rate == 4 {
                match filt_type {
                    FilterType::Equiripple => Some(&HTAPS_DSD256_8TO1_EQ),
                    _ => None,
                }
            } else {
                None
            }
        }
        // 16:1
        16 => match filt_type {
            FilterType::XLD => Some(&HTAPS_16TO1_XLD),
            // E – equiripple: now support DSD64 with dedicated table, DSD128 with DDR table
            FilterType::Equiripple => {
                if dsd_rate == 1 {
                    Some(&HTAPS_DSD64_16TO1_EQ)
                } else if dsd_rate == 2 {
                    Some(&HTAPS_DDR_16TO1_EQ)
                } else if dsd_rate == 4 {
                    // New dedicated DSD256 16:1 equiripple half taps
                    Some(&HTAPS_DSD256_16TO1_EQ)
                } else {
                    None
                }
            }
            // C – Chebyshev only provided for DSD128; fallback None for others
            FilterType::Chebyshev => {
                if dsd_rate == 2 {
                    Some(&HTAPS_DDR_16TO1_CHEB)
                } else {
                    None
                }
            }
            _ => None,
        },
        // 32:1
        32 => match filt_type {
            FilterType::XLD => Some(&HTAPS_32TO1),
            FilterType::Equiripple => {
                if dsd_rate == 1 {
                    Some(&HTAPS_DSD64_32TO1_EQ)
                } else if dsd_rate == 4 {
                    // New dedicated DSD256 32:1 equiripple half taps
                    Some(&HTAPS_DSD256_32TO1_EQ)
                } else {
                    Some(&HTAPS_DDR_32TO1_EQ)
                }
            }
            FilterType::Chebyshev => Some(&HTAPS_DDR_32TO1_CHEB),
            _ => None,
        },
        // 64:1
        64 => match filt_type {
            FilterType::Equiripple => {
                if dsd_rate == 4 {
                    Some(&HTAPS_DSD256_64TO1_EQ)
                } else if dsd_rate == 8 {
                    Some(&HTAPS_DSD512_64TO1_EQ)
                } else {
                    Some(&HTAPS_DDR_64TO1_EQ)
                }
            }
            FilterType::Chebyshev => Some(&HTAPS_DDR_64TO1_CHEB),
            _ => None,
        },
        _ => None,
    }
}

/*
 Copyright (c) 2023 clone206

 This file is part of rdsd2pcm

 rdsd2pcm is free software: you can redistribute it and/or modify it
 under the terms of the GNU General Public License as published by the
 Free Software Foundation, either version 3 of the License, or
 (at your option) any later version.

 rdsd2pcm is distributed in the hope that it will be useful, but
 WITHOUT ANY WARRANTY; without even the implied warranty of
 MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 GNU General Public License for more details.
 You should have received a copy of the GNU General Public License
 along with rdsd2pcm. If not, see <https://www.gnu.org/licenses/>.
*/

use crate::filters_lm::HTAPS_1_34MHZ_7TO1_EQ;
use crate::filters_lm::HTAPS_2_68MHZ_7TO1_EQ;
use crate::filters_lm::HTAPS_2_68MHZ_14TO1_EQ;
use crate::filters_lm::HTAPS_2_68MHZ_28TO1_EQ;
use crate::filters_lm::HTAPS_2MHZ_7TO1_EQ;
use crate::filters_lm::HTAPS_4MHZ_7TO1_EQ;
use crate::filters_lm::HTAPS_288K_3TO1_EQ;
use crate::filters_lm::HTAPS_576K_3TO1_EQ;
use crate::filters_lm::HTAPS_DDRX5_7TO_1_EQ;
use crate::filters_lm::HTAPS_DDRX5_14TO1_EQ; // ADD first-stage half taps (5× up, 14:1 down)
use crate::filters_lm::HTAPS_DDRX10_21TO1_EQ;
use crate::filters_lm::HTAPS_DSDX5_7TO1_EQ;
use crate::filters_lm::HTAPS_DSDX10_21TO1_EQ;
use dsd_reader::dsd_file::DSD_64_RATE;
use log::debug;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage1Mode {
    SlotSim,
    Accum,
}
// Global LUT cache for Stage1, shared across instances/channels.
// Keyed by (L, len(right_half), hash(contents of right_half)) to avoid rebuilding.
type Stage1LutTable = Vec<Vec<[f64; 256]>>;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Stage1LutKey {
    l: u32,
    n: u32,
    hash: u64,
}

static STAGE1_LUT_CACHE: OnceLock<
    Mutex<HashMap<Stage1LutKey, Arc<Stage1LutTable>>>,
> = OnceLock::new();

// One-time config diagnostics guards
static ST1_DIAG_ONCE: OnceLock<()> = OnceLock::new();

// ------------------------------------------------------------------------------------
// Tuning knobs (via env) for chunk sizing in Stage1
// Sensible baked-in defaults (used when env vars are unset):
//   Stage1: TARGET≈96 groups, aligned to M (ALIGN=M)
// Stage1 env vars (override defaults):
//   DSD2DXD_STAGE1_CHUNK_MULT   (usize, default 0)   -> if >0, chunk = M * MULT
//   DSD2DXD_STAGE1_CHUNK_TARGET (usize, default 128) -> if >0, round up to multiple of M near TARGET
//   DSD2DXD_STAGE1_CHUNK_ALIGN  (usize, default 0)   -> if 0, ALIGN=M; else round chunk down to multiple of ALIGN

static ST1_MULT: OnceLock<usize> = OnceLock::new();
static ST1_ALIGN: OnceLock<usize> = OnceLock::new();
static ST1_TARGET: OnceLock<usize> = OnceLock::new();

// ====================================================================================
// Generalized L/M resampler
// ====================================================================================
pub struct LMResampler {
    // Unified Stage1 polyphase
    stage1_poly: Option<Stage1Poly>,
    stage2_decim: Option<DecimFIRSym>,
    stage3_decim: Option<DecimFIRSym>,
    // Reusable buffer for batching Stage1 outputs before pushing to s1 ring
    s1_scratch: Vec<f64>,
    s2_scratch: Vec<f64>,
}
impl LMResampler {
    pub fn new(
        l: u32,
        m: i32,
        out_rate: u32,
    ) -> Result<Self, Box<dyn Error>> {
        match m {
            588 => {
                // DSD256 -> 96k two‑stage path: (×5 -> /21) => ~10.752 MHz -> /28 => 96k
                if out_rate == 96_000 && l == 5 {
                    debug!(
                        "Two-stage DSD256->96k: (×{} -> /21) -> /28 [DDRX10_21TO1 + 2.68MHz 28:1]",
                        l
                    );
                    return Ok(Self {
                        stage1_poly: Some(Stage1Poly::new(
                            &HTAPS_DDRX10_21TO1_EQ[..],
                            l,
                            21,
                        )),
                        stage2_decim: Some(DecimFIRSym::new_from_half(
                            &HTAPS_2_68MHZ_28TO1_EQ[..],
                            28,
                        )),
                        stage3_decim: None,
                        s2_scratch: Vec::new(),
                        s1_scratch: Vec::new(),
                    });
                }
                return Err(
                    "Unsupported 588: must be out_rate=96k and L=5".into(),
                );
            }
            294 => {
                // DSD256 -> 192k two‑stage path: (×5 -> /21) => 2.688 MHz -> /14 => 192k
                if out_rate == 192_000 && l == 5 {
                    debug!(
                        "Two-stage DSD256->192k: (×{} -> /21) -> /14 [DDRX10_21TO1 + 2.68MHz 14:1]",
                        l
                    );
                    return Ok(Self {
                        stage1_poly: Some(Stage1Poly::new(
                            &HTAPS_DDRX10_21TO1_EQ[..],
                            l,
                            21,
                        )),
                        stage2_decim: Some(DecimFIRSym::new_from_half(
                            &HTAPS_2_68MHZ_14TO1_EQ[..],
                            14,
                        )),
                        stage3_decim: None,
                        s2_scratch: Vec::new(),
                        s1_scratch: Vec::new(),
                    });
                }
                // Original cascade Stage1 definitions
                debug!(
                    "Equiripple L/M path: L={} M=294 — (×L -> /14 -> /7 -> /3) [Stage2/3 direct].",
                    l
                );
                return Ok(Self {
                    // Stage 2 (decimator by 7)
                    stage2_decim: Some(DecimFIRSym::new_from_half(
                        &HTAPS_2MHZ_7TO1_EQ,
                        7,
                    )),
                    // Stage 3 (decimator by 3)
                    stage3_decim: Some(DecimFIRSym::new_from_half(
                        &HTAPS_288K_3TO1_EQ,
                        3,
                    )),
                    stage1_poly: Some(Stage1Poly::new(
                        &HTAPS_DDRX5_14TO1_EQ,
                        l,
                        14,
                    )),
                    s2_scratch: Vec::new(),
                    s1_scratch: Vec::new(),
                });
            }
            147 => {
                // DSD128 -> 384k two‑stage path (×L -> /21) -> /7
                if out_rate == 384_000 {
                    debug!(
                        "Two-stage L={}/M=147 path enabled: (×L -> /21 (poly) -> /7) => 384k",
                        l
                    );
                    return Ok(Self {
                        stage1_poly: Some(Stage1Poly::new(
                            &HTAPS_DDRX10_21TO1_EQ[..],
                            l,
                            21,
                        )),
                        stage2_decim: Some(DecimFIRSym::new_from_half(
                            &HTAPS_2_68MHZ_7TO1_EQ[..],
                            7,
                        )),
                        stage3_decim: None,
                        s2_scratch: Vec::new(),
                        s1_scratch: Vec::new(),
                    });
                }

                if out_rate == 192_000 {
                    if l == 5 {
                        // DSD128 -> 192k three-stage path
                        debug!(
                            "Three-stage L=5/M=147 path enabled: (×5 -> /7 (poly) -> /7 -> /3) => 192k"
                        );
                        return Ok(Self {
                            stage1_poly: Some(Stage1Poly::new(
                                &HTAPS_DDRX5_7TO_1_EQ[..],
                                l,
                                7,
                            )),
                            stage2_decim: Some(
                                DecimFIRSym::new_from_half(
                                    &HTAPS_4MHZ_7TO1_EQ[..],
                                    7,
                                ),
                            ),
                            stage3_decim: Some(
                                DecimFIRSym::new_from_half(
                                    &HTAPS_576K_3TO1_EQ[..],
                                    3,
                                ),
                            ),
                            s2_scratch: Vec::new(),
                            s1_scratch: Vec::new(),
                        });
                    } else if l == 10 {
                        // DSD64 -> 192k two-stage path (existing)
                        debug!(
                            "Two-stage L=10/M=147 path enabled: (×10 -> /21 (poly) -> /7) => 192k"
                        );
                        return Ok(Self {
                            stage1_poly: Some(Stage1Poly::new(
                                &HTAPS_DSDX10_21TO1_EQ[..],
                                l,
                                21,
                            )),
                            stage2_decim: Some(
                                DecimFIRSym::new_from_half(
                                    &HTAPS_1_34MHZ_7TO1_EQ[..],
                                    7,
                                ),
                            ),
                            stage3_decim: None,
                            s2_scratch: Vec::new(),
                            s1_scratch: Vec::new(),
                        });
                    }
                }

                debug!(
                    "Equiripple L/M path: L={} M=147 — (×L -> /7 -> /7 -> /3) [Stage2/3 direct] => 96K",
                    l
                );
                return Ok(Self {
                    stage1_poly: Some(Stage1Poly::new(
                        &HTAPS_DSDX5_7TO1_EQ[..],
                        l,
                        7,
                    )),
                    // Stage 2 (decimator by 7)
                    stage2_decim: Some(DecimFIRSym::new_from_half(
                        &HTAPS_2MHZ_7TO1_EQ[..],
                        7,
                    )),
                    // Stage 3 (decimator by 3)
                    stage3_decim: Some(DecimFIRSym::new_from_half(
                        &HTAPS_288K_3TO1_EQ[..],
                        3,
                    )),
                    s2_scratch: Vec::new(),
                    s1_scratch: Vec::new(),
                });
            }
            _ => return Err("Unsupported L/M combination".into()),
        }
    }

    // Returns number of PCM frames produced into `out`.
    #[inline(always)]
    pub fn process_bytes_lm(
        &mut self,
        bytes: &[u8],
        out: &mut [f64],
    ) -> usize {
        let s1 = match self.stage1_poly.as_mut() {
            Some(x) => x,
            None => return 0,
        };
        let p2 = match self.stage2_decim.as_mut() {
            Some(x) => x,
            None => return 0,
        };
        let mut p3_opt = self.stage3_decim.as_mut();

        let mut produced_total = 0usize;
        let mut i = 0usize; // byte cursor

        // Precompute an upper bound on stage1 outputs/byte to size chunks conservatively.
        let y1_per_byte_ub =
            ((8 * s1.l as usize) + (s1.m as usize) - 1) / (s1.m as usize);
        let y1_per_byte_ub = y1_per_byte_ub.max(1);

        // Helper to emit stage1 outputs for a byte into s1_scratch
        let push_byte =
            |b: u8, s1: &mut Stage1Poly, dst: &mut Vec<f64>| {
                for bit in 0..8 {
                    s1.push_all((b >> bit) & 1, |y1| dst.push(y1));
                }
            };

        while i < bytes.len() && produced_total < out.len() {
            // Budget frames we can still write this call
            let out_budget = out.len() - produced_total;
            // Derive upstream budgets based on whether Stage3 exists
            let (need_s2_in, need_s1_out) = if let Some(ref p3) = p3_opt {
                let s2 = out_budget.saturating_mul(p3.decim);
                (s2, s2.saturating_mul(p2.decim))
            } else {
                // Two-stage: Stage2 writes straight to out
                (out_budget, out_budget.saturating_mul(p2.decim))
            };

            // Take at most this many bytes this iteration so that, even in the worst case,
            // stage1 outputs won't exceed what downstream can consume.
            // ceil(need_s1_out / y1_per_byte_ub)
            let mut take_bytes = if need_s1_out == 0 {
                0
            } else {
                (need_s1_out + y1_per_byte_ub - 1) / y1_per_byte_ub
            };
            take_bytes = take_bytes.min(bytes.len() - i);
            if take_bytes == 0 {
                break;
            }

            // 1) Stage1: generate into s1_scratch
            self.s1_scratch.clear();
            // Reserve enough for worst case to avoid reallocation branches
            let reserve = take_bytes.saturating_mul(y1_per_byte_ub);
            if self.s1_scratch.capacity() < reserve {
                self.s1_scratch
                    .reserve(reserve - self.s1_scratch.capacity());
            }
            let mut consumed_bytes = 0usize;
            for &byte in &bytes[i..i + take_bytes] {
                push_byte(byte, s1, &mut self.s1_scratch);
                consumed_bytes += 1;
                if self.s1_scratch.len() >= need_s1_out {
                    break;
                }
            }

            // 2) Stage2: decimate; target is either s2_scratch (3-stage) or out (2-stage)
            let used_s1 =
                core::cmp::min(self.s1_scratch.len(), need_s1_out);
            if let Some(ref mut p3) = p3_opt {
                let cap_s2 = need_s2_in.max(1);
                if self.s2_scratch.len() < cap_s2 {
                    self.s2_scratch.resize(cap_s2, 0.0);
                }
                let n2 = p2.process_block(
                    &self.s1_scratch[..used_s1],
                    &mut self.s2_scratch[..cap_s2],
                );
                // 3) Stage3: decimate into out
                let n3 = p3.process_block(
                    &self.s2_scratch[..n2],
                    &mut out[produced_total..],
                );
                produced_total += n3;
            } else {
                // Two-stage: write directly to out
                let n2 = p2.process_block(
                    &self.s1_scratch[..used_s1],
                    &mut out[produced_total..],
                );
                produced_total += n2;
            }

            // Advance byte cursor by the number of bytes we actually consumed in Stage1.
            i += consumed_bytes;
        }
        produced_total
    }
}

#[derive(Debug)]
struct Stage1Poly {
    // Shared
    l: u32,
    m: u32,
    // Bit-packed ring (one bit per input sample): 1 for +1, 0 for -1
    ring_bits: Vec<u64>,
    bits_mask: usize, // same capacity as ring (power of 2) - 1
    wbits: usize,     // next write bit index
    mode: Stage1Mode,
    // Precomputed LUTs (f64 only)
    lut_f64: Option<Arc<Vec<Vec<[f64; 256]>>>>,
    // Byte-oriented ring mirroring newest-first 8-bit windows per input bit
    byte_ring: Vec<u8>,
    byte_mask: usize,
    wbyte: usize,
    rolling_byte: u8,
    // Legacy SlotSim fields removed
    delay_high: u64,
    high_index: u64,
    phase1: u32,
    // Accum fields
    acc: u32,
    phase_mod: u32,
    input_count: u64,
    input_delay: u64,
    // Common
    primed: bool,
}

impl Stage1Poly {
    fn new(right_half: &[f64], l: u32, m: u32) -> Self {
        // Full symmetric taps length when mirroring right_half as [rev(right_half), right_half]
        let n = (right_half.len() * 2) as u64;
        let delay_high = (n - 1) / 2; // (N-1)/2
        // Max taps per phase is ceil(n / l)
        let max_len = ((n as usize) + l as usize - 1) / l as usize;
        let cap = max_len.next_power_of_two().max(128);
        let input_delay = (delay_high + (l as u64 - 1)) / l as u64; // used only in Accum. Always use f64 LUTs
        let lut_f64: Option<Arc<Vec<Vec<[f64; 256]>>>> =
            Some(get_or_build_stage1_lut(right_half, l));
        // Determine maximum groups across phases to size the byte ring
        let mut groups_max = 0usize;
        if let Some(ref tbl) = lut_f64 {
            for phase in tbl.iter() {
                groups_max = groups_max.max(phase.len());
            }
        }
        let byte_cap = (groups_max + 16).next_power_of_two().max(32);
        let me = Self {
            l,
            m,
            ring_bits: vec![0u64; (cap + 63) / 64],
            bits_mask: cap - 1,
            wbits: 0,
            mode: if m == 21 {
                Stage1Mode::Accum
            } else {
                Stage1Mode::SlotSim
            },
            delay_high,
            high_index: 0,
            phase1: 0,
            lut_f64,
            byte_ring: vec![0u8; byte_cap],
            byte_mask: byte_cap - 1,
            wbyte: 0,
            rolling_byte: 0,
            acc: 0,
            phase_mod: 0,
            input_count: 0,
            input_delay,
            primed: false,
        };

        // One-time Stage1 diagnostics when env vars are set (prints to stderr, without -v)
        if ST1_DIAG_ONCE.set(()).is_ok()
            && any_env_present(&[
                "DSD2DXD_STAGE1_CHUNK_MULT",
                "DSD2DXD_STAGE1_CHUNK_TARGET",
                "DSD2DXD_STAGE1_CHUNK_ALIGN",
            ])
        {
            // Report groups/phase range and effective chunk + threading summary
            let (min_g, max_g) = if let Some(ref tbl) = me.lut_f64 {
                if tbl.is_empty() {
                    (0, 0)
                } else {
                    let mut min_g = usize::MAX;
                    let mut max_g = 0usize;
                    for phase in tbl.iter() {
                        let g = phase.len();
                        if g < min_g {
                            min_g = g;
                        }
                        if g > max_g {
                            max_g = g;
                        }
                    }
                    (min_g, max_g)
                }
            } else {
                (0, 0)
            };
            let (st1_mult, st1_align, st1_target) =
                get_stage1_chunk_params();
            let example_groups = max_g;
            let chunk_eff = if example_groups > 0 {
                compute_stage1_chunk(example_groups, m as usize, l)
            } else {
                0
            };
            debug!(
                "[CFG] Stage1 L={} M={}: groups/phase={}..{}, chunk_eff={} mode=lut(f64) (params: MULT={}, TARGET={}, ALIGN={} [0=>M])",
                l,
                m,
                min_g,
                max_g,
                chunk_eff,
                st1_mult,
                st1_target,
                st1_align
            );
        }
        me
    }

    #[inline(always)]
    fn set_next_bit(&mut self, is_pos: bool) {
        let word = self.wbits >> 6;
        let bit = self.wbits & 63;
        if is_pos {
            self.ring_bits[word] |= 1u64 << bit;
        } else {
            self.ring_bits[word] &= !(1u64 << bit);
        }
        self.wbits = (self.wbits + 1) & self.bits_mask;
        // Update rolling byte (newest-first in LSB) and write to byte ring
        let b = if is_pos { 1u8 } else { 0u8 };
        self.rolling_byte = ((self.rolling_byte << 1) & 0xFF) | b;
        self.byte_ring[self.wbyte] = self.rolling_byte;
        self.wbyte = (self.wbyte + 1) & self.byte_mask;
    }

    #[inline(always)]
    fn push_all<F: FnMut(f64)>(&mut self, bit: u8, mut emit: F) {
        match self.mode {
            Stage1Mode::Accum => {
                self.push_accum_all(bit, emit);
            }
            Stage1Mode::SlotSim => {
                // Original slot simulation at high-rate: write once, advance L slots
                self.set_next_bit(bit != 0);
                for _ in 0..self.l {
                    self.sim_high_slot(&mut emit);
                }
            }
        }
    }

    #[inline]
    fn sim_high_slot<F: FnMut(f64)>(&mut self, emit: &mut F) {
        let idx_high = self.high_index;
        self.high_index += 1;
        if !self.primed {
            if idx_high >= self.delay_high {
                self.primed = true;
                self.phase1 = 0;
            }
            return;
        }
        self.phase1 += 1;
        if self.phase1 != self.m {
            return;
        }
        self.phase1 = 0;
        let phase = (idx_high % self.l as u64) as usize;
        let tbl = self.lut_f64.as_ref().unwrap();
        // Use chunked summation with baked chunk size
        let chunk = compute_stage1_chunk(
            tbl[phase].len(),
            self.m as usize,
            self.l,
        );
        let sum = self.sum_phase_groups_chunked(&tbl[phase][..], chunk);
        emit(sum);
    }

    // Accumulator mode: emit all outputs scheduled for this input bit
    #[inline(always)]
    fn push_accum_all<F: FnMut(f64)>(&mut self, bit: u8, mut emit: F) {
        // Update bitset only
        self.set_next_bit(bit != 0);
        self.input_count += 1;
        self.acc += self.l;
        // Possibly produce multiple outputs if acc exceeds m multiple times
        while self.acc >= self.m {
            self.acc -= self.m;
            if !self.primed {
                if self.input_count >= self.input_delay {
                    self.primed = true;
                } else {
                    // Advance phase even when not emitting to keep alignment
                    self.phase_mod =
                        (self.phase_mod + (self.m % self.l)) % self.l;
                    continue;
                }
            }
            let phase = self.phase_mod as usize;
            let tbl = self.lut_f64.as_ref().unwrap();
            let chunk = compute_stage1_chunk(
                tbl[phase].len(),
                self.m as usize,
                self.l,
            );
            let sum =
                self.sum_phase_groups_chunked(&tbl[phase][..], chunk);
            emit(sum);
            self.phase_mod = (self.phase_mod + (self.m % self.l)) % self.l;
        }
    }

    // Sum all phase groups using the bit-packed ring and per-group
    // LUTs, but process groups in chunks of up to `chunk_bytes` to
    // reduce loop overhead and improve locality of byte extraction.
    #[inline]
    fn sum_phase_groups_chunked(
        &self,
        phase_lut: &[[f64; 256]],
        chunk_bytes: usize,
    ) -> f64 {
        let mut sum = 0.0;
        let total_groups = phase_lut.len();
        if total_groups == 0 {
            return 0.0;
        }
        // Start at last written rolling byte (newest 8-bit window)
        let mut bidx =
            (self.wbyte.wrapping_add(self.byte_mask)) & self.byte_mask; // newest byte index (time t)
        let capb = self.byte_mask + 1;
        let mask = self.byte_mask;
        let mut g = 0usize;
        while g < total_groups {
            let this_chunk = core::cmp::min(chunk_bytes, total_groups - g);
            // Non-unrolled inner loop
            let mut k = 0usize;
            while k < this_chunk {
                let idx = bidx
                    .wrapping_add(capb)
                    .wrapping_sub((k as usize) << 3)
                    & mask;
                let byte = self.byte_ring[idx] as usize;
                sum += phase_lut[g + k][byte];
                k += 1;
            }
            // Advance bidx by the number of groups processed in this chunk
            bidx = bidx
                .wrapping_add(capb)
                .wrapping_sub((this_chunk as usize) << 3)
                & mask;
            g += this_chunk;
        }
        sum
    }
}

/// Lightweight decimator structure direct symmetric convolution only
#[derive(Debug)]
struct DecimFIRSym {
    full: Vec<f64>, // full symmetric taps
    len: usize,
    half: usize,
    has_center: bool,
    center: usize, // (len-1)/2, used for initial output schedule
    decim: usize,  // decimation factor D
    ring: Vec<f64>,
    mask: usize,
    w: usize,          // next write index
    count: usize,      // total samples seen
    next_out_t: usize, // next t index at which an output is due
}

impl DecimFIRSym {
    fn new_from_half(right_half: &[f64], decim: usize) -> Self {
        // Reconstruct full taps (mirror right_half)
        let mut full: Vec<f64> =
            right_half.iter().rev().cloned().collect();
        full.extend_from_slice(right_half);
        let len = full.len();
        let center = (len - 1) / 2;
        let has_center = len % 2 == 1;
        let half = if has_center { (len - 1) / 2 } else { len / 2 };
        // Ring capacity: next power of two >= len + decim (margin)
        let cap = (len + decim).next_power_of_two();
        return Self {
            full,
            len,
            half,
            has_center,
            center,
            decim,
            ring: vec![0.0; cap],
            mask: cap - 1,
            w: 0,
            count: 0,
            next_out_t: center,
        };
    }

    /// Block processing: feed a slice of inputs and write produced outputs into `out`.
    /// Returns number of output samples written.
    #[inline(always)]
    fn process_block(&mut self, input: &[f64], out: &mut [f64]) -> usize {
        let mut produced = 0usize;
        for &x in input {
            // Write newest sample
            self.ring[self.w] = x;
            self.w = (self.w + 1) & self.mask;
            let t = self.count;
            self.count += 1;

            // Produce only when we've reached the next scheduled output time
            if t != self.next_out_t {
                continue;
            }
            // Advance next output time for future samples
            self.next_out_t = self.next_out_t.saturating_add(self.decim);

            let acc = self.convolve_direct();
            if produced < out.len() {
                out[produced] = acc;
                produced += 1;
            }
        }
        produced
    }

    /// Direct full-rate symmetric FIR convolution
    /// Evaluates y[t] = sum_{k=0..len-1} h[k] * x[t - k] when push() determines an output is due.
    #[inline(always)]
    fn convolve_direct(&self) -> f64 {
        let cap = self.ring.len();
        let newest = (self.w + cap - 1) & self.mask; // index of x[t]
        let mut total = 0.0;

        // Pairwise sum using symmetry: for k in 0..half-1
        // y += h[k] * (x[t-k] + x[t-(len-1-k)])
        let len = self.len;
        let half = self.half;
        let has_center = self.has_center;
        let center = self.center;

        // Indices for front (newest side) and back (oldest side)
        let mut idx_front = newest; // x[t - 0]
        let mut idx_back = (newest + cap - (len - 1)) & self.mask; // x[t - (len-1)]

        let mut k = 0usize;
        while k < half {
            let c = self.full[k];
            let xf = self.ring[idx_front];
            let xb = self.ring[idx_back];
            total += c * (xf + xb);
            // advance indices
            idx_front = (idx_front + cap - 1) & self.mask; // older by 1
            idx_back = (idx_back + 1) & self.mask; // newer by 1
            k += 1;
        }

        // Center tap (odd length)
        if has_center {
            let ic = (newest + cap - center) & self.mask;
            total += self.full[center] * self.ring[ic];
        }
        total
    }
}

#[inline]
fn env_usize(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
}

// env_bool removed (unused)

#[inline]
fn env_present(name: &str) -> bool {
    env::var_os(name).is_some()
}

#[inline]
fn any_env_present(names: &[&str]) -> bool {
    names.iter().any(|n| env_present(n))
}

#[inline]
fn round_up_to_multiple(x: usize, a: usize) -> usize {
    if a == 0 {
        return x;
    }
    if x == 0 {
        return 0;
    }
    ((x + a - 1) / a) * a
}

#[inline]
fn round_down_to_multiple(x: usize, a: usize) -> usize {
    if a == 0 {
        return x;
    }
    (x / a) * a
}

#[inline]
fn get_stage1_chunk_params() -> (usize, usize, usize) {
    // Allow zeros as sentinels to enable baked defaults downstream (ALIGN=M, TARGET=128)
    let mult = *ST1_MULT
        .get_or_init(|| env_usize("DSD2DXD_STAGE1_CHUNK_MULT", 0));
    let align = *ST1_ALIGN
        .get_or_init(|| env_usize("DSD2DXD_STAGE1_CHUNK_ALIGN", 0));
    let target = *ST1_TARGET
        .get_or_init(|| env_usize("DSD2DXD_STAGE1_CHUNK_TARGET", 256));
    (mult, align, target)
}

#[inline]
fn compute_stage1_chunk(total_groups: usize, m: usize, _l: u32) -> usize {
    let (mult, mut align, target) = get_stage1_chunk_params();
    // Default chunk: round TARGET≈128 up to multiple of M
    let mut chunk = if target > 0 {
        round_up_to_multiple(target, m)
    } else if mult > 0 {
        m.saturating_mul(mult)
    } else {
        round_up_to_multiple(128, m)
    };
    if chunk == 0 {
        chunk = m.max(1);
    }
    // Default ALIGN=M if unset
    if align == 0 {
        align = m.max(1);
    }
    if chunk > total_groups {
        chunk = total_groups;
    }
    // apply alignment by rounding down; keep within [min, total_groups]
    let min_allowed = align.min(total_groups).max(1);
    chunk = round_down_to_multiple(chunk, align)
        .max(min_allowed)
        .min(total_groups);
    chunk
}

#[inline]
fn hash_f64_slice_fnv1a(v: &[f64]) -> u64 {
    // 64-bit FNV-1a over raw bit patterns, stable for identical tap tables
    let mut h: u64 = 0xcbf29ce484222325; // FNV offset basis
    const P: u64 = 0x00000100000001B3; // FNV prime
    for &f in v {
        let b = f.to_bits();
        h ^= b;
        h = h.wrapping_mul(P);
    }
    h
}

fn build_stage1_lut_from_right_half(
    right_half: &[f64],
    l: u32,
) -> Vec<Vec<[f64; 256]>> {
    // Reconstruct full symmetric taps and polyphase decomposition
    let mut full: Vec<f64> = right_half.iter().rev().cloned().collect();
    full.extend_from_slice(right_half);
    let mut phases: Vec<Vec<f64>> = vec![Vec::new(); l as usize];
    for (i, &c) in full.iter().enumerate() {
        phases[i % l as usize].push(c);
    }
    // Build 8-bit LUT per phase and per group of 8 taps (newest-first order)
    let mut out: Vec<Vec<[f64; 256]>> = Vec::with_capacity(phases.len());
    for taps in &phases {
        let n = taps.len();
        let groups = (n + 7) / 8;
        let mut phase_lut: Vec<[f64; 256]> = Vec::with_capacity(groups);
        for g in 0..groups {
            let base = g * 8;
            let mut coeffs: [f64; 8] = [0.0; 8];
            for j in 0..8 {
                let idx = base + j;
                if idx < n {
                    coeffs[j] = taps[idx];
                }
            }
            let mut group_lut = [0.0f64; 256];
            for b in 0u16..256u16 {
                let mut sum = 0.0f64;
                for j in 0..8 {
                    let sign = if (b >> j) & 1 == 1 { 1.0 } else { -1.0 };
                    sum += coeffs[j] * sign;
                }
                group_lut[b as usize] = sum;
            }
            phase_lut.push(group_lut);
        }
        out.push(phase_lut);
    }
    out
}

fn get_or_build_stage1_lut(
    right_half: &[f64],
    l: u32,
) -> Arc<Vec<Vec<[f64; 256]>>> {
    let key = Stage1LutKey {
        l,
        n: right_half.len() as u32,
        hash: hash_f64_slice_fnv1a(right_half),
    };
    let cache =
        STAGE1_LUT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    // Fast path: try hit
    if let Some(found) = cache.lock().unwrap().get(&key) {
        return found.clone();
    }
    // Miss: build and insert
    let lut = build_stage1_lut_from_right_half(right_half, l);
    let arc = Arc::new(lut);
    cache.lock().unwrap().insert(key, arc.clone());
    arc
}

pub fn compute_decim_and_upsample(
    in_rate: i32,
    out_rate: u32,
) -> (i32, u32) {
    // Determine decimation ratio (M)
    let decim_ratio: i32 = if out_rate == 96_000 && in_rate == 4 {
        // DSD256 -> 96k two-stage LM path: M=588 (×5 -> /21 -> /28)
        588
    } else if (out_rate == 96_000 && in_rate == 2)
        || (out_rate == 192_000 && in_rate == 4)
    {
        294
    } else if out_rate == 96_000
        || out_rate == 192_000
        || out_rate == 384_000
    {
        147
    } else {
        // Integer ratio attempt (fallback to 64)
        let base = DSD_64_RATE * (in_rate as u32);
        if out_rate > 0 && base % (out_rate as u32) == 0 {
            (base / (out_rate as u32)) as i32
        } else {
            64
        }
    };

    // Upsample (L) selection
    let upsample_ratio: u32 = if decim_ratio < 147 {
        1
    } else if out_rate == 384_000 {
        // Specialized two-phase L choices for 384k: DSD64->L20, DSD128->L10, DSD256->L5
        if in_rate == 1 {
            20
        } else if in_rate == 2 {
            10
        } else if in_rate == 4 {
            5
        } else {
            5
        }
    } else if out_rate == 192_000 {
        // 192k: DSD64->L10, DSD128->L5, DSD256->L5 (reuses L5 path)
        if in_rate == 1 {
            10
        } else if in_rate == 2 {
            5
        } else if in_rate == 4 {
            5
        } else {
            5
        }
    } else if out_rate == 96_000 {
        5
    } else {
        5
    };
    (decim_ratio, upsample_ratio)
}

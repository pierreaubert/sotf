use super::super::AudioFrame;
use super::misc::{MAX_DOWNMIX_CH, SPIN_MS_RINGBUFFER, recycle_frame_data, write_chunk_bulk};
use rtrb::Producer;
use std::sync::mpsc::SyncSender;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::engine) enum FrameWriteOutcome {
    Written { samples: usize },
    Dropped,
    ConversionBufferTooSmall,
}

#[inline(always)]
pub(in crate::engine) fn required_conversion_capacity(
    num_frames: usize,
    output_channels: usize,
) -> usize {
    num_frames.saturating_mul(output_channels)
}

/// Write one processed frame into the playback ring buffer.
///
/// This is the producer-side audio hot path. It must stay allocation-free:
/// callers pre-size `conversion_buffer` for the largest expected converted
/// frame before entering steady-state playback.
#[inline(always)]
pub(in crate::engine) fn write_frame_to_ring(
    producer: &mut Producer<f32>,
    recycle_tx: &SyncSender<Vec<f32>>,
    conversion_buffer: &mut Vec<f32>,
    channels: usize,
    frame: AudioFrame,
) -> FrameWriteOutcome {
    if frame.num_channels != channels {
        let num_frames = frame.num_frames;
        let target_len = required_conversion_capacity(num_frames, channels);
        if target_len > producer.buffer().capacity() {
            recycle_frame_data(recycle_tx, frame.data, "converted frame drop");
            std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_RINGBUFFER));
            return FrameWriteOutcome::Dropped;
        }

        if conversion_buffer.capacity() < target_len {
            recycle_frame_data(recycle_tx, frame.data, "converted frame oversized");
            return FrameWriteOutcome::ConversionBufferTooSmall;
        }
        debug_assert!(
            conversion_buffer.capacity() >= target_len,
            "playback conversion buffer must be preallocated before hot path"
        );

        conversion_buffer.clear();
        conversion_buffer.resize(target_len, 0.0);

        if frame.num_channels > channels && channels == 2 {
            downmix_to_stereo(&frame, conversion_buffer);
        } else if frame.num_channels == 2 && channels > 2 {
            upmix_stereo_to_n(&frame, channels, conversion_buffer);
        } else {
            copy_shared_channels(&frame, channels, conversion_buffer);
        }

        let chunk = match producer.write_chunk_uninit(conversion_buffer.len()) {
            Ok(chunk) => chunk,
            Err(_) => {
                recycle_frame_data(recycle_tx, frame.data, "converted frame drop");
                std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_RINGBUFFER));
                return FrameWriteOutcome::Dropped;
            }
        };
        write_chunk_bulk(chunk, conversion_buffer);
        recycle_frame_data(recycle_tx, frame.data, "converted frame written");
        return FrameWriteOutcome::Written {
            samples: conversion_buffer.len(),
        };
    }

    let frame_samples = frame.data.len();
    let chunk = match producer.write_chunk_uninit(frame_samples) {
        Ok(chunk) => chunk,
        Err(_) => {
            recycle_frame_data(recycle_tx, frame.data, "frame drop");
            std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_RINGBUFFER));
            return FrameWriteOutcome::Dropped;
        }
    };
    write_chunk_bulk(chunk, &frame.data);
    recycle_frame_data(recycle_tx, frame.data, "frame written");
    FrameWriteOutcome::Written {
        samples: frame_samples,
    }
}

#[inline(always)]
fn downmix_to_stereo(frame: &AudioFrame, conversion_buffer: &mut [f32]) {
    let n = frame.num_channels.min(MAX_DOWNMIX_CH);
    let has_lfe = n != 5;

    let (sl_idx, sr_idx) = if has_lfe { (4, 5) } else { (3, 4) };
    let (bl_idx, br_idx) = if has_lfe { (6, 7) } else { (5, 6) };
    let (tfl_idx, tfr_idx) = if has_lfe { (8, 9) } else { (7, 8) };

    const C_COEFF: f32 = 0.707;
    const SURROUND_COEFF: f32 = 0.707;
    const BACK_COEFF: f32 = 0.5;
    const HEIGHT_COEFF: f32 = 0.5;

    let mut coeff_sum: f32 = 1.0;
    if n > 2 {
        coeff_sum += C_COEFF;
    }
    if sl_idx < n {
        coeff_sum += SURROUND_COEFF;
    }
    if bl_idx < n {
        coeff_sum += BACK_COEFF;
    }
    if tfl_idx < n {
        coeff_sum += HEIGHT_COEFF;
    }
    let norm = 1.0 / coeff_sum;

    let mut lc = [0.0f32; MAX_DOWNMIX_CH];
    let mut rc = [0.0f32; MAX_DOWNMIX_CH];
    lc[0] = norm;
    rc[1] = norm;
    if n > 2 {
        lc[2] = C_COEFF * norm;
        rc[2] = C_COEFF * norm;
    }
    if sl_idx < n {
        lc[sl_idx] = SURROUND_COEFF * norm;
    }
    if sr_idx < n {
        rc[sr_idx] = SURROUND_COEFF * norm;
    }
    if bl_idx < n {
        lc[bl_idx] = BACK_COEFF * norm;
    }
    if br_idx < n {
        rc[br_idx] = BACK_COEFF * norm;
    }
    if tfl_idx < n {
        lc[tfl_idx] = HEIGHT_COEFF * norm;
    }
    if tfr_idx < n {
        rc[tfr_idx] = HEIGHT_COEFF * norm;
    }

    let lc = &lc[..n];
    let rc = &rc[..n];
    for i in 0..frame.num_frames {
        let src_base = i * frame.num_channels;
        let src = &frame.data[src_base..src_base + n];
        let mut l = 0.0f32;
        let mut r = 0.0f32;
        for ch in 0..n {
            l += src[ch] * lc[ch];
            r += src[ch] * rc[ch];
        }
        conversion_buffer[i * 2] = l;
        conversion_buffer[i * 2 + 1] = r;
    }
}

#[inline(always)]
fn upmix_stereo_to_n(frame: &AudioFrame, channels: usize, conversion_buffer: &mut [f32]) {
    for i in 0..frame.num_frames {
        let src_base = i * 2;
        let dst_base = i * channels;
        conversion_buffer[dst_base] = frame.data[src_base];
        conversion_buffer[dst_base + 1] = frame.data[src_base + 1];
    }
}

#[inline(always)]
fn copy_shared_channels(frame: &AudioFrame, channels: usize, conversion_buffer: &mut [f32]) {
    let shared_channels = frame.num_channels.min(channels);
    for i in 0..frame.num_frames {
        let src_base = i * frame.num_channels;
        let dst_base = i * channels;
        conversion_buffer[dst_base..dst_base + shared_channels]
            .copy_from_slice(&frame.data[src_base..src_base + shared_channels]);
    }
}

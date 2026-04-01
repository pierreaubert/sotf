// ============================================================================
// SIMD Optimizations for Complex Multiplication
// ============================================================================
//
// These functions provide SIMD-accelerated complex multiplication for the
// frequency-domain HRTF convolution hot paths. Complex multiplication:
//   (a + bi) * (c + di) = (ac - bd) + (ad + bc)i
//
// Platform support:
// - x86-64: AVX2 (processes 4 complex f32 at once using 256-bit registers)
// - aarch64: NEON (processes 2 complex f32 at once using 128-bit registers)
// - fallback: Scalar implementation for all other platforms
//
// Performance gains: 2-4x speedup on supported platforms for FFT sizes >= 512

use rustfft::num_complex::Complex;

// AVX2 shuffle constant for swapping re/im pairs
#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
const SHUFFLE_SWAP_RE_IM: i32 = 0b10110001; // Swaps: [re, im] -> [im, re]

#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
/// # Safety
/// Caller must ensure `dst`, `src` and `hrtf` have at least `start + 4` elements.
#[inline]
pub unsafe fn complex_mul_add_simd_chunk(
    dst: &mut [Complex<f32>],
    src: &[Complex<f32>],
    hrtf: &[Complex<f32>],
    start: usize,
) {
    use std::arch::x86_64::*;

    // Process 4 complex numbers (8 floats) at once using AVX2
    // Input layout: [re0, im0, re1, im1, re2, im2, re3, im3]
    unsafe {
        let src_ptr = src.as_ptr().add(start) as *const f32;
        let hrtf_ptr = hrtf.as_ptr().add(start) as *const f32;
        let dst_ptr = dst.as_mut_ptr().add(start) as *mut f32;

        // Load 4 complex numbers
        let a = _mm256_loadu_ps(src_ptr);
        let b = _mm256_loadu_ps(hrtf_ptr);
        let dst_val = _mm256_loadu_ps(dst_ptr);

        // Complex multiplication: (a + bi) * (c + di) = (ac - bd) + (ad + bc)i

        // Duplicate real and imaginary parts correctly:
        // moveldup: duplicates even elements [0, 0, 2, 2, 4, 4, 6, 6] -> [re0, re0, re1, re1, ...]
        // movehdup: duplicates odd elements  [1, 1, 3, 3, 5, 5, 7, 7] -> [im0, im0, im1, im1, ...]
        let a_re = _mm256_moveldup_ps(a);
        let a_im = _mm256_movehdup_ps(a);

        // Compute: a.re * b = [re*re, re*im, ...] = [ac, ad, ...]
        let ac_ad = _mm256_mul_ps(a_re, b);

        // Swap b's re/im: [im, re, im, re, ...] = [d, c, ...]
        let b_swapped = _mm256_shuffle_ps(b, b, SHUFFLE_SWAP_RE_IM);

        // Compute: a.im * b_swapped = [im*im, im*re, ...] = [bd, bc, ...]
        let bd_bc = _mm256_mul_ps(a_im, b_swapped);

        // Combine using addsub: performs [a[0]-b[0], a[1]+b[1], a[2]-b[2], a[3]+b[3], ...]
        // This gives us: [(ac - bd), (ad + bc), ...] = [result.re, result.im, ...]
        let result = _mm256_addsub_ps(ac_ad, bd_bc);

        // Add to destination (accumulate)
        let final_result = _mm256_add_ps(dst_val, result);

        _mm256_storeu_ps(dst_ptr, final_result);
    }
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
/// # Safety
/// Caller must ensure `dst`, `src` and `hrtf` have at least `start + 2` elements.
#[inline]
pub unsafe fn complex_mul_add_simd_chunk(
    dst: &mut [Complex<f32>],
    src: &[Complex<f32>],
    hrtf: &[Complex<f32>],
    start: usize,
) {
    use std::arch::aarch64::*;

    // Process 2 complex numbers (4 floats) at once using NEON
    // Input layout: [re0, im0, re1, im1]
    unsafe {
        let src_ptr = src.as_ptr().add(start) as *const f32;
        let hrtf_ptr = hrtf.as_ptr().add(start) as *const f32;
        let dst_ptr = dst.as_mut_ptr().add(start) as *mut f32;

        // Load 2 complex numbers
        let a = vld1q_f32(src_ptr);
        let b = vld1q_f32(hrtf_ptr);
        let dst_val = vld1q_f32(dst_ptr);

        // Complex multiplication: (a + bi) * (c + di) = (ac - bd) + (ad + bc)i

        // Duplicate real and imaginary parts properly for 2 complex numbers:
        // vtrn1q_f32 extracts elements [0, 2, 0, 2] from both inputs (when both are same)
        // vtrn2q_f32 extracts elements [1, 3, 1, 3] from both inputs (when both are same)
        // This gives us [re0, re0, re1, re1] and [im0, im0, im1, im1]
        let a_re = vtrn1q_f32(a, a); // [re0, re0, re1, re1]
        let a_im = vtrn2q_f32(a, a); // [im0, im0, im1, im1]

        // Compute: a.re * b = [re*re, re*im, ...] = [ac, ad, ...]
        let ac_ad = vmulq_f32(a_re, b);

        // Swap b's re/im using vrev64: [re0, im0, re1, im1] -> [im0, re0, im1, re1]
        let b_swapped = vrev64q_f32(b);

        // Compute: a.im * b_swapped = [im*im, im*re, ...] = [bd, bc, ...]
        let bd_bc = vmulq_f32(a_im, b_swapped);

        // Combine: (ac - bd, ad + bc)
        // Create alternating negation mask: [0x80000000, 0, 0x80000000, 0] for [-, +, -, +]
        let sign_bit: u32 = 0x80000000;
        let neg_mask = vreinterpretq_f32_u32(vsetq_lane_u32::<2>(
            sign_bit,
            vsetq_lane_u32::<0>(sign_bit, vdupq_n_u32(0)),
        ));

        // Apply alternating negation to bd_bc, then add to ac_ad
        let bd_bc_negated = vreinterpretq_f32_u32(veorq_u32(
            vreinterpretq_u32_f32(bd_bc),
            vreinterpretq_u32_f32(neg_mask),
        ));
        let result = vaddq_f32(ac_ad, bd_bc_negated);

        // Add to destination (accumulate)
        let final_result = vaddq_f32(dst_val, result);

        vst1q_f32(dst_ptr, final_result);
    }
}

#[cfg(not(any(
    all(target_arch = "x86_64", target_feature = "avx2"),
    all(target_arch = "aarch64", target_feature = "neon")
)))]
#[inline]
pub fn complex_mul_add_simd_chunk(
    dst: &mut [Complex<f32>],
    src: &[Complex<f32>],
    hrtf: &[Complex<f32>],
    start: usize,
) {
    // Scalar fallback (will be optimized by LLVM auto-vectorization)
    dst[start] += src[start] * hrtf[start];
}

/// SIMD-optimized complex multiply-accumulate
///
/// Computes: `dst[i] += src[i] * hrtf[i]` for all `i`
///
/// Uses platform-specific SIMD instructions for maximum performance:
/// - AVX2 on x86-64 (4 complex at once)
/// - NEON on aarch64 (2 complex at once)
/// - Scalar fallback with auto-vectorization hints
#[inline]
pub fn complex_mul_add_simd(dst: &mut [Complex<f32>], src: &[Complex<f32>], hrtf: &[Complex<f32>]) {
    let len = dst.len();

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    {
        // Process 4 complex at a time with AVX2
        let simd_len = (len / 4) * 4;

        for i in (0..simd_len).step_by(4) {
            unsafe {
                complex_mul_add_simd_chunk(dst, src, hrtf, i);
            }
        }

        // Scalar remainder
        for i in simd_len..len {
            dst[i] += src[i] * hrtf[i];
        }
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        // Process 2 complex at a time with NEON
        let simd_len = (len / 2) * 2;

        for i in (0..simd_len).step_by(2) {
            unsafe {
                complex_mul_add_simd_chunk(dst, src, hrtf, i);
            }
        }

        // Scalar remainder
        for i in simd_len..len {
            dst[i] += src[i] * hrtf[i];
        }
    }

    #[cfg(not(any(
        all(target_arch = "x86_64", target_feature = "avx2"),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        // Scalar fallback
        for i in 0..len {
            dst[i] += src[i] * hrtf[i];
        }
    }
}

/// SIMD-optimized complex multiplication (without accumulation)
///
/// Computes: `dst[i] = src[i] * hrtf[i]` for all `i`
#[inline]
pub fn complex_mul_simd(dst: &mut [Complex<f32>], src: &[Complex<f32>], hrtf: &[Complex<f32>]) {
    let len = dst.len();

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    {
        use std::arch::x86_64::*;

        let simd_len = (len / 4) * 4;

        for i in (0..simd_len).step_by(4) {
            unsafe {
                let src_ptr = src.as_ptr().add(i) as *const f32;
                let hrtf_ptr = hrtf.as_ptr().add(i) as *const f32;
                let dst_ptr = dst.as_mut_ptr().add(i) as *mut f32;

                let a = _mm256_loadu_ps(src_ptr);
                let b = _mm256_loadu_ps(hrtf_ptr);

                let a_re = _mm256_moveldup_ps(a);
                let a_im = _mm256_movehdup_ps(a);
                let ac_ad = _mm256_mul_ps(a_re, b);
                let b_swapped = _mm256_shuffle_ps(b, b, SHUFFLE_SWAP_RE_IM);
                let bd_bc = _mm256_mul_ps(a_im, b_swapped);
                let result = _mm256_addsub_ps(ac_ad, bd_bc);

                _mm256_storeu_ps(dst_ptr, result);
            }
        }

        for i in simd_len..len {
            dst[i] = src[i] * hrtf[i];
        }
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        use std::arch::aarch64::*;

        let simd_len = (len / 2) * 2;

        for i in (0..simd_len).step_by(2) {
            unsafe {
                let src_ptr = src.as_ptr().add(i) as *const f32;
                let hrtf_ptr = hrtf.as_ptr().add(i) as *const f32;
                let dst_ptr = dst.as_mut_ptr().add(i) as *mut f32;

                let a = vld1q_f32(src_ptr);
                let b = vld1q_f32(hrtf_ptr);

                let a_re = vtrn1q_f32(a, a);
                let a_im = vtrn2q_f32(a, a);
                let ac_ad = vmulq_f32(a_re, b);
                let b_swapped = vrev64q_f32(b);
                let bd_bc = vmulq_f32(a_im, b_swapped);

                let sign_bit: u32 = 0x80000000;
                let neg_mask = vreinterpretq_f32_u32(vsetq_lane_u32::<2>(
                    sign_bit,
                    vsetq_lane_u32::<0>(sign_bit, vdupq_n_u32(0)),
                ));

                let bd_bc_negated = vreinterpretq_f32_u32(veorq_u32(
                    vreinterpretq_u32_f32(bd_bc),
                    vreinterpretq_u32_f32(neg_mask),
                ));

                let result = vaddq_f32(ac_ad, bd_bc_negated);
                vst1q_f32(dst_ptr, result);
            }
        }

        for i in simd_len..len {
            dst[i] = src[i] * hrtf[i];
        }
    }

    #[cfg(not(any(
        all(target_arch = "x86_64", target_feature = "avx2"),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        for i in 0..len {
            dst[i] = src[i] * hrtf[i];
        }
    }
}

/// SIMD-optimized in-place complex multiplication
///
/// Computes: `dst[i] *= hrtf[i]` for all `i`
#[inline]
#[allow(dead_code)]
pub fn complex_mul_inplace_simd(dst: &mut [Complex<f32>], hrtf: &[Complex<f32>]) {
    let len = dst.len();

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    {
        use std::arch::x86_64::*;

        let simd_len = (len / 4) * 4;

        for i in (0..simd_len).step_by(4) {
            unsafe {
                let dst_ptr = dst.as_mut_ptr().add(i) as *mut f32;
                let hrtf_ptr = hrtf.as_ptr().add(i) as *const f32;

                let a = _mm256_loadu_ps(dst_ptr);
                let b = _mm256_loadu_ps(hrtf_ptr);

                let a_re = _mm256_moveldup_ps(a);
                let a_im = _mm256_movehdup_ps(a);
                let ac_ad = _mm256_mul_ps(a_re, b);
                let b_swapped = _mm256_shuffle_ps(b, b, SHUFFLE_SWAP_RE_IM);
                let bd_bc = _mm256_mul_ps(a_im, b_swapped);
                let result = _mm256_addsub_ps(ac_ad, bd_bc);

                _mm256_storeu_ps(dst_ptr, result);
            }
        }

        for i in simd_len..len {
            dst[i] *= hrtf[i];
        }
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        use std::arch::aarch64::*;

        let simd_len = (len / 2) * 2;

        for i in (0..simd_len).step_by(2) {
            unsafe {
                let dst_ptr = dst.as_mut_ptr().add(i) as *mut f32;
                let hrtf_ptr = hrtf.as_ptr().add(i) as *const f32;

                let a = vld1q_f32(dst_ptr);
                let b = vld1q_f32(hrtf_ptr);

                let a_re = vtrn1q_f32(a, a);
                let a_im = vtrn2q_f32(a, a);
                let ac_ad = vmulq_f32(a_re, b);
                let b_swapped = vrev64q_f32(b);
                let bd_bc = vmulq_f32(a_im, b_swapped);

                let sign_bit: u32 = 0x80000000;
                let neg_mask = vreinterpretq_f32_u32(vsetq_lane_u32::<2>(
                    sign_bit,
                    vsetq_lane_u32::<0>(sign_bit, vdupq_n_u32(0)),
                ));

                let bd_bc_negated = vreinterpretq_f32_u32(veorq_u32(
                    vreinterpretq_u32_f32(bd_bc),
                    vreinterpretq_u32_f32(neg_mask),
                ));

                let result = vaddq_f32(ac_ad, bd_bc_negated);
                vst1q_f32(dst_ptr, result);
            }
        }

        for i in simd_len..len {
            dst[i] *= hrtf[i];
        }
    }

    #[cfg(not(any(
        all(target_arch = "x86_64", target_feature = "avx2"),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        for i in 0..len {
            dst[i] *= hrtf[i];
        }
    }
}

// ============================================================================
// Real-valued SIMD operations for windowing and overlap-add
// ============================================================================

/// SIMD-optimized multiply-accumulate for overlap-add synthesis (no window)
///
/// Computes: `dst[i] += src[i] * scale` for all `i`
#[inline]
pub fn scale_add_simd(dst: &mut [f32], src: &[f32], scale: f32) {
    debug_assert_eq!(dst.len(), src.len());
    let len = dst.len();

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    {
        use std::arch::x86_64::*;

        let scale_vec = unsafe { _mm256_set1_ps(scale) };
        let simd_len = (len / 8) * 8;

        for i in (0..simd_len).step_by(8) {
            unsafe {
                let src_ptr = src.as_ptr().add(i);
                let dst_ptr = dst.as_mut_ptr().add(i);

                let s = _mm256_loadu_ps(src_ptr);
                let d = _mm256_loadu_ps(dst_ptr);

                // src * scale + dst
                let ss = _mm256_mul_ps(s, scale_vec);
                let result = _mm256_add_ps(d, ss);

                _mm256_storeu_ps(dst_ptr, result);
            }
        }

        // Scalar remainder
        for i in simd_len..len {
            dst[i] += src[i] * scale;
        }
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        use std::arch::aarch64::*;

        let scale_vec = unsafe { vdupq_n_f32(scale) };
        let simd_len = (len / 4) * 4;

        for i in (0..simd_len).step_by(4) {
            unsafe {
                let src_ptr = src.as_ptr().add(i);
                let dst_ptr = dst.as_mut_ptr().add(i);

                let s = vld1q_f32(src_ptr);
                let d = vld1q_f32(dst_ptr);

                // FMA: dst + src * scale in a single fused instruction
                let result = vfmaq_f32(d, s, scale_vec);

                vst1q_f32(dst_ptr, result);
            }
        }

        // Scalar remainder
        for i in simd_len..len {
            dst[i] += src[i] * scale;
        }
    }

    #[cfg(not(any(
        all(target_arch = "x86_64", target_feature = "avx2"),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        for i in 0..len {
            dst[i] += src[i] * scale;
        }
    }
}

/// SIMD-optimized in-place scaling.
///
/// Computes: `data[i] *= scale` for all `i`
#[inline]
pub fn scale_add_simd_inplace(data: &mut [f32], scale: f32) {
    let len = data.len();

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    {
        use std::arch::x86_64::*;

        let scale_vec = unsafe { _mm256_set1_ps(scale) };
        let simd_len = (len / 8) * 8;

        for i in (0..simd_len).step_by(8) {
            unsafe {
                let ptr = data.as_mut_ptr().add(i);
                let d = _mm256_loadu_ps(ptr);
                _mm256_storeu_ps(ptr, _mm256_mul_ps(d, scale_vec));
            }
        }

        for sample in data.iter_mut().take(len).skip(simd_len) {
            *sample *= scale;
        }
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        use std::arch::aarch64::*;

        let scale_vec = unsafe { vdupq_n_f32(scale) };
        let simd_len = (len / 4) * 4;

        for i in (0..simd_len).step_by(4) {
            unsafe {
                let ptr = data.as_mut_ptr().add(i);
                let d = vld1q_f32(ptr);
                vst1q_f32(ptr, vmulq_f32(d, scale_vec));
            }
        }

        for sample in &mut data[simd_len..len] {
            *sample *= scale;
        }
    }

    #[cfg(not(any(
        all(target_arch = "x86_64", target_feature = "avx2"),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        for sample in data {
            *sample *= scale;
        }
    }
}

/// SIMD-optimized linear interpolation (blend) between two buffers
///
/// Computes: `dst[i] = prev[i] + alpha * (dst[i] - prev[i])` for all `i`
/// Equivalent to: `dst[i] = (1 - alpha) * prev[i] + alpha * dst[i]`
///
/// Used for crossfading between old and new filter outputs in STFT plugins.
#[inline]
pub fn blend_simd(dst: &mut [f32], prev: &[f32], alpha: f32) {
    debug_assert_eq!(dst.len(), prev.len());
    let len = dst.len();

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    {
        use std::arch::x86_64::*;

        let alpha_vec = unsafe { _mm256_set1_ps(alpha) };
        let simd_len = (len / 8) * 8;

        for i in (0..simd_len).step_by(8) {
            unsafe {
                let prev_ptr = prev.as_ptr().add(i);
                let dst_ptr = dst.as_mut_ptr().add(i);

                let p = _mm256_loadu_ps(prev_ptr);
                let d = _mm256_loadu_ps(dst_ptr);

                // prev + alpha * (dst - prev)
                let diff = _mm256_sub_ps(d, p);
                let result = _mm256_fmadd_ps(alpha_vec, diff, p);

                _mm256_storeu_ps(dst_ptr, result);
            }
        }

        for i in simd_len..len {
            dst[i] = prev[i] + alpha * (dst[i] - prev[i]);
        }
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        use std::arch::aarch64::*;

        let alpha_vec = unsafe { vdupq_n_f32(alpha) };
        let simd_len = (len / 4) * 4;

        for i in (0..simd_len).step_by(4) {
            unsafe {
                let prev_ptr = prev.as_ptr().add(i);
                let dst_ptr = dst.as_mut_ptr().add(i);

                let p = vld1q_f32(prev_ptr);
                let d = vld1q_f32(dst_ptr);

                // prev + alpha * (dst - prev)
                let diff = vsubq_f32(d, p);
                let result = vfmaq_f32(p, alpha_vec, diff);

                vst1q_f32(dst_ptr, result);
            }
        }

        for i in simd_len..len {
            dst[i] = prev[i] + alpha * (dst[i] - prev[i]);
        }
    }

    #[cfg(not(any(
        all(target_arch = "x86_64", target_feature = "avx2"),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        for i in 0..len {
            dst[i] = prev[i] + alpha * (dst[i] - prev[i]);
        }
    }
}

/// SIMD-optimized windowed copy (for FFT input preparation)
///
/// Computes: `dst[i] = src[i] * window[i]` for all `i`
#[inline]
pub fn window_mul_simd(dst: &mut [f32], src: &[f32], window: &[f32]) {
    debug_assert_eq!(dst.len(), src.len());
    debug_assert_eq!(dst.len(), window.len());
    let len = dst.len();

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    {
        use std::arch::x86_64::*;

        let simd_len = (len / 8) * 8;

        for i in (0..simd_len).step_by(8) {
            unsafe {
                let src_ptr = src.as_ptr().add(i);
                let win_ptr = window.as_ptr().add(i);
                let dst_ptr = dst.as_mut_ptr().add(i);

                let s = _mm256_loadu_ps(src_ptr);
                let w = _mm256_loadu_ps(win_ptr);
                let result = _mm256_mul_ps(s, w);

                _mm256_storeu_ps(dst_ptr, result);
            }
        }

        for i in simd_len..len {
            dst[i] = src[i] * window[i];
        }
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        use std::arch::aarch64::*;

        let simd_len = (len / 4) * 4;

        for i in (0..simd_len).step_by(4) {
            unsafe {
                let src_ptr = src.as_ptr().add(i);
                let win_ptr = window.as_ptr().add(i);
                let dst_ptr = dst.as_mut_ptr().add(i);

                let s = vld1q_f32(src_ptr);
                let w = vld1q_f32(win_ptr);
                let result = vmulq_f32(s, w);

                vst1q_f32(dst_ptr, result);
            }
        }

        for i in simd_len..len {
            dst[i] = src[i] * window[i];
        }
    }

    #[cfg(not(any(
        all(target_arch = "x86_64", target_feature = "avx2"),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        for i in 0..len {
            dst[i] = src[i] * window[i];
        }
    }
}

/// SIMD-optimized in-place window multiplication.
#[inline]
pub fn window_mul_simd_inplace(data: &mut [f32], window: &[f32]) {
    let len = data.len().min(window.len());

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    {
        use std::arch::x86_64::*;
        let simd_len = (len / 8) * 8;
        for i in (0..simd_len).step_by(8) {
            unsafe {
                let ptr = data.as_mut_ptr().add(i);
                let win_ptr = window.as_ptr().add(i);
                let d = _mm256_loadu_ps(ptr);
                let w = _mm256_loadu_ps(win_ptr);
                _mm256_storeu_ps(ptr, _mm256_mul_ps(d, w));
            }
        }
        for i in simd_len..len {
            data[i] *= window[i];
        }
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        use std::arch::aarch64::*;
        let simd_len = (len / 4) * 4;
        for i in (0..simd_len).step_by(4) {
            unsafe {
                let ptr = data.as_mut_ptr().add(i);
                let win_ptr = window.as_ptr().add(i);
                let d = vld1q_f32(ptr);
                let w = vld1q_f32(win_ptr);
                vst1q_f32(ptr, vmulq_f32(d, w));
            }
        }
        for i in simd_len..len {
            data[i] *= window[i];
        }
    }

    #[cfg(not(any(
        all(target_arch = "x86_64", target_feature = "avx2"),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        for i in 0..len {
            data[i] *= window[i];
        }
    }
}

/// Deinterleave stereo buffer into separate L/R channels
///
/// Input: [L0, R0, L1, R1, L2, R2, ...]
/// Output: left = [L0, L1, L2, ...], right = [R0, R1, R2, ...]
#[inline]
pub fn deinterleave_stereo(input: &[f32], left: &mut [f32], right: &mut [f32]) {
    debug_assert_eq!(input.len(), left.len() * 2);
    debug_assert_eq!(left.len(), right.len());

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    {
        use std::arch::x86_64::*;

        let len = left.len();
        let simd_len = (len / 8) * 8;

        for i in (0..simd_len).step_by(8) {
            unsafe {
                // Load 16 floats (8 stereo pairs)
                let in_ptr = input.as_ptr().add(i * 2);
                let v0 = _mm256_loadu_ps(in_ptr); // L0 R0 L1 R1 L2 R2 L3 R3
                let v1 = _mm256_loadu_ps(in_ptr.add(8)); // L4 R4 L5 R5 L6 R6 L7 R7

                // Shuffle to separate L and R
                // Within 256-bit lanes, shuffle to group L/R
                let shuf_l = _mm256_shuffle_ps(v0, v1, 0b10_00_10_00); // L0 L1 L4 L5 | L2 L3 L6 L7
                let shuf_r = _mm256_shuffle_ps(v0, v1, 0b11_01_11_01); // R0 R1 R4 R5 | R2 R3 R6 R7

                // Permute to get correct order
                let left_vec = _mm256_permute4x64_pd(
                    std::mem::transmute::<__m256, __m256d>(shuf_l),
                    0b11_01_10_00,
                );
                let right_vec = _mm256_permute4x64_pd(
                    std::mem::transmute::<__m256, __m256d>(shuf_r),
                    0b11_01_10_00,
                );

                _mm256_storeu_ps(
                    left.as_mut_ptr().add(i),
                    std::mem::transmute::<__m256d, __m256>(left_vec),
                );
                _mm256_storeu_ps(
                    right.as_mut_ptr().add(i),
                    std::mem::transmute::<__m256d, __m256>(right_vec),
                );
            }
        }

        // Scalar remainder
        for i in simd_len..len {
            left[i] = input[i * 2];
            right[i] = input[i * 2 + 1];
        }
    }

    #[cfg(not(all(target_arch = "x86_64", target_feature = "avx2")))]
    {
        // Scalar fallback (compiler will auto-vectorize for NEON)
        for (i, chunk) in input.chunks_exact(2).enumerate() {
            left[i] = chunk[0];
            right[i] = chunk[1];
        }
    }
}

/// Interleave separate L/R channels into stereo buffer
///
/// Input: left = [L0, L1, L2, ...], right = [R0, R1, R2, ...]
/// Output: [L0, R0, L1, R1, L2, R2, ...]
#[inline]
#[allow(dead_code)]
pub fn interleave_stereo(left: &[f32], right: &[f32], output: &mut [f32]) {
    debug_assert_eq!(left.len(), right.len());
    debug_assert_eq!(output.len(), left.len() * 2);

    // Scalar version - compiler auto-vectorizes well
    for i in 0..left.len() {
        output[i * 2] = left[i];
        output[i * 2 + 1] = right[i];
    }
}

/// Flush denormal numbers to zero to prevent CPU performance spikes and audio glitches.
///
/// Denormal floats (values with magnitude < 1e-30) cause significant CPU overhead
/// when processed by FMA instructions, leading to audio artifacts and crackle.
/// This function checks each sample and sets denormals to exactly 0.0.
#[inline]
pub fn flush_denormals_inplace(samples: &mut [f32]) {
    const DENORM_THRESHOLD: f32 = 1e-30;

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    {
        use std::arch::x86_64::*;

        let threshold = unsafe { _mm256_set1_ps(DENORM_THRESHOLD) };
        let zero = unsafe { _mm256_set1_ps(0.0) };
        let len = samples.len();
        let simd_len = (len / 8) * 8;

        for i in (0..simd_len).step_by(8) {
            unsafe {
                let ptr = samples.as_mut_ptr().add(i);
                let val = _mm256_loadu_ps(ptr);
                let abs_val = _mm256_andnot_ps(_mm256_set1_ps(-0.0), val);
                let mask = _mm256_cmp_ps(abs_val, threshold, _CMP_LT_OQ);
                let result = _mm256_blendv_ps(val, zero, mask);
                _mm256_storeu_ps(ptr, result);
            }
        }

        for sample in samples.iter_mut().take(len).skip(simd_len) {
            if sample.abs() < DENORM_THRESHOLD {
                *sample = 0.0;
            }
        }
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        use std::arch::aarch64::*;

        let threshold = unsafe { vdupq_n_f32(DENORM_THRESHOLD) };
        let zero = unsafe { vdupq_n_f32(0.0) };
        let len = samples.len();
        let simd_len = (len / 4) * 4;

        for i in (0..simd_len).step_by(4) {
            unsafe {
                let ptr = samples.as_mut_ptr().add(i);
                let val = vld1q_f32(ptr);
                let abs_val = vabsq_f32(val);
                let mask = vcltq_f32(abs_val, threshold);
                let result = vbslq_f32(mask, zero, val);
                vst1q_f32(ptr, result);
            }
        }

        for sample in &mut samples[simd_len..len] {
            if sample.abs() < DENORM_THRESHOLD {
                *sample = 0.0;
            }
        }
    }

    #[cfg(not(any(
        all(target_arch = "x86_64", target_feature = "avx2"),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        for sample in samples {
            if sample.abs() < DENORM_THRESHOLD {
                *sample = 0.0;
            }
        }
    }
}

/// Enable FTZ (Flush To Zero) and DAZ (Denormals Are Zero) CPU flags.
///
/// When enabled, denormal floating-point numbers are automatically flushed to zero
/// by the CPU hardware. This prevents the severe performance degradation that occurs
/// when IIR filter state variables (like biquad y1/y2) contain denormals.
///
/// This function is idempotent and should be called once per audio processing thread.
/// On unsupported platforms, this is a no-op.
///
/// Returns true if the flags were successfully set, false otherwise (e.g., unsupported platform).
#[inline]
pub fn enable_ftz_daz() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        // MXCSR register bits:
        // Bit 15: FTZ (Flush To Zero) - flush denormal results to zero
        // Bit 6: DAZ (Denormals Are Zero) - treat denormal inputs as zero
        unsafe {
            let mut mxcsr: u32 = 0;
            std::arch::asm!("stmxcsr [{}]", in(reg) &mut mxcsr, options(nostack, preserves_flags));
            mxcsr |= (1 << 15) | (1 << 6); // FTZ | DAZ
            std::arch::asm!("ldmxcsr [{}]", in(reg) &mxcsr, options(nostack, preserves_flags));
        }
        true
    }

    #[cfg(target_arch = "aarch64")]
    {
        // On AArch64, use the FPCR register
        // Bit 24: FZ (Flush-to-Zero)
        // Note: AArch64 always treats denormal inputs as zero in FZ mode
        unsafe {
            let mut fpcr: u64;
            std::arch::asm!("mrs {}, fpcr", out(reg) fpcr);
            fpcr |= 1 << 24; // FZ bit
            std::arch::asm!("msr fpcr, {}", in(reg) fpcr);
        }
        true
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        false
    }
}

/// Flush denormals in complex buffer (applies to both real and imaginary parts)
#[inline]
pub fn flush_denormals_complex_inplace(samples: &mut [Complex<f32>]) {
    // A Complex<f32> is just two f32s (re and im)
    // We can treat the whole buffer as a slice of f32 and use flush_denormals_inplace
    let len = samples.len() * 2;
    let ptr = samples.as_mut_ptr() as *mut f32;
    let f32_samples = unsafe { std::slice::from_raw_parts_mut(ptr, len) };
    flush_denormals_inplace(f32_samples);
}

#[cfg(test)]
mod denorm_tests {
    use super::*;

    #[test]
    fn test_flush_denormals_basic() {
        let mut samples = [1e-31_f32, 1e-20, 1e-10, 0.0, -1e-31, 1.0];
        flush_denormals_inplace(&mut samples);
        assert_eq!(samples[0], 0.0);
        assert_eq!(samples[1], 1e-20);
        assert_eq!(samples[2], 1e-10);
        assert_eq!(samples[3], 0.0);
        assert_eq!(samples[4], 0.0);
        assert_eq!(samples[5], 1.0);
    }

    #[test]
    fn test_flush_denormals_complex() {
        use rustfft::num_complex::Complex;
        let mut samples = [
            Complex::new(1e-31, 1e-30),
            Complex::new(1.0, 1e-31),
            Complex::new(0.0, 0.0),
        ];
        flush_denormals_complex_inplace(&mut samples);
        assert_eq!(samples[0].re, 0.0);
        assert!((samples[0].im - 1e-30).abs() < 1e-35);
        assert_eq!(samples[1].re, 1.0);
        assert_eq!(samples[1].im, 0.0);
        assert_eq!(samples[2].re, 0.0);
        assert_eq!(samples[2].im, 0.0);
    }

    #[test]
    fn test_flush_denormals_empty() {
        let mut samples: [f32; 0] = [];
        flush_denormals_inplace(&mut samples);
    }

    #[test]
    fn test_flush_denormals_unaligned() {
        let mut samples = [1e-31_f32; 7];
        flush_denormals_inplace(&mut samples);
        for s in samples.iter() {
            assert_eq!(*s, 0.0);
        }
    }
}

#[cfg(test)]
#[allow(clippy::needless_range_loop)]
mod tests {
    use super::*;

    // ============================================================================
    // Denormal Flushing Tests
    // ============================================================================

    #[test]
    fn test_flush_denormals_basic() {
        let mut samples = [1e-31_f32, 1e-20, 1e-10, 0.0, -1e-31, 1.0];
        flush_denormals_inplace(&mut samples);
        assert_eq!(samples[0], 0.0);
        assert_eq!(samples[1], 1e-20);
        assert_eq!(samples[2], 1e-10);
        assert_eq!(samples[3], 0.0);
        assert_eq!(samples[4], 0.0);
        assert_eq!(samples[5], 1.0);
    }

    #[test]
    fn test_flush_denormals_complex() {
        use rustfft::num_complex::Complex;
        let mut samples = [
            Complex::new(1e-31, 1e-30),
            Complex::new(1.0, 1e-31),
            Complex::new(0.0, 0.0),
        ];
        flush_denormals_complex_inplace(&mut samples);
        assert_eq!(samples[0].re, 0.0);
        assert!((samples[0].im - 1e-30).abs() < 1e-35);
        assert_eq!(samples[1].re, 1.0);
        assert_eq!(samples[1].im, 0.0);
        assert_eq!(samples[2].re, 0.0);
        assert_eq!(samples[2].im, 0.0);
    }

    #[test]
    fn test_flush_denormals_empty() {
        let mut samples: [f32; 0] = [];
        flush_denormals_inplace(&mut samples);
    }

    #[test]
    fn test_flush_denormals_unaligned() {
        let mut samples = [1e-31_f32; 7];
        flush_denormals_inplace(&mut samples);
        for s in samples.iter() {
            assert_eq!(*s, 0.0);
        }
    }

    #[test]
    fn test_enable_ftz_daz_does_not_panic() {
        // enable_ftz_daz() should complete without panicking on any platform.
        // On x86_64 or aarch64 it returns true; on other platforms false.
        let result = enable_ftz_daz();
        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
        assert!(
            result,
            "enable_ftz_daz should return true on supported platforms"
        );
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        assert!(
            !result,
            "enable_ftz_daz should return false on unsupported platforms"
        );
    }

    #[test]
    fn test_apply_gain_simd_known_values() {
        // Apply gain of 2.0 to known input
        let mut buffer = vec![1.0, 2.0, 3.0, 4.0, -1.0, 0.5, -0.5, 0.0, 1.5];
        let expected: Vec<f32> = buffer.iter().map(|&x| x * 2.0).collect();
        apply_gain_simd(&mut buffer, 2.0);
        for (i, (&got, &exp)) in buffer.iter().zip(expected.iter()).enumerate() {
            assert!(
                (got - exp).abs() < 1e-6,
                "apply_gain_simd mismatch at index {}: got {}, expected {}",
                i,
                got,
                exp
            );
        }
    }

    #[test]
    fn test_apply_gain_simd_zero_gain() {
        let mut buffer = vec![1.0, -2.0, 3.5, 0.7];
        apply_gain_simd(&mut buffer, 0.0);
        for (i, &v) in buffer.iter().enumerate() {
            assert_eq!(
                v, 0.0,
                "apply_gain_simd with zero gain: index {} not zero",
                i
            );
        }
    }

    #[test]
    fn test_apply_gain_simd_unity_gain() {
        let original = vec![1.0, -2.0, 3.5, 0.7, 0.0, -0.1];
        let mut buffer = original.clone();
        apply_gain_simd(&mut buffer, 1.0);
        assert_eq!(buffer, original);
    }

    #[test]
    fn test_apply_per_channel_gain_simd_stereo() {
        // Stereo buffer: apply different gains to L and R
        let mut buffer = vec![1.0, 2.0, 3.0, 4.0]; // 2 frames, 2 channels
        let gains = vec![0.5, 2.0]; // L=0.5, R=2.0
        apply_per_channel_gain_simd(&mut buffer, 2, &gains);
        assert!((buffer[0] - 0.5).abs() < 1e-6, "L frame 0");
        assert!((buffer[1] - 4.0).abs() < 1e-6, "R frame 0");
        assert!((buffer[2] - 1.5).abs() < 1e-6, "L frame 1");
        assert!((buffer[3] - 8.0).abs() < 1e-6, "R frame 1");
    }

    // ============================================================================
    // SIMD Correctness Tests
    // ============================================================================
    //
    // These tests verify that SIMD-optimized complex multiplication produces
    // identical results to scalar computation

    #[test]
    fn test_simd_complex_mul_add_correctness() {
        // Test SIMD complex multiply-accumulate against scalar reference
        use rustfft::num_complex::Complex;

        // Test with 8 complex numbers (to test both AVX2 and NEON code paths)
        let src = vec![
            Complex::new(1.0, 2.0),
            Complex::new(3.0, 4.0),
            Complex::new(-1.0, 0.5),
            Complex::new(0.0, -2.0),
            Complex::new(2.5, -1.5),
            Complex::new(-3.5, 2.5),
            Complex::new(1.1, -0.9),
            Complex::new(-0.8, 1.2),
        ];

        let hrtf = vec![
            Complex::new(0.5, 0.25),
            Complex::new(-1.0, 1.5),
            Complex::new(2.0, -0.5),
            Complex::new(0.75, 0.75),
            Complex::new(-0.5, 2.0),
            Complex::new(1.5, -1.0),
            Complex::new(0.9, 0.3),
            Complex::new(-1.1, 0.7),
        ];

        let initial = vec![
            Complex::new(0.1, 0.2),
            Complex::new(0.3, 0.4),
            Complex::new(0.5, 0.6),
            Complex::new(0.7, 0.8),
            Complex::new(0.9, 1.0),
            Complex::new(1.1, 1.2),
            Complex::new(1.3, 1.4),
            Complex::new(1.5, 1.6),
        ];

        // Scalar reference computation
        let mut expected = initial.clone();
        for i in 0..src.len() {
            expected[i] += src[i] * hrtf[i];
        }

        // SIMD computation
        let mut result = initial.clone();
        complex_mul_add_simd(&mut result, &src, &hrtf);

        // Compare results with tolerance for floating point errors
        const EPSILON: f32 = 1e-6;
        for i in 0..src.len() {
            assert!(
                (result[i].re - expected[i].re).abs() < EPSILON,
                "SIMD result[{}].re = {}, expected = {} (diff = {})",
                i,
                result[i].re,
                expected[i].re,
                (result[i].re - expected[i].re).abs()
            );
            assert!(
                (result[i].im - expected[i].im).abs() < EPSILON,
                "SIMD result[{}].im = {}, expected = {} (diff = {})",
                i,
                result[i].im,
                expected[i].im,
                (result[i].im - expected[i].im).abs()
            );
        }
    }

    #[test]
    fn test_simd_complex_mul_correctness() {
        // Test SIMD complex multiplication (without accumulation)
        use rustfft::num_complex::Complex;

        let src = vec![
            Complex::new(2.0, 3.0),
            Complex::new(-1.5, 2.5),
            Complex::new(0.5, -1.0),
            Complex::new(4.0, -2.0),
        ];

        let hrtf = vec![
            Complex::new(1.0, 0.5),
            Complex::new(2.0, -1.0),
            Complex::new(-0.5, 1.5),
            Complex::new(0.75, 0.25),
        ];

        // Scalar reference
        let expected: Vec<Complex<f32>> = src.iter().zip(hrtf.iter()).map(|(a, b)| a * b).collect();

        // SIMD computation
        let mut result = vec![Complex::new(0.0, 0.0); src.len()];
        complex_mul_simd(&mut result, &src, &hrtf);

        // Compare
        const EPSILON: f32 = 1e-6;
        for i in 0..src.len() {
            assert!(
                (result[i].re - expected[i].re).abs() < EPSILON,
                "SIMD result[{}].re = {}, expected = {}",
                i,
                result[i].re,
                expected[i].re
            );
            assert!(
                (result[i].im - expected[i].im).abs() < EPSILON,
                "SIMD result[{}].im = {}, expected = {}",
                i,
                result[i].im,
                expected[i].im
            );
        }
    }

    #[test]
    fn test_simd_edge_cases() {
        // Test edge cases: zeros, ones, conjugates
        use rustfft::num_complex::Complex;

        // Test 1: Multiply by zero
        let src = vec![
            Complex::new(1.0, 2.0),
            Complex::new(3.0, 4.0),
            Complex::new(5.0, 6.0),
            Complex::new(7.0, 8.0),
        ];
        let zero = vec![Complex::new(0.0, 0.0); 4];
        let mut result = src.clone();
        let input = result.clone();
        complex_mul_simd(&mut result, &input, &zero);
        for i in 0..4 {
            assert_eq!(result[i].re, 0.0);
            assert_eq!(result[i].im, 0.0);
        }

        // Test 2: Multiply by one (identity)
        let one = vec![Complex::new(1.0, 0.0); 4];
        let mut result = vec![Complex::new(0.0, 0.0); 4];
        complex_mul_simd(&mut result, &src, &one);
        for i in 0..4 {
            assert!((result[i].re - src[i].re).abs() < 1e-6);
            assert!((result[i].im - src[i].im).abs() < 1e-6);
        }

        // Test 3: Multiply by conjugate (should give real result)
        let a = Complex::new(3.0, 4.0);
        let a_conj = Complex::new(3.0, -4.0);
        let src = vec![a, a, a, a];
        let conj = vec![a_conj, a_conj, a_conj, a_conj];
        let mut result = vec![Complex::new(0.0, 0.0); 4];
        complex_mul_simd(&mut result, &src, &conj);

        // a * conj(a) = |a|^2 = 3^2 + 4^2 = 25
        for i in 0..4 {
            assert!((result[i].re - 25.0).abs() < 1e-5);
            assert!(result[i].im.abs() < 1e-5); // Should be approximately zero
        }
    }

    #[test]
    fn test_simd_large_buffer() {
        // Test with realistic FFT buffer sizes
        use rustfft::num_complex::Complex;

        for fft_size in [512, 1024, 2048, 4096] {
            let mut src = Vec::with_capacity(fft_size);
            let mut hrtf = Vec::with_capacity(fft_size);

            // Fill with test pattern
            for i in 0..fft_size {
                let phase = (i as f32) * 0.01;
                src.push(Complex::new(phase.cos(), phase.sin()));
                hrtf.push(Complex::new(0.5, 0.25));
            }

            // Scalar reference
            let mut expected = vec![Complex::new(0.1, 0.2); fft_size];
            for i in 0..fft_size {
                expected[i] += src[i] * hrtf[i];
            }

            // SIMD computation
            let mut result = vec![Complex::new(0.1, 0.2); fft_size];
            complex_mul_add_simd(&mut result, &src, &hrtf);

            // Verify all elements match
            for i in 0..fft_size {
                assert!(
                    (result[i].re - expected[i].re).abs() < 1e-5,
                    "FFT size {}, index {}: SIMD mismatch",
                    fft_size,
                    i
                );
                assert!(
                    (result[i].im - expected[i].im).abs() < 1e-5,
                    "FFT size {}, index {}: SIMD mismatch",
                    fft_size,
                    i
                );
            }
        }
    }

    #[test]
    fn test_simd_unaligned_sizes() {
        // Test with buffer sizes that don't align to SIMD width
        // This ensures the scalar remainder loop works correctly
        use rustfft::num_complex::Complex;

        for size in [1, 3, 5, 7, 9, 13, 17] {
            let src: Vec<Complex<f32>> = (0..size)
                .map(|i| Complex::new(i as f32, (i as f32) * 0.5))
                .collect();
            let hrtf: Vec<Complex<f32>> = (0..size)
                .map(|i| Complex::new(0.5, (i as f32) * 0.1))
                .collect();

            // Scalar reference
            let expected: Vec<Complex<f32>> =
                src.iter().zip(hrtf.iter()).map(|(a, b)| a * b).collect();

            // SIMD computation
            let mut result = vec![Complex::new(0.0, 0.0); size];
            complex_mul_simd(&mut result, &src, &hrtf);

            // Verify
            for i in 0..size {
                assert!(
                    (result[i].re - expected[i].re).abs() < 1e-6,
                    "Size {}, index {}: re mismatch",
                    size,
                    i
                );
                assert!(
                    (result[i].im - expected[i].im).abs() < 1e-6,
                    "Size {}, index {}: im mismatch",
                    size,
                    i
                );
            }
        }
    }

    // ============================================================================
    // Comprehensive Tests for complex_mul_inplace_simd
    // ============================================================================

    #[test]
    fn test_simd_complex_mul_inplace_correctness() {
        // Basic correctness test for in-place complex multiplication
        use rustfft::num_complex::Complex;

        let src = vec![
            Complex::new(2.0, 3.0),
            Complex::new(-1.5, 2.5),
            Complex::new(0.5, -1.0),
            Complex::new(4.0, -2.0),
        ];

        let hrtf = vec![
            Complex::new(1.0, 0.5),
            Complex::new(2.0, -1.0),
            Complex::new(-0.5, 1.5),
            Complex::new(0.75, 0.25),
        ];

        // Scalar reference
        let mut expected = src.clone();
        for i in 0..expected.len() {
            expected[i] *= hrtf[i];
        }

        // SIMD in-place computation
        let mut result = src.clone();
        complex_mul_inplace_simd(&mut result, &hrtf);

        const EPSILON: f32 = 1e-6;
        for i in 0..result.len() {
            assert!(
                (result[i].re - expected[i].re).abs() < EPSILON,
                "Index {}: re mismatch {} vs {}",
                i,
                result[i].re,
                expected[i].re
            );
            assert!(
                (result[i].im - expected[i].im).abs() < EPSILON,
                "Index {}: im mismatch {} vs {}",
                i,
                result[i].im,
                expected[i].im
            );
        }
    }

    #[test]
    fn test_simd_inplace_large_buffers() {
        // Test in-place multiplication with realistic FFT buffer sizes
        use rustfft::num_complex::Complex;

        for fft_size in [128, 256, 512, 1024, 2048] {
            let mut src: Vec<Complex<f32>> = (0..fft_size)
                .map(|i| {
                    let phase = (i as f32) * 0.01;
                    Complex::new(phase.cos(), phase.sin())
                })
                .collect();

            let hrtf: Vec<Complex<f32>> = (0..fft_size)
                .map(|i| Complex::new(0.5 + (i as f32) * 0.001, 0.25))
                .collect();

            // Scalar reference
            let mut expected = src.clone();
            for i in 0..fft_size {
                expected[i] *= hrtf[i];
            }

            // SIMD computation
            complex_mul_inplace_simd(&mut src, &hrtf);

            // Verify all elements match
            for i in 0..fft_size {
                assert!(
                    (src[i].re - expected[i].re).abs() < 1e-5,
                    "FFT size {}, index {}: re mismatch",
                    fft_size,
                    i
                );
                assert!(
                    (src[i].im - expected[i].im).abs() < 1e-5,
                    "FFT size {}, index {}: im mismatch",
                    fft_size,
                    i
                );
            }
        }
    }

    #[test]
    fn test_simd_inplace_unaligned() {
        // Test with sizes that don't align to SIMD width (4 for AVX2, 2 for NEON)
        use rustfft::num_complex::Complex;

        for size in [1, 2, 3, 5, 6, 7, 9, 10, 11, 15, 17, 19, 23] {
            let mut src: Vec<Complex<f32>> = (0..size)
                .map(|i| Complex::new((i as f32) * 0.5, (i as f32) * -0.3))
                .collect();

            let hrtf: Vec<Complex<f32>> = (0..size)
                .map(|i| Complex::new(1.0 + (i as f32) * 0.1, 0.5))
                .collect();

            // Scalar reference
            let mut expected = src.clone();
            for i in 0..size {
                expected[i] *= hrtf[i];
            }

            // SIMD computation
            complex_mul_inplace_simd(&mut src, &hrtf);

            // Verify
            for i in 0..size {
                assert!(
                    (src[i].re - expected[i].re).abs() < 1e-6,
                    "Size {}, index {}: re mismatch",
                    size,
                    i
                );
                assert!(
                    (src[i].im - expected[i].im).abs() < 1e-6,
                    "Size {}, index {}: im mismatch",
                    size,
                    i
                );
            }
        }
    }

    #[test]
    fn test_simd_inplace_edge_cases() {
        // Test edge cases: zeros, ones, conjugates, negative values
        use rustfft::num_complex::Complex;

        // Test 1: Multiply by zero (should zero out the buffer)
        let mut src = vec![
            Complex::new(1.0, 2.0),
            Complex::new(3.0, 4.0),
            Complex::new(5.0, 6.0),
            Complex::new(7.0, 8.0),
        ];
        let zero = vec![Complex::new(0.0, 0.0); 4];
        complex_mul_inplace_simd(&mut src, &zero);
        for i in 0..4 {
            assert!(src[i].re.abs() < 1e-6, "Expected zero, got {}", src[i].re);
            assert!(src[i].im.abs() < 1e-6, "Expected zero, got {}", src[i].im);
        }

        // Test 2: Multiply by one (should be identity)
        let original = vec![
            Complex::new(1.5, 2.5),
            Complex::new(-3.5, 4.5),
            Complex::new(5.5, -6.5),
            Complex::new(-7.5, -8.5),
        ];
        let mut src = original.clone();
        let one = vec![Complex::new(1.0, 0.0); 4];
        complex_mul_inplace_simd(&mut src, &one);
        for i in 0..4 {
            assert!((src[i].re - original[i].re).abs() < 1e-6);
            assert!((src[i].im - original[i].im).abs() < 1e-6);
        }

        // Test 3: Multiply by conjugate (should give real result with magnitude |a|^2)
        let a = Complex::new(3.0, 4.0);
        let a_conj = Complex::new(3.0, -4.0);
        let mut src = vec![a; 8];
        let conj = vec![a_conj; 8];
        complex_mul_inplace_simd(&mut src, &conj);
        // a * conj(a) = |a|^2 = 3^2 + 4^2 = 25
        for i in 0..8 {
            assert!(
                (src[i].re - 25.0).abs() < 1e-5,
                "Expected 25.0, got {}",
                src[i].re
            );
            assert!(src[i].im.abs() < 1e-5, "Expected ~0, got {}", src[i].im);
        }

        // Test 4: Multiply by i (rotation by 90 degrees)
        let mut src = vec![Complex::new(1.0, 0.0); 4];
        let i_val = vec![Complex::new(0.0, 1.0); 4];
        complex_mul_inplace_simd(&mut src, &i_val);
        for idx in 0..4 {
            assert!(src[idx].re.abs() < 1e-6, "Expected 0, got {}", src[idx].re);
            assert!(
                (src[idx].im - 1.0).abs() < 1e-6,
                "Expected 1, got {}",
                src[idx].im
            );
        }
    }

    #[test]
    fn test_simd_inplace_negative_values() {
        // Test with all negative values
        use rustfft::num_complex::Complex;

        let mut src = vec![
            Complex::new(-1.0, -2.0),
            Complex::new(-3.0, -4.0),
            Complex::new(-5.0, -6.0),
            Complex::new(-7.0, -8.0),
        ];

        let hrtf = vec![
            Complex::new(-0.5, -0.25),
            Complex::new(-1.0, -1.5),
            Complex::new(-2.0, 0.5),
            Complex::new(0.75, -0.75),
        ];

        // Scalar reference
        let mut expected = src.clone();
        for i in 0..expected.len() {
            expected[i] *= hrtf[i];
        }

        // SIMD computation
        complex_mul_inplace_simd(&mut src, &hrtf);

        const EPSILON: f32 = 1e-6;
        for i in 0..src.len() {
            assert!((src[i].re - expected[i].re).abs() < EPSILON);
            assert!((src[i].im - expected[i].im).abs() < EPSILON);
        }
    }

    // ============================================================================
    // Comprehensive Tests for compute_covariance_simd
    // ============================================================================

    #[test]
    fn test_covariance_basic_correctness() {
        // Test basic covariance computation against scalar reference
        use rustfft::num_complex::Complex;

        let left = vec![
            Complex::new(1.0, 2.0),
            Complex::new(3.0, 4.0),
            Complex::new(-1.0, 0.5),
            Complex::new(0.0, -2.0),
            Complex::new(2.5, -1.5),
            Complex::new(-3.5, 2.5),
            Complex::new(1.1, -0.9),
            Complex::new(-0.8, 1.2),
        ];

        let right = vec![
            Complex::new(0.5, 0.25),
            Complex::new(-1.0, 1.5),
            Complex::new(2.0, -0.5),
            Complex::new(0.75, 0.75),
            Complex::new(-0.5, 2.0),
            Complex::new(1.5, -1.0),
            Complex::new(0.9, 0.3),
            Complex::new(-1.1, 0.7),
        ];

        // Scalar reference
        let mut expected_xx = 0.0_f32;
        let mut expected_yy = 0.0_f32;
        let mut expected_xy = Complex::new(0.0, 0.0);
        for i in 0..left.len() {
            expected_xx += left[i].norm_sqr();
            expected_yy += right[i].norm_sqr();
            expected_xy += left[i] * right[i].conj();
        }

        // SIMD computation
        let (cov_xx, cov_yy, cov_xy) = compute_covariance_simd(&left, &right, 0, left.len());

        const EPSILON: f32 = 1e-5;
        assert!(
            (cov_xx - expected_xx).abs() < EPSILON,
            "cov_xx mismatch: {} vs {}",
            cov_xx,
            expected_xx
        );
        assert!(
            (cov_yy - expected_yy).abs() < EPSILON,
            "cov_yy mismatch: {} vs {}",
            cov_yy,
            expected_yy
        );
        assert!(
            (cov_xy.re - expected_xy.re).abs() < EPSILON,
            "cov_xy.re mismatch: {} vs {}",
            cov_xy.re,
            expected_xy.re
        );
        assert!(
            (cov_xy.im - expected_xy.im).abs() < EPSILON,
            "cov_xy.im mismatch: {} vs {}",
            cov_xy.im,
            expected_xy.im
        );
    }

    #[test]
    fn test_covariance_with_ranges() {
        // Test covariance computation with different start/end ranges
        use rustfft::num_complex::Complex;

        let left: Vec<Complex<f32>> = (0..32)
            .map(|i| Complex::new(i as f32 * 0.5, i as f32 * -0.3))
            .collect();
        let right: Vec<Complex<f32>> = (0..32)
            .map(|i| Complex::new(i as f32 * -0.4, i as f32 * 0.6))
            .collect();

        // Test various ranges
        for (start, end) in [(0, 8), (4, 12), (10, 20), (5, 25), (0, 32)] {
            // Scalar reference
            let mut expected_xx = 0.0_f32;
            let mut expected_yy = 0.0_f32;
            let mut expected_xy = Complex::new(0.0, 0.0);
            for i in start..end {
                expected_xx += left[i].norm_sqr();
                expected_yy += right[i].norm_sqr();
                expected_xy += left[i] * right[i].conj();
            }

            // SIMD computation
            let (cov_xx, cov_yy, cov_xy) = compute_covariance_simd(&left, &right, start, end);

            const EPSILON: f32 = 1e-4;
            assert!(
                (cov_xx - expected_xx).abs() < EPSILON,
                "Range [{}, {}): cov_xx mismatch: {} vs {}",
                start,
                end,
                cov_xx,
                expected_xx
            );
            assert!(
                (cov_yy - expected_yy).abs() < EPSILON,
                "Range [{}, {}): cov_yy mismatch: {} vs {}",
                start,
                end,
                cov_yy,
                expected_yy
            );
            assert!(
                (cov_xy.re - expected_xy.re).abs() < EPSILON,
                "Range [{}, {}): cov_xy.re mismatch: {} vs {}",
                start,
                end,
                cov_xy.re,
                expected_xy.re
            );
            assert!(
                (cov_xy.im - expected_xy.im).abs() < EPSILON,
                "Range [{}, {}): cov_xy.im mismatch: {} vs {}",
                start,
                end,
                cov_xy.im,
                expected_xy.im
            );
        }
    }

    #[test]
    fn test_covariance_large_buffers() {
        // Test with realistic FFT buffer sizes
        use rustfft::num_complex::Complex;

        for fft_size in [128, 256, 512, 1024, 2048, 4096] {
            let left: Vec<Complex<f32>> = (0..fft_size)
                .map(|i| {
                    let phase = (i as f32) * 0.01;
                    Complex::new(phase.cos(), phase.sin())
                })
                .collect();

            let right: Vec<Complex<f32>> = (0..fft_size)
                .map(|i| {
                    let phase = (i as f32) * 0.02;
                    Complex::new(phase.sin(), phase.cos())
                })
                .collect();

            // Scalar reference
            let mut expected_xx = 0.0_f32;
            let mut expected_yy = 0.0_f32;
            let mut expected_xy = Complex::new(0.0, 0.0);
            for i in 0..fft_size {
                expected_xx += left[i].norm_sqr();
                expected_yy += right[i].norm_sqr();
                expected_xy += left[i] * right[i].conj();
            }

            // SIMD computation
            let (cov_xx, cov_yy, cov_xy) = compute_covariance_simd(&left, &right, 0, fft_size);

            // Relative tolerance for larger sums
            let rel_epsilon = 1e-4;
            assert!(
                (cov_xx - expected_xx).abs() < expected_xx * rel_epsilon,
                "FFT size {}: cov_xx mismatch",
                fft_size
            );
            assert!(
                (cov_yy - expected_yy).abs() < expected_yy * rel_epsilon,
                "FFT size {}: cov_yy mismatch",
                fft_size
            );
            assert!(
                (cov_xy.re - expected_xy.re).abs() < expected_xy.re.abs() * rel_epsilon + 1e-5,
                "FFT size {}: cov_xy.re mismatch",
                fft_size
            );
            assert!(
                (cov_xy.im - expected_xy.im).abs() < expected_xy.im.abs() * rel_epsilon + 1e-5,
                "FFT size {}: cov_xy.im mismatch",
                fft_size
            );
        }
    }

    #[test]
    fn test_covariance_unaligned_ranges() {
        // Test with ranges that don't align to SIMD width
        use rustfft::num_complex::Complex;

        let left: Vec<Complex<f32>> = (0..50)
            .map(|i| Complex::new(i as f32 * 0.2, i as f32 * 0.3))
            .collect();
        let right: Vec<Complex<f32>> = (0..50)
            .map(|i| Complex::new(i as f32 * -0.1, i as f32 * 0.4))
            .collect();

        // Test with various unaligned ranges
        for (start, end) in [(0, 1), (0, 3), (1, 4), (2, 7), (5, 11), (10, 23), (15, 37)] {
            // Scalar reference
            let mut expected_xx = 0.0_f32;
            let mut expected_yy = 0.0_f32;
            let mut expected_xy = Complex::new(0.0, 0.0);
            for i in start..end {
                expected_xx += left[i].norm_sqr();
                expected_yy += right[i].norm_sqr();
                expected_xy += left[i] * right[i].conj();
            }

            // SIMD computation
            let (cov_xx, cov_yy, cov_xy) = compute_covariance_simd(&left, &right, start, end);

            const EPSILON: f32 = 1e-5;
            assert!(
                (cov_xx - expected_xx).abs() < EPSILON,
                "Range [{}, {}): cov_xx mismatch",
                start,
                end
            );
            assert!(
                (cov_yy - expected_yy).abs() < EPSILON,
                "Range [{}, {}): cov_yy mismatch",
                start,
                end
            );
            assert!(
                (cov_xy.re - expected_xy.re).abs() < EPSILON,
                "Range [{}, {}): cov_xy.re mismatch",
                start,
                end
            );
            assert!(
                (cov_xy.im - expected_xy.im).abs() < EPSILON,
                "Range [{}, {}): cov_xy.im mismatch",
                start,
                end
            );
        }
    }

    #[test]
    fn test_covariance_edge_cases() {
        // Test covariance with edge cases
        use rustfft::num_complex::Complex;

        // Test 1: All zeros
        let zero_left = vec![Complex::new(0.0, 0.0); 8];
        let zero_right = vec![Complex::new(0.0, 0.0); 8];
        let (cov_xx, cov_yy, cov_xy) = compute_covariance_simd(&zero_left, &zero_right, 0, 8);
        assert!(cov_xx.abs() < 1e-6, "Expected zero cov_xx");
        assert!(cov_yy.abs() < 1e-6, "Expected zero cov_yy");
        assert!(cov_xy.norm_sqr() < 1e-6, "Expected zero cov_xy");

        // Test 2: Real-only signals
        let real_left: Vec<Complex<f32>> = (0..8).map(|i| Complex::new(i as f32, 0.0)).collect();
        let real_right: Vec<Complex<f32>> = (0..8)
            .map(|i| Complex::new((i as f32) * 0.5, 0.0))
            .collect();
        let (cov_xx, cov_yy, cov_xy) = compute_covariance_simd(&real_left, &real_right, 0, 8);

        // cov_xy should be real (imaginary part should be ~0)
        assert!(
            cov_xy.im.abs() < 1e-5,
            "Expected real cov_xy for real signals"
        );

        // Verify values match scalar computation
        let mut expected_xx = 0.0;
        let mut expected_yy = 0.0;
        for i in 0..8 {
            expected_xx += (i * i) as f32;
            expected_yy += ((i as f32) * 0.5).powi(2);
        }
        assert!((cov_xx - expected_xx).abs() < 1e-5);
        assert!((cov_yy - expected_yy).abs() < 1e-5);

        // Test 3: Imaginary-only signals
        let imag_left: Vec<Complex<f32>> = (0..8).map(|i| Complex::new(0.0, i as f32)).collect();
        let imag_right: Vec<Complex<f32>> = (0..8)
            .map(|i| Complex::new(0.0, (i as f32) * 2.0))
            .collect();
        let (_cov_xx, _cov_yy, cov_xy) = compute_covariance_simd(&imag_left, &imag_right, 0, 8);

        // cov_xy should be real (because conj flips imaginary sign)
        assert!(
            cov_xy.im.abs() < 1e-5,
            "Expected real cov_xy for imaginary signals"
        );

        // Test 4: Single element range
        let single_left = vec![Complex::new(3.0, 4.0)];
        let single_right = vec![Complex::new(1.0, 2.0)];
        let (cov_xx, cov_yy, cov_xy) = compute_covariance_simd(&single_left, &single_right, 0, 1);
        assert!((cov_xx - 25.0).abs() < 1e-5); // 3^2 + 4^2 = 25
        assert!((cov_yy - 5.0).abs() < 1e-5); // 1^2 + 2^2 = 5
        // (3 + 4i) * (1 - 2i) = 3 - 6i + 4i - 8i^2 = 3 - 2i + 8 = 11 - 2i
        assert!((cov_xy.re - 11.0).abs() < 1e-5);
        assert!((cov_xy.im - (-2.0)).abs() < 1e-5);
    }

    // ============================================================================
    // Numerical Accuracy and Stress Tests
    // ============================================================================

    #[test]
    fn test_numerical_accuracy_small_values() {
        // Test with very small values to check for denormal handling
        use rustfft::num_complex::Complex;

        let small = 1e-20_f32;
        let src = vec![
            Complex::new(small, small),
            Complex::new(small * 2.0, small * 3.0),
            Complex::new(small * 4.0, small * 5.0),
            Complex::new(small * 6.0, small * 7.0),
        ];

        let hrtf = vec![
            Complex::new(1.0, 0.5),
            Complex::new(2.0, -1.0),
            Complex::new(-0.5, 1.5),
            Complex::new(0.75, 0.25),
        ];

        // Scalar reference
        let expected: Vec<Complex<f32>> = src.iter().zip(hrtf.iter()).map(|(a, b)| a * b).collect();

        // SIMD computation
        let mut result = vec![Complex::new(0.0, 0.0); src.len()];
        complex_mul_simd(&mut result, &src, &hrtf);

        // Relative tolerance for very small numbers
        for i in 0..src.len() {
            let re_diff = (result[i].re - expected[i].re).abs();
            let im_diff = (result[i].im - expected[i].im).abs();

            // For very small numbers, check relative error or absolute error for near-zero
            if expected[i].re.abs() > 1e-15 {
                assert!(re_diff / expected[i].re.abs() < 1e-3);
            } else {
                assert!(re_diff < 1e-25);
            }

            if expected[i].im.abs() > 1e-15 {
                assert!(im_diff / expected[i].im.abs() < 1e-3);
            } else {
                assert!(im_diff < 1e-25);
            }
        }
    }

    #[test]
    fn test_numerical_accuracy_large_values() {
        // Test with large values to check for overflow handling
        use rustfft::num_complex::Complex;

        let large = 1e10_f32;
        let src = vec![
            Complex::new(large, large * 0.5),
            Complex::new(large * 2.0, large * 1.5),
            Complex::new(large * 0.3, large * 0.7),
            Complex::new(large * 1.2, large * 0.8),
        ];

        let hrtf = vec![
            Complex::new(1e-5, 5e-6),
            Complex::new(2e-5, -1e-5),
            Complex::new(-5e-6, 1.5e-5),
            Complex::new(7.5e-6, 2.5e-6),
        ];

        // Scalar reference
        let mut expected = vec![Complex::new(0.0, 0.0); src.len()];
        for i in 0..src.len() {
            expected[i] = src[i] * hrtf[i];
        }

        // SIMD computation
        let mut result = vec![Complex::new(0.0, 0.0); src.len()];
        complex_mul_simd(&mut result, &src, &hrtf);

        // Relative tolerance
        for i in 0..src.len() {
            let re_rel_err = (result[i].re - expected[i].re).abs() / expected[i].re.abs().max(1.0);
            let im_rel_err = (result[i].im - expected[i].im).abs() / expected[i].im.abs().max(1.0);
            assert!(
                re_rel_err < 1e-5,
                "Index {}: re rel error too large: {}",
                i,
                re_rel_err
            );
            assert!(
                im_rel_err < 1e-5,
                "Index {}: im rel error too large: {}",
                i,
                im_rel_err
            );
        }
    }

    #[test]
    fn test_accumulation_accuracy() {
        // Test that repeated accumulation doesn't degrade accuracy significantly
        use rustfft::num_complex::Complex;

        let src = vec![
            Complex::new(0.1, 0.2),
            Complex::new(0.3, 0.4),
            Complex::new(0.5, 0.6),
            Complex::new(0.7, 0.8),
        ];

        let hrtf = vec![
            Complex::new(0.5, 0.25),
            Complex::new(-1.0, 1.5),
            Complex::new(2.0, -0.5),
            Complex::new(0.75, 0.75),
        ];

        // Scalar reference: accumulate 100 times
        let mut expected = vec![Complex::new(0.0, 0.0); src.len()];
        for _ in 0..100 {
            for i in 0..src.len() {
                expected[i] += src[i] * hrtf[i];
            }
        }

        // SIMD computation: accumulate 100 times
        let mut result = vec![Complex::new(0.0, 0.0); src.len()];
        for _ in 0..100 {
            complex_mul_add_simd(&mut result, &src, &hrtf);
        }

        // Check relative error
        const REL_EPSILON: f32 = 1e-4;
        for i in 0..src.len() {
            let re_abs_err = (result[i].re - expected[i].re).abs();
            let im_abs_err = (result[i].im - expected[i].im).abs();

            // Use relative error if expected value is large enough, otherwise use absolute error
            let re_err = if expected[i].re.abs() > 1e-6 {
                re_abs_err / expected[i].re.abs()
            } else {
                re_abs_err
            };
            let im_err = if expected[i].im.abs() > 1e-6 {
                im_abs_err / expected[i].im.abs()
            } else {
                im_abs_err
            };

            assert!(
                re_err < REL_EPSILON,
                "Index {}: re accumulated error too large: {} (abs: {}, expected: {})",
                i,
                re_err,
                re_abs_err,
                expected[i].re
            );
            assert!(
                im_err < REL_EPSILON,
                "Index {}: im accumulated error too large: {} (abs: {}, expected: {})",
                i,
                im_err,
                im_abs_err,
                expected[i].im
            );
        }
    }

    #[test]
    fn test_platform_specific_simd_widths() {
        // Test that SIMD code paths are correctly exercised
        use rustfft::num_complex::Complex;

        // Test sizes that exercise SIMD boundaries:
        // - AVX2 processes 4 complex at a time
        // - NEON processes 2 complex at a time
        let test_sizes = vec![
            1,  // No SIMD, scalar only
            2,  // NEON: 1 iteration, AVX2: scalar only
            3,  // NEON: 1 iteration + 1 scalar, AVX2: scalar only
            4,  // NEON: 2 iterations, AVX2: 1 iteration
            5,  // NEON: 2 iterations + 1 scalar, AVX2: 1 iteration + 1 scalar
            8,  // NEON: 4 iterations, AVX2: 2 iterations
            9,  // Mixed
            12, // NEON: 6 iterations, AVX2: 3 iterations
            16, // NEON: 8 iterations, AVX2: 4 iterations
        ];

        for size in test_sizes {
            let src: Vec<Complex<f32>> = (0..size)
                .map(|i| Complex::new(i as f32 * 0.3, i as f32 * -0.2))
                .collect();
            let hrtf: Vec<Complex<f32>> = (0..size)
                .map(|i| Complex::new(1.0 + i as f32 * 0.1, 0.5))
                .collect();

            // Test all three functions

            // 1. complex_mul_add_simd
            let mut result_add = vec![Complex::new(1.0, 2.0); size];
            let mut expected_add = result_add.clone();
            for i in 0..size {
                expected_add[i] += src[i] * hrtf[i];
            }
            complex_mul_add_simd(&mut result_add, &src, &hrtf);
            for i in 0..size {
                assert!(
                    (result_add[i].re - expected_add[i].re).abs() < 1e-6,
                    "mul_add size {}, index {}: re mismatch",
                    size,
                    i
                );
                assert!(
                    (result_add[i].im - expected_add[i].im).abs() < 1e-6,
                    "mul_add size {}, index {}: im mismatch",
                    size,
                    i
                );
            }

            // 2. complex_mul_simd
            let mut result_mul = vec![Complex::new(0.0, 0.0); size];
            let expected_mul: Vec<Complex<f32>> =
                src.iter().zip(hrtf.iter()).map(|(a, b)| a * b).collect();
            complex_mul_simd(&mut result_mul, &src, &hrtf);
            for i in 0..size {
                assert!(
                    (result_mul[i].re - expected_mul[i].re).abs() < 1e-6,
                    "mul size {}, index {}: re mismatch",
                    size,
                    i
                );
                assert!(
                    (result_mul[i].im - expected_mul[i].im).abs() < 1e-6,
                    "mul size {}, index {}: im mismatch",
                    size,
                    i
                );
            }

            // 3. complex_mul_inplace_simd
            let mut result_inplace = src.clone();
            let mut expected_inplace = src.clone();
            for i in 0..size {
                expected_inplace[i] *= hrtf[i];
            }
            complex_mul_inplace_simd(&mut result_inplace, &hrtf);
            for i in 0..size {
                assert!(
                    (result_inplace[i].re - expected_inplace[i].re).abs() < 1e-6,
                    "inplace size {}, index {}: re mismatch",
                    size,
                    i
                );
                assert!(
                    (result_inplace[i].im - expected_inplace[i].im).abs() < 1e-6,
                    "inplace size {}, index {}: im mismatch",
                    size,
                    i
                );
            }
        }
    }

    #[test]
    fn test_stress_test_random_data() {
        // Stress test with pseudo-random data
        use rustfft::num_complex::Complex;

        // Simple LCG for deterministic "random" values
        let mut seed = 12345_u32;
        let lcg = |s: &mut u32| -> f32 {
            *s = s.wrapping_mul(1103515245).wrapping_add(12345);
            ((*s / 65536) % 32768) as f32 / 32768.0 - 0.5
        };

        for size in [64, 128, 256, 512] {
            let src: Vec<Complex<f32>> = (0..size)
                .map(|_| Complex::new(lcg(&mut seed), lcg(&mut seed)))
                .collect();
            let hrtf: Vec<Complex<f32>> = (0..size)
                .map(|_| Complex::new(lcg(&mut seed), lcg(&mut seed)))
                .collect();

            // Scalar reference
            let expected: Vec<Complex<f32>> =
                src.iter().zip(hrtf.iter()).map(|(a, b)| a * b).collect();

            // SIMD computation
            let mut result = vec![Complex::new(0.0, 0.0); size];
            complex_mul_simd(&mut result, &src, &hrtf);

            // Verify
            for i in 0..size {
                assert!(
                    (result[i].re - expected[i].re).abs() < 1e-5,
                    "Stress test size {}, index {}: re mismatch",
                    size,
                    i
                );
                assert!(
                    (result[i].im - expected[i].im).abs() < 1e-5,
                    "Stress test size {}, index {}: im mismatch",
                    size,
                    i
                );
            }
        }
    }
}

// ============================================================================
// SIMD Covariance Calculation for ERB Band Processing
// ============================================================================
//
// Computes covariance statistics for two complex arrays (left/right channels):
//   cov_xx = sum(|left[i]|^2)
//   cov_yy = sum(|right[i]|^2)
//   cov_xy = sum(left[i] * conj(right[i]))
//
// These are used for PCA-based direct/ambient decomposition in the upmixer.

/// SIMD-accelerated covariance calculation for ERB bands
///
/// Returns (cov_xx, cov_yy, cov_xy) where:
/// - cov_xx: sum of left channel energy
/// - cov_yy: sum of right channel energy
/// - cov_xy: complex cross-correlation
pub fn compute_covariance_simd(
    left: &[Complex<f32>],
    right: &[Complex<f32>],
    start: usize,
    end: usize,
) -> (f32, f32, Complex<f32>) {
    assert_eq!(left.len(), right.len());
    assert!(end <= left.len());
    assert!(start < end);

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    let count = end - start;

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    {
        use std::arch::x86_64::*;

        let mut cov_xx;
        let mut cov_yy;
        let mut cov_xy = Complex::new(0.0, 0.0);

        // SIMD path: process 4 complex numbers at once
        let simd_len = (count / 4) * 4;
        let simd_end = start + simd_len;

        unsafe {
            let mut sum_xx = _mm256_setzero_ps();
            let mut sum_yy = _mm256_setzero_ps();
            let mut sum_xy_re = _mm256_setzero_ps();
            let _sum_xy_im = _mm256_setzero_ps();

            for i in (start..simd_end).step_by(4) {
                let left_ptr = left.as_ptr().add(i) as *const f32;
                let right_ptr = right.as_ptr().add(i) as *const f32;

                // Load 4 complex numbers: [re0, im0, re1, im1, re2, im2, re3, im3]
                let l = _mm256_loadu_ps(left_ptr);
                let r = _mm256_loadu_ps(right_ptr);

                // Compute norm_sqr: re^2 + im^2
                let l_sqr = _mm256_mul_ps(l, l);
                let r_sqr = _mm256_mul_ps(r, r);

                // Horizontal add pairs: [re0^2 + im0^2, re1^2 + im1^2, ...]
                let l_norm = _mm256_hadd_ps(l_sqr, l_sqr);
                let r_norm = _mm256_hadd_ps(r_sqr, r_sqr);

                sum_xx = _mm256_add_ps(sum_xx, l_norm);
                sum_yy = _mm256_add_ps(sum_yy, r_norm);

                // Compute cross-correlation: left * conj(right)
                let sign_mask = _mm256_set_ps(-0.0, 0.0, -0.0, 0.0, -0.0, 0.0, -0.0, 0.0);
                let r_conj = _mm256_xor_ps(r, sign_mask);

                // Complex multiplication: left * r_conj
                let l_re = _mm256_moveldup_ps(l);
                let l_im = _mm256_movehdup_ps(l);

                let ac_ad = _mm256_mul_ps(l_re, r_conj);
                let r_conj_swap = _mm256_shuffle_ps(r_conj, r_conj, 0b10110001);
                let bd_bc = _mm256_mul_ps(l_im, r_conj_swap);

                let result = _mm256_addsub_ps(ac_ad, bd_bc);

                // Accumulate real parts (even indices) and imaginary parts (odd indices)
                sum_xy_re = _mm256_add_ps(sum_xy_re, result);
            }

            // Horizontal reduction to scalars
            let xx_arr = std::mem::transmute::<__m256, [f32; 8]>(sum_xx);
            let yy_arr = std::mem::transmute::<__m256, [f32; 8]>(sum_yy);
            let xy_arr = std::mem::transmute::<__m256, [f32; 8]>(sum_xy_re);

            // Sum hadd results (each norm appears twice in each lane)
            // Lane 0: [norm0, norm1, norm0, norm1], Lane 1: [norm2, norm3, norm2, norm3]
            cov_xx = xx_arr[0] + xx_arr[1] + xx_arr[4] + xx_arr[5];
            cov_yy = yy_arr[0] + yy_arr[1] + yy_arr[4] + yy_arr[5];

            // Sum re (even) and im (odd) separately
            cov_xy.re = xy_arr[0] + xy_arr[2] + xy_arr[4] + xy_arr[6];
            cov_xy.im = xy_arr[1] + xy_arr[3] + xy_arr[5] + xy_arr[7];
        }

        // Scalar tail for remaining elements
        for i in simd_end..end {
            let l = left[i];
            let r = right[i];
            cov_xx += l.norm_sqr();
            cov_yy += r.norm_sqr();
            cov_xy += l * r.conj();
        }

        (cov_xx, cov_yy, cov_xy)
    }

    #[cfg(not(all(target_arch = "x86_64", target_feature = "avx2")))]
    {
        let mut cov_xx = 0.0_f32;
        let mut cov_yy = 0.0_f32;
        let mut cov_xy = Complex::new(0.0, 0.0);

        for i in start..end {
            let l = left[i];
            let r = right[i];
            cov_xx += l.norm_sqr();
            cov_yy += r.norm_sqr();
            cov_xy += l * r.conj();
        }

        (cov_xx, cov_yy, cov_xy)
    }
}

/// SIMD-optimized gain application for a single gain value
#[inline]
pub fn apply_gain_simd(buffer: &mut [f32], gain: f32) {
    let len = buffer.len();

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    {
        use std::arch::x86_64::*;
        let gain_vec = unsafe { _mm256_set1_ps(gain) };
        let simd_len = (len / 8) * 8;
        for i in (0..simd_len).step_by(8) {
            unsafe {
                let ptr = buffer.as_mut_ptr().add(i);
                let v = _mm256_loadu_ps(ptr);
                let res = _mm256_mul_ps(v, gain_vec);
                _mm256_storeu_ps(ptr, res);
            }
        }
        for sample in buffer.iter_mut().take(len).skip(simd_len) {
            *sample *= gain;
        }
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        use std::arch::aarch64::*;
        let gain_vec = unsafe { vdupq_n_f32(gain) };
        let simd_len = (len / 4) * 4;
        for i in (0..simd_len).step_by(4) {
            unsafe {
                let ptr = buffer.as_mut_ptr().add(i);
                let v = vld1q_f32(ptr);
                let res = vmulq_f32(v, gain_vec);
                vst1q_f32(ptr, res);
            }
        }
        for sample in buffer[simd_len..len].iter_mut() {
            *sample *= gain;
        }
    }

    #[cfg(not(any(
        all(target_arch = "x86_64", target_feature = "avx2"),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        for val in buffer.iter_mut() {
            *val *= gain;
        }
    }
}

/// SIMD-optimized per-channel gain application
#[inline]
pub fn apply_per_channel_gain_simd(buffer: &mut [f32], channels: usize, gains: &[f32]) {
    let len = buffer.len();
    let num_frames = len / channels;

    // This is harder to SIMD generically for any channel count.
    // We prioritize common channel counts (1, 2, 6, 8) or use scalar for now.
    // For stereo (channels == 2), we can optimize easily.
    if channels == 2 {
        #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
        {
            use std::arch::x86_64::*;
            let gains_vec = unsafe {
                _mm256_set_ps(
                    gains[1], gains[0], gains[1], gains[0], gains[1], gains[0], gains[1], gains[0],
                )
            };
            let simd_len = (num_frames / 4) * 4;
            for i in (0..simd_len).step_by(4) {
                unsafe {
                    let ptr = buffer.as_mut_ptr().add(i * 2);
                    let v = _mm256_loadu_ps(ptr);
                    let res = _mm256_mul_ps(v, gains_vec);
                    _mm256_storeu_ps(ptr, res);
                }
            }
            for i in simd_len..num_frames {
                buffer[i * 2] *= gains[0];
                buffer[i * 2 + 1] *= gains[1];
            }
            return;
        }

        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        {
            use std::arch::aarch64::*;
            let gains_vec = unsafe {
                let g = [gains[0], gains[1], gains[0], gains[1]];
                vld1q_f32(g.as_ptr())
            };
            let simd_len = (num_frames / 2) * 2;
            for i in (0..simd_len).step_by(2) {
                unsafe {
                    let ptr = buffer.as_mut_ptr().add(i * 2);
                    let v = vld1q_f32(ptr);
                    let res = vmulq_f32(v, gains_vec);
                    vst1q_f32(ptr, res);
                }
            }
            for i in simd_len..num_frames {
                buffer[i * 2] *= gains[0];
                buffer[i * 2 + 1] *= gains[1];
            }
            return;
        }
    }

    // Fallback scalar loop
    for frame in 0..num_frames {
        for ch in 0..channels {
            buffer[frame * channels + ch] *= gains[ch];
        }
    }
}

/// Fast inverse square root (1/sqrt(x)) using one Newton-Raphson iteration.
/// ~0.1% relative error — suitable for normalizing all-pass filter magnitudes.
#[inline(always)]
pub fn fast_inv_sqrt(x: f32) -> f32 {
    let half = 0.5 * x;
    let i = f32::to_bits(x);
    let i = 0x5f37_59df - (i >> 1); // Initial "magic number" estimate
    let y = f32::from_bits(i);
    y * (1.5 - half * y * y) // One Newton-Raphson refinement
}

/// SIMD-optimized peak detection (maximum absolute value)
#[inline]
pub fn find_max_abs_simd(samples: &[f32]) -> f32 {
    let len = samples.len();
    if len == 0 {
        return 0.0;
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    {
        use std::arch::x86_64::*;
        let mut max_vec = unsafe { _mm256_setzero_ps() };
        let abs_mask = unsafe { _mm256_set1_ps(-0.0) };
        let simd_len = (len / 8) * 8;

        for i in (0..simd_len).step_by(8) {
            unsafe {
                let ptr = samples.as_ptr().add(i);
                let v = _mm256_loadu_ps(ptr);
                let av = _mm256_andnot_ps(abs_mask, v);
                max_vec = _mm256_max_ps(max_vec, av);
            }
        }

        let mut max_val = 0.0_f32;
        unsafe {
            let arr = std::mem::transmute::<__m256, [f32; 8]>(max_vec);
            for &v in &arr {
                if v > max_val {
                    max_val = v;
                }
            }
        }

        for sample in samples.iter().take(len).skip(simd_len) {
            let v = sample.abs();
            if v > max_val {
                max_val = v;
            }
        }
        max_val
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        use std::arch::aarch64::*;
        let mut max_vec = unsafe { vdupq_n_f32(0.0) };
        let simd_len = (len / 4) * 4;

        for i in (0..simd_len).step_by(4) {
            unsafe {
                let ptr = samples.as_ptr().add(i);
                let v = vld1q_f32(ptr);
                let av = vabsq_f32(v);
                max_vec = vmaxq_f32(max_vec, av);
            }
        }

        let mut max_val = unsafe { vmaxvq_f32(max_vec) };

        for sample in &samples[simd_len..len] {
            let v = sample.abs();
            if v > max_val {
                max_val = v;
            }
        }
        max_val
    }

    #[cfg(not(any(
        all(target_arch = "x86_64", target_feature = "avx2"),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        let mut max_val = 0.0_f32;
        for &s in samples {
            let v = s.abs();
            if v > max_val {
                max_val = v;
            }
        }
        max_val
    }
}

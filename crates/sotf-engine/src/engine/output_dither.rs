/// Allocation-free triangular-PDF dither for the final float-to-integer boundary.
pub(crate) struct TpdfDither {
    state: u64,
}

impl TpdfDither {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9e37_79b9_7f4a_7c15
            } else {
                seed
            },
        }
    }

    #[inline(always)]
    fn uniform(&mut self) -> f64 {
        // xorshift64*: the generator is captured by the stream callback and
        // never shared, so advancing it needs no atomic or lock.
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        let value = self.state.wrapping_mul(0x2545_f491_4f6c_dd1d);
        ((value >> 40) as u32) as f64 * (1.0 / 16_777_216.0)
    }

    /// Return TPDF noise spanning ±1 target integer LSB.
    #[inline(always)]
    fn noise_lsb(&mut self) -> f64 {
        self.uniform() - self.uniform()
    }

    /// Quantize normalized floating-point audio to a signed PCM word with
    /// round-to-nearest TPDF dither and saturating rails.
    pub(crate) fn quantize_signed(&mut self, sample: f32, bits: u16) -> i32 {
        debug_assert!((2..=32).contains(&bits));
        let scale = 2.0f64.powi(i32::from(bits) - 1);
        let normalized = if sample.is_nan() {
            0.0
        } else {
            sample.clamp(-1.0, 1.0)
        };
        let quantized = (f64::from(normalized) * scale + self.noise_lsb()).round();
        let minimum = -scale;
        let maximum = scale - 1.0;
        quantized.clamp(minimum, maximum) as i32
    }
}

#[cfg(not(target_os = "ios"))]
pub(super) trait DitheredFromF32: cpal::SizedSample {
    fn from_f32_dithered(sample: f32, dither: &mut TpdfDither, enabled: bool) -> Self;
    fn silence() -> Self;
}

#[cfg(not(target_os = "ios"))]
macro_rules! impl_signed_dithered {
    ($ty:ty, $scale:expr) => {
        impl DitheredFromF32 for $ty {
            #[inline(always)]
            fn from_f32_dithered(sample: f32, dither: &mut TpdfDither, enabled: bool) -> Self {
                let noise = if enabled { dither.noise_lsb() } else { 0.0 };
                let normalized = if sample.is_nan() {
                    0.0
                } else {
                    sample.clamp(-1.0, 1.0)
                };
                let quantized = (f64::from(normalized) * $scale + noise).round();
                quantized.clamp(<$ty>::MIN as f64, <$ty>::MAX as f64) as $ty
            }

            #[inline(always)]
            fn silence() -> Self {
                0
            }
        }
    };
}

#[cfg(not(target_os = "ios"))]
macro_rules! impl_unsigned_dithered {
    ($ty:ty, $scale:expr) => {
        impl DitheredFromF32 for $ty {
            #[inline(always)]
            fn from_f32_dithered(sample: f32, dither: &mut TpdfDither, enabled: bool) -> Self {
                let noise = if enabled { dither.noise_lsb() } else { 0.0 };
                let normalized = if sample.is_nan() {
                    0.0
                } else {
                    sample.clamp(-1.0, 1.0)
                };
                let quantized = (f64::from(normalized) * $scale + $scale + noise).round();
                quantized.clamp(<$ty>::MIN as f64, <$ty>::MAX as f64) as $ty
            }

            #[inline(always)]
            fn silence() -> Self {
                (1 as $ty) << (<$ty>::BITS - 1)
            }
        }
    };
}

#[cfg(not(target_os = "ios"))]
impl_signed_dithered!(i16, 32_768.0f64);
#[cfg(not(target_os = "ios"))]
impl_signed_dithered!(i32, 2_147_483_648.0f64);
#[cfg(not(target_os = "ios"))]
impl_unsigned_dithered!(u16, 32_768.0f64);
#[cfg(not(target_os = "ios"))]
impl_unsigned_dithered!(u32, 2_147_483_648.0f64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tpdf_is_deterministic_for_a_seed() {
        let mut first = TpdfDither::new(42);
        let mut second = TpdfDither::new(42);

        for _ in 0..128 {
            assert_eq!(first.noise_lsb(), second.noise_lsb());
        }
    }

    #[test]
    fn tpdf_is_zero_mean_and_bounded_to_one_lsb() {
        let mut dither = TpdfDither::new(0x1234_5678_9abc_def0);
        let mut sum = 0.0f64;
        let samples = 100_000;

        for _ in 0..samples {
            let value = dither.noise_lsb();
            assert!(value.abs() <= 1.0);
            sum += value;
        }

        assert!((sum / samples as f64).abs() < 0.01);
    }

    #[test]
    fn signed_pcm_quantizer_supports_16_and_24_bit_outputs() {
        for bits in [16, 24] {
            let mut dither = TpdfDither::new(u64::from(bits));
            let scale = 1i32 << (bits - 1);
            let mut below = false;
            let mut above = false;
            for _ in 0..4096 {
                let value = dither.quantize_signed(0.0, bits);
                below |= value < 0;
                above |= value > 0;
                assert!((-scale..scale).contains(&value));
            }
            assert!(below && above);
            assert_eq!(dither.quantize_signed(1.0, bits), scale - 1);
            assert!(
                matches!(dither.quantize_signed(-1.0, bits), v if v == -scale || v == -scale + 1)
            );
        }
    }

    #[cfg(not(target_os = "ios"))]
    macro_rules! conversion_tests {
        ($name:ident, $ty:ty, $silence:expr, $scale:expr) => {
            mod $name {
                use super::*;

                #[test]
                fn dither_reaches_adjacent_codes_at_silence() {
                    let mut dither = TpdfDither::new(7);
                    let mut below = false;
                    let mut above = false;
                    for _ in 0..4096 {
                        let value = <$ty>::from_f32_dithered(0.0, &mut dither, true) as i128;
                        below |= value < $silence;
                        above |= value > $silence;
                    }
                    assert!(below && above);
                }

                #[test]
                fn fractional_lsb_conversion_is_unbiased() {
                    let mut dither = TpdfDither::new(11);
                    let input = (0.25f64 / $scale) as f32;
                    let mut sum = 0.0;
                    let samples = 100_000;
                    for _ in 0..samples {
                        sum += <$ty>::from_f32_dithered(input, &mut dither, true) as f64
                            - $silence as f64;
                    }
                    assert!((sum / samples as f64 - 0.25).abs() < 0.02);
                }

                #[test]
                fn rails_saturate_and_mute_is_exact() {
                    let mut dither = TpdfDither::new(13);
                    for _ in 0..1024 {
                        let positive = <$ty>::from_f32_dithered(1.0, &mut dither, true);
                        let negative = <$ty>::from_f32_dithered(-1.0, &mut dither, true);
                        assert_eq!(positive, <$ty>::MAX);
                        assert!(negative == <$ty>::MIN || negative == <$ty>::MIN.saturating_add(1));
                        assert_eq!(
                            <$ty>::from_f32_dithered(0.0, &mut dither, false),
                            <$ty as DitheredFromF32>::silence()
                        );
                    }
                }

                #[test]
                fn nan_stays_at_silence() {
                    let mut dither = TpdfDither::new(17);
                    for _ in 0..1024 {
                        let value = <$ty>::from_f32_dithered(f32::NAN, &mut dither, true) as i128;
                        assert!((value - $silence).abs() <= 1);
                    }
                }
            }
        };
    }

    #[cfg(not(target_os = "ios"))]
    conversion_tests!(i16_conversion, i16, 0i128, 32_768.0f64);
    #[cfg(not(target_os = "ios"))]
    conversion_tests!(i32_conversion, i32, 0i128, 2_147_483_648.0f64);
    #[cfg(not(target_os = "ios"))]
    conversion_tests!(u16_conversion, u16, 32_768i128, 32_768.0f64);
    #[cfg(not(target_os = "ios"))]
    conversion_tests!(u32_conversion, u32, 2_147_483_648i128, 2_147_483_648.0f64);
}

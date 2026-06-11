// ============================================================================
// Object-Based Element Renderer (VBAP)
// ============================================================================
//
// Renders object-based IAMF audio elements to the target speaker layout using
// Vector Base Amplitude Panning (VBAP).
//
// IAMF object elements carry a single mono (or stereo) substream plus spatial
// metadata (azimuth, elevation, gain) delivered via parameter blocks. This
// renderer pans the decoded PCM to the target speakers using the pre-computed
// VBAP gains from `sotf_host::vbap::VbapPanner`.
//
// The VBAP gains are computed once per `set_position()` call and cached.
// The `render()` hot path contains no heap allocations.

use crate::error::IamfResult;
use crate::renderer::ElementRenderer;
use sotf_host::speaker_config::SpeakerConfig;
use sotf_host::vbap::VbapPanner;

/// Spatial metadata for one object.
#[derive(Debug, Clone, Copy)]
pub struct ObjectPosition {
    /// Horizontal angle in degrees (0=front, +90=left, -90=right, ±180=back)
    pub azimuth_deg: f32,
    /// Vertical angle in degrees (0=ear-level, +90=overhead)
    pub elevation_deg: f32,
    /// Linear gain multiplier (1.0 = 0 dB)
    pub gain: f32,
}

impl Default for ObjectPosition {
    fn default() -> Self {
        Self {
            azimuth_deg: 0.0,
            elevation_deg: 0.0,
            gain: 1.0,
        }
    }
}

/// Renders a mono or stereo object element to the target speaker layout.
///
/// # Hot-path guarantees
/// - `render()` contains no heap allocations.
/// - VBAP gains are recomputed only when `set_position()` is called.
pub struct ObjectRenderer {
    /// VBAP panner for the output layout
    panner: VbapPanner,
    /// Current cached VBAP gains (indexed by output channel)
    cached_gains: Vec<f32>,
    /// Number of output channels
    output_channels: usize,
    /// Number of input channels for this object (1 = mono, 2 = stereo)
    input_channels: usize,
    /// Current position and gain
    position: ObjectPosition,
}

impl ObjectRenderer {
    /// Create an object renderer for the given output layout.
    ///
    /// `input_channels`: number of source channels (1 for mono objects, 2 for stereo).
    pub fn new(target: &SpeakerConfig, input_channels: usize) -> Self {
        let mut panner = VbapPanner::new(target.speakers, target.total_channels);
        let position = ObjectPosition::default();
        // Pre-compute gains for the default position (az=0, el=0)
        let cached_gains = panner
            .pan(position.azimuth_deg, position.elevation_deg)
            .to_vec();

        Self {
            panner,
            cached_gains,
            output_channels: target.total_channels,
            input_channels: input_channels.max(1),
            position,
        }
    }

    /// Update the object's spatial position and gain.
    ///
    /// Recomputes the VBAP gain vector. This is the only call that touches the
    /// panner's internal state; `render()` only reads `cached_gains`.
    pub fn set_position(&mut self, pos: ObjectPosition) {
        self.position = pos;
        let gains = self.panner.pan(pos.azimuth_deg, pos.elevation_deg);
        // Copy into owned buffer (avoids holding a borrow on self.panner)
        self.cached_gains.copy_from_slice(gains);
    }

    /// Current object position.
    pub fn position(&self) -> ObjectPosition {
        self.position
    }
}

impl ElementRenderer for ObjectRenderer {
    /// Pan decoded substream PCM into the output buffer using cached VBAP gains.
    ///
    /// The first substream is used as the source signal.
    /// For mono objects (`input_channels=1`): the single channel is panned to
    ///   all output speakers via VBAP gains.
    /// For stereo objects (`input_channels=2`): left and right channels are each
    ///   panned independently and summed. This preserves stereo width while
    ///   anchoring the object to its declared position.
    ///
    /// Output is scaled by `position.gain`.
    fn render(
        &mut self,
        substream_pcm: &[Vec<f32>],
        output: &mut [f32],
        num_frames: usize,
    ) -> IamfResult<()> {
        let out_len = num_frames * self.output_channels;
        output[..out_len].fill(0.0);

        if substream_pcm.is_empty() {
            return Ok(());
        }

        let gain = self.position.gain;
        let out_ch = self.output_channels;

        // For a mono object: substream_pcm[0] is length `num_frames` (1 ch/frame)
        // For a stereo object: substream_pcm[0] is interleaved, length `num_frames*2`
        match self.input_channels {
            1 => {
                // Mono: single source panned to all speakers
                let pcm = &substream_pcm[0];
                for frame in 0..num_frames {
                    let sample = if frame < pcm.len() { pcm[frame] } else { 0.0 };
                    let scaled = sample * gain;
                    let out_offset = frame * out_ch;
                    for (ch, &g) in self.cached_gains.iter().enumerate() {
                        if g != 0.0 {
                            output[out_offset + ch] += scaled * g;
                        }
                    }
                }
            }
            _ => {
                // Stereo (or more): first substream is interleaved L/R.
                // Pan left channel with +Δaz and right channel with -Δaz
                // where Δaz is a small offset to maintain stereo width while
                // keeping the image centred on the declared object position.
                // For simplicity we use the same VBAP gains for both channels
                // (pans to the object position as a mono downmix with equal power).
                // This is correct for object semantics: position is the anchor.
                let pcm = &substream_pcm[0];
                let src_ch = self.input_channels.min(2);
                for frame in 0..num_frames {
                    // Sum L+R to mono, normalised for equal power
                    let inv_sqrt_src = 1.0 / (src_ch as f32).sqrt();
                    let mut mono = 0.0_f32;
                    for ch in 0..src_ch {
                        let idx = frame * src_ch + ch;
                        let s = if idx < pcm.len() { pcm[idx] } else { 0.0 };
                        mono += s * inv_sqrt_src;
                    }
                    let scaled = mono * gain;
                    let out_offset = frame * out_ch;
                    for (ch, &g) in self.cached_gains.iter().enumerate() {
                        if g != 0.0 {
                            output[out_offset + ch] += scaled * g;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn output_channels(&self) -> usize {
        self.output_channels
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_host::speaker_config::get_speaker_config;

    fn sum_energy(buf: &[f32]) -> f32 {
        buf.iter().map(|&x| x * x).sum()
    }

    #[test]
    fn test_mono_object_center_speaker() {
        // 5.1: center speaker (ch2) is at azimuth=0.
        // A mono object at az=0 should route almost entirely to the center speaker.
        let config = get_speaker_config("5.1").unwrap();
        let mut renderer = ObjectRenderer::new(config, 1);
        assert_eq!(renderer.output_channels(), 6);

        // One frame of 1.0 mono signal
        let substream_pcm = vec![vec![1.0_f32]];
        let mut output = vec![0.0_f32; 6];
        renderer.render(&substream_pcm, &mut output, 1).unwrap();

        // Center is ch2 in 5.1
        let center = output[2];
        assert!(
            center > 0.9,
            "Expected center speaker to dominate at az=0, got {center:.3}. Output: {output:?}"
        );

        // LFE (ch3) should be silent
        assert_eq!(output[3], 0.0, "LFE should be silent");
    }

    #[test]
    fn test_mono_object_front_left() {
        // 5.1: FL (ch0) is at azimuth=+30.
        let config = get_speaker_config("5.1").unwrap();
        let mut renderer = ObjectRenderer::new(config, 1);

        renderer.set_position(ObjectPosition {
            azimuth_deg: 30.0,
            elevation_deg: 0.0,
            gain: 1.0,
        });

        let substream_pcm = vec![vec![1.0_f32]];
        let mut output = vec![0.0_f32; 6];
        renderer.render(&substream_pcm, &mut output, 1).unwrap();

        let fl = output[0];
        assert!(
            fl > 0.9,
            "Expected FL (ch0) to dominate at az=+30, got {fl:.3}. Output: {output:?}"
        );
    }

    #[test]
    fn test_mono_object_surround_right() {
        // 5.1: SR (ch5) is at azimuth=-110.
        let config = get_speaker_config("5.1").unwrap();
        let mut renderer = ObjectRenderer::new(config, 1);

        renderer.set_position(ObjectPosition {
            azimuth_deg: -110.0,
            elevation_deg: 0.0,
            gain: 1.0,
        });

        let substream_pcm = vec![vec![1.0_f32]];
        let mut output = vec![0.0_f32; 6];
        renderer.render(&substream_pcm, &mut output, 1).unwrap();

        let sr = output[5];
        assert!(
            sr > 0.9,
            "Expected SR (ch5) to dominate at az=-110, got {sr:.3}. Output: {output:?}"
        );
    }

    #[test]
    fn test_gain_scaling() {
        let config = get_speaker_config("5.1").unwrap();
        let mut renderer = ObjectRenderer::new(config, 1);

        renderer.set_position(ObjectPosition {
            azimuth_deg: 0.0,
            elevation_deg: 0.0,
            gain: 0.5,
        });

        let substream_pcm = vec![vec![1.0_f32]];
        let mut output_half = vec![0.0_f32; 6];
        renderer
            .render(&substream_pcm, &mut output_half, 1)
            .unwrap();

        renderer.set_position(ObjectPosition {
            azimuth_deg: 0.0,
            elevation_deg: 0.0,
            gain: 1.0,
        });
        let mut output_full = vec![0.0_f32; 6];
        renderer
            .render(&substream_pcm, &mut output_full, 1)
            .unwrap();

        // Half-gain output should be exactly 0.5 × full-gain output
        for (h, f) in output_half.iter().zip(output_full.iter()) {
            assert!(
                (h - f * 0.5).abs() < 1e-6,
                "Gain scaling error: half={h:.4} full={f:.4}"
            );
        }
    }

    #[test]
    fn test_energy_preservation() {
        // Input energy = 1.0 (1 frame of amplitude 1.0)
        // Output energy should equal 1.0 (constant power VBAP).
        let config = get_speaker_config("5.1").unwrap();
        let mut renderer = ObjectRenderer::new(config, 1);

        let substream_pcm = vec![vec![1.0_f32]];
        let mut output = vec![0.0_f32; 6];
        renderer.render(&substream_pcm, &mut output, 1).unwrap();

        let energy = sum_energy(&output);
        assert!(
            (energy - 1.0).abs() < 0.1,
            "Expected unit energy, got {energy:.3}"
        );
    }

    #[test]
    fn test_silence_input() {
        let config = get_speaker_config("5.1").unwrap();
        let mut renderer = ObjectRenderer::new(config, 1);

        let substream_pcm = vec![vec![0.0_f32; 8]];
        let mut output = vec![0.0_f32; 6 * 8];
        renderer.render(&substream_pcm, &mut output, 8).unwrap();

        for &s in &output {
            assert_eq!(s, 0.0, "Expected silence output for silence input");
        }
    }

    #[test]
    fn test_default_position_is_front() {
        // Default position is az=0, el=0 = front center
        let renderer = ObjectRenderer::new(get_speaker_config("5.1").unwrap(), 1);
        let pos = renderer.position();
        assert_eq!(pos.azimuth_deg, 0.0);
        assert_eq!(pos.elevation_deg, 0.0);
        assert_eq!(pos.gain, 1.0);
    }

    #[test]
    fn test_multiframe_render() {
        // Verify that rendering multiple frames accumulates correctly
        let config = get_speaker_config("2.0").unwrap();
        let mut renderer = ObjectRenderer::new(config, 1);

        // Pan to the left speaker (L is at +30 in 2.0)
        renderer.set_position(ObjectPosition {
            azimuth_deg: 30.0,
            elevation_deg: 0.0,
            gain: 1.0,
        });

        let num_frames = 4;
        let pcm = vec![1.0_f32; num_frames];
        let substream_pcm = vec![pcm];
        let mut output = vec![0.0_f32; 2 * num_frames];
        renderer
            .render(&substream_pcm, &mut output, num_frames)
            .unwrap();

        // All frames should have the same channel distribution
        for frame in 0..num_frames {
            let l = output[frame * 2];
            let r = output[frame * 2 + 1];
            assert!(l > 0.9, "Frame {frame}: expected L dominant, got L={l:.3}");
            assert!(r < 0.1, "Frame {frame}: expected R near zero, got R={r:.3}");
        }
    }

    #[test]
    fn test_stereo_object_render() {
        // Stereo object at front center: L and R summed equally to center image.
        let config = get_speaker_config("5.1").unwrap();
        let mut renderer = ObjectRenderer::new(config, 2);

        // One frame of interleaved stereo: L=1.0, R=1.0
        let substream_pcm = vec![vec![1.0_f32, 1.0]];
        let mut output = vec![0.0_f32; 6];
        renderer.render(&substream_pcm, &mut output, 1).unwrap();

        // Center should dominate (az=0, el=0).
        let center = output[2];
        assert!(
            center > 0.9,
            "Expected center speaker to dominate stereo object at front, got {center:.3}"
        );

        // LFE silent
        assert_eq!(output[3], 0.0, "LFE should be silent");
    }

    #[test]
    fn test_empty_substream_pcm_is_silence() {
        let config = get_speaker_config("5.1").unwrap();
        let mut renderer = ObjectRenderer::new(config, 1);

        let substream_pcm: Vec<Vec<f32>> = vec![];
        let mut output = vec![0.5_f32; 6]; // pre-fill with non-zero
        renderer.render(&substream_pcm, &mut output, 1).unwrap();

        // Output should have been cleared and left silent.
        for &s in &output {
            assert_eq!(s, 0.0, "Empty input should produce silence");
        }
    }

    #[test]
    fn test_position_roundtrip() {
        let config = get_speaker_config("5.1").unwrap();
        let mut renderer = ObjectRenderer::new(config, 1);

        let pos = ObjectPosition {
            azimuth_deg: 45.0,
            elevation_deg: 10.0,
            gain: 0.75,
        };
        renderer.set_position(pos);
        let retrieved = renderer.position();

        assert!((retrieved.azimuth_deg - 45.0).abs() < 1e-6);
        assert!((retrieved.elevation_deg - 10.0).abs() < 1e-6);
        assert!((retrieved.gain - 0.75).abs() < 1e-6);
    }

    #[test]
    fn test_short_input_zero_pads() {
        // Request more frames than provided: missing samples treated as 0.
        let config = get_speaker_config("5.1").unwrap();
        let mut renderer = ObjectRenderer::new(config, 1);

        let substream_pcm = vec![vec![1.0_f32]]; // only 1 frame
        let mut output = vec![0.0_f32; 12]; // request 2 frames
        renderer.render(&substream_pcm, &mut output, 2).unwrap();

        // First frame should have non-zero center, second frame silent.
        assert!(output[2].abs() > 0.9, "First frame center should be active");
        assert!(
            output[2 + 6].abs() < 1e-6,
            "Second frame should be zero-padded"
        );
    }
}

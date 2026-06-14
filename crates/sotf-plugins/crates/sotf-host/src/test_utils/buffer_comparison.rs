/// Utilities for comparing audio buffers.
pub struct BufferComparison;

impl BufferComparison {
    pub fn compare_rms(buf1: &[f32], buf2: &[f32], threshold: f32) -> bool {
        if buf1.len() != buf2.len() {
            return false;
        }
        if buf1.is_empty() {
            return true;
        }

        let mut sum_sq_diff = 0.0;
        for (s1, s2) in buf1.iter().zip(buf2.iter()) {
            let diff = s1 - s2;
            sum_sq_diff += diff * diff;
        }
        let rms_diff = (sum_sq_diff / buf1.len() as f32).sqrt();
        rms_diff < threshold
    }

    pub fn compare_bit_accurate(buf1: &[f32], buf2: &[f32]) -> bool {
        buf1 == buf2
    }
}

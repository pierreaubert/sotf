// ============================================================================
// Time-Domain Transient Suppressor (De-clicker)
// ============================================================================
//
// This module implements a simple time-domain transient suppressor to remove
// impulsive noise (clicks/pops) before spectral processing.
//
// Algorithm:
// 1. Tracks the local energy envelope of the signal.
// 2. Detects sudden spikes that exceed the local envelope by a significant factor.
// 3. Attenuates these spikes to match the local envelope.

pub struct TransientSuppressor {
    channels: usize,
    // Per-channel state
    last_samples: Vec<f32>,
    envelope: Vec<f32>,
    
    // Parameters
    threshold_factor: f32, // How many times above envelope to trigger
    decay: f32,           // Envelope decay factor
}

impl TransientSuppressor {
    pub fn new(channels: usize) -> Self {
        Self {
            channels,
            last_samples: vec![0.0; channels],
            envelope: vec![0.0; channels],
            threshold_factor: 5.0, // Trigger if > 5x local average
            decay: 0.995,         // Slower decay to hold envelope longer
        }
    }

    pub fn reset(&mut self) {
        self.last_samples.fill(0.0);
        self.envelope.fill(0.0);
    }

    /// Process interleaved audio buffer in-place
    pub fn process(&mut self, buffer: &mut [f32]) {
        for frame in buffer.chunks_mut(self.channels) {
            for (ch, sample) in frame.iter_mut().enumerate() {
                let abs_sample = sample.abs();
                
                // Initialize envelope on first sample or silence
                if self.envelope[ch] == 0.0 {
                    self.envelope[ch] = abs_sample;
                }

                let current_envelope = self.envelope[ch];

                // Detect click: sudden large value above envelope
                // We add a small floor to envelope to avoid triggering on silence/noise floor
                let threshold = current_envelope * self.threshold_factor + 0.001;
                
                let mut processed_abs = abs_sample;

                if abs_sample > threshold {
                    // It's a pop/click. Clamp it.
                    let sign = if *sample >= 0.0 { 1.0 } else { -1.0 };
                    *sample = sign * threshold;
                    processed_abs = threshold;
                    
                    // During a click, we don't want to spike the envelope, 
                    // but we might want to let it rise slightly in case it's a valid loud attack.
                    // For now, we'll just treat the clamped value as the signal for envelope tracking.
                }

                // Update envelope
                if processed_abs > self.envelope[ch] {
                    // Attack
                    self.envelope[ch] = processed_abs;
                } else {
                    // Decay
                    self.envelope[ch] = self.envelope[ch] * self.decay + processed_abs * (1.0 - self.decay);
                }
                
                self.last_samples[ch] = *sample;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transient_suppression() {
        let mut suppressor = TransientSuppressor::new(1);
        
        // Create a buffer with a steady signal and a huge spike
        let mut buffer = Vec::new();
        
        // Steady state (0.1)
        for _ in 0..100 {
            buffer.push(0.1);
        }
        
        // Spike (2.0) - 20x the signal
        buffer.push(2.0);
        
        // Return to steady state
        for _ in 0..100 {
            buffer.push(0.1);
        }

        let original_spike = buffer[100];
        let original_first = buffer[0];
        
        suppressor.process(&mut buffer);
        
        // The spike should be attenuated
        let processed_spike = buffer[100];
        assert!(processed_spike < original_spike, "Spike was not attenuated: {} -> {}", original_spike, processed_spike);
        
        // It should still be higher than steady state (clamped to threshold)
        // Threshold approx 0.1 * 5 + 0.001 = 0.501
        assert!(processed_spike > 0.1, "Spike was attenuated too much: {}", processed_spike);
        assert!(processed_spike < 1.0, "Spike was not attenuated enough: {}", processed_spike);

        // Steady state should be unaffected
        assert!((buffer[0] - original_first).abs() < 1e-6, "First sample affected: {} vs {}", buffer[0], original_first);
    }
}

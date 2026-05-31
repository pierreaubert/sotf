// ============================================================================
// Recording — Multi-track audio recording with punch-in/out
// ============================================================================

use std::path::PathBuf;

/// Configuration for recording on a track.
#[derive(Debug, Clone)]
pub struct RecordingConfig {
    /// Output directory for recorded files
    pub output_dir: PathBuf,
    /// Number of channels to record
    pub channels: usize,
    /// Sample rate
    pub sample_rate: u32,
    /// Optional punch-in point (samples). None = record from current position.
    pub punch_in: Option<u64>,
    /// Optional punch-out point (samples). None = record until stopped.
    pub punch_out: Option<u64>,
}

/// State of a recording session on a single track.
pub struct RecordingSession {
    /// WAV writer for the recorded file
    writer: hound::WavWriter<std::io::BufWriter<std::fs::File>>,
    /// Path to the output file
    pub output_path: PathBuf,
    /// Number of channels being recorded
    pub channels: usize,
    /// Sample rate
    pub sample_rate: u32,
    /// Punch-in point in samples (None = record immediately)
    pub punch_in: Option<u64>,
    /// Punch-out point in samples (None = manual stop)
    pub punch_out: Option<u64>,
    /// Number of frames written so far
    pub frames_written: u64,
    /// Whether the session is actively writing (between punch-in and punch-out)
    pub active: bool,
    /// Timeline position when recording started
    pub start_position: u64,
}

impl RecordingSession {
    /// Create a new recording session and open the output WAV file.
    pub fn new(config: &RecordingConfig, start_position: u64) -> Result<Self, String> {
        std::fs::create_dir_all(&config.output_dir)
            .map_err(|e| format!("Failed to create output dir: {e}"))?;

        let filename = format!(
            "recording_{}.wav",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        );
        let output_path = config.output_dir.join(&filename);

        let spec = hound::WavSpec {
            channels: config.channels as u16,
            sample_rate: config.sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };

        let writer = hound::WavWriter::create(&output_path, spec)
            .map_err(|e| format!("Failed to create recording file: {e}"))?;

        let active = config.punch_in.is_none(); // Active immediately if no punch-in

        Ok(Self {
            writer,
            output_path,
            channels: config.channels,
            sample_rate: config.sample_rate,
            punch_in: config.punch_in,
            punch_out: config.punch_out,
            frames_written: 0,
            active,
            start_position,
        })
    }

    /// Write audio samples to the recording.
    ///
    /// `position` is the current timeline position in samples.
    /// Respects punch-in/out boundaries.
    pub fn write_block(
        &mut self,
        samples: &[f32],
        position: u64,
        num_frames: usize,
    ) -> Result<(), String> {
        // Check punch-in
        if let Some(punch_in) = self.punch_in
            && !self.active
            && position + num_frames as u64 > punch_in
        {
            self.active = true;
        }

        // Check punch-out
        if let Some(punch_out) = self.punch_out
            && position >= punch_out
        {
            self.active = false;
            return Ok(());
        }

        if !self.active {
            return Ok(());
        }

        // Determine which samples to write (handle partial punch-in/out within block)
        let block_start = position;
        let block_end = position + num_frames as u64;

        let write_start = if let Some(punch_in) = self.punch_in {
            punch_in.max(block_start)
        } else {
            block_start
        };

        let write_end = if let Some(punch_out) = self.punch_out {
            punch_out.min(block_end)
        } else {
            block_end
        };

        if write_start >= write_end {
            return Ok(());
        }

        let start_frame = (write_start - block_start) as usize;
        let end_frame = (write_end - block_start) as usize;
        let start_sample = start_frame * self.channels;
        let end_sample = (end_frame * self.channels).min(samples.len());

        for &s in &samples[start_sample..end_sample] {
            self.writer
                .write_sample(s.clamp(-1.0, 1.0))
                .map_err(|e| format!("Write error: {e}"))?;
        }

        self.frames_written += (end_frame - start_frame) as u64;
        if self
            .punch_out
            .is_some_and(|punch_out| write_end >= punch_out)
        {
            self.active = false;
        }
        Ok(())
    }

    /// Finalize the recording and close the file.
    pub fn finalize(self) -> Result<RecordingResult, String> {
        let path = self.output_path.clone();
        let frames = self.frames_written;
        let channels = self.channels;
        let sample_rate = self.sample_rate;
        let start_pos = self.start_position;

        self.writer
            .finalize()
            .map_err(|e| format!("Failed to finalize recording: {e}"))?;

        Ok(RecordingResult {
            output_path: path,
            frames_recorded: frames,
            channels,
            sample_rate,
            start_position: start_pos,
        })
    }
}

/// Result of a completed recording session.
#[derive(Debug, Clone)]
pub struct RecordingResult {
    /// Path to the recorded WAV file
    pub output_path: PathBuf,
    /// Number of frames recorded
    pub frames_recorded: u64,
    /// Number of channels
    pub channels: usize,
    /// Sample rate
    pub sample_rate: u32,
    /// Timeline position where the recording started
    pub start_position: u64,
}

impl RecordingResult {
    /// Create a Clip and Region from this recording result for adding to a track.
    pub fn to_region(&self) -> super::clip::Region {
        let clip = super::clip::Clip::from_file(&self.output_path, self.frames_recorded);
        super::clip::Region::new(clip, self.start_position)
    }
}

/// Create a shared input ring buffer for receiving audio from a cpal input stream.
///
/// Returns (producer, consumer). The cpal callback pushes samples via the producer,
/// and the recording session reads via the consumer.
pub fn create_input_buffer(capacity_samples: usize) -> (rtrb::Producer<f32>, rtrb::Consumer<f32>) {
    rtrb::RingBuffer::new(capacity_samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recording_session_basic() {
        let dir = std::env::temp_dir().join("sotf_test_recording_basic");
        std::fs::create_dir_all(&dir).unwrap();

        let config = RecordingConfig {
            output_dir: dir.clone(),
            channels: 1,
            sample_rate: 48000,
            punch_in: None,
            punch_out: None,
        };

        let mut session = RecordingSession::new(&config, 0).unwrap();

        // Write 2 blocks of audio
        let block1 = vec![0.5f32; 1024];
        session.write_block(&block1, 0, 1024).unwrap();
        session.write_block(&block1, 1024, 1024).unwrap();

        assert_eq!(session.frames_written, 2048);

        let result = session.finalize().unwrap();
        assert_eq!(result.frames_recorded, 2048);
        assert!(result.output_path.exists());

        // Verify WAV content
        let reader = hound::WavReader::open(&result.output_path).unwrap();
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().sample_rate, 48000);
        let samples: Vec<f32> = reader.into_samples::<f32>().map(|s| s.unwrap()).collect();
        assert_eq!(samples.len(), 2048);
        for &s in &samples {
            assert!((s - 0.5).abs() < 1e-6);
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_recording_punch_in_out() {
        let dir = std::env::temp_dir().join("sotf_test_recording_punch");
        std::fs::create_dir_all(&dir).unwrap();

        let config = RecordingConfig {
            output_dir: dir.clone(),
            channels: 1,
            sample_rate: 48000,
            punch_in: Some(512),
            punch_out: Some(1536),
        };

        let mut session = RecordingSession::new(&config, 0).unwrap();

        // Block 0: pos 0..1024, punch-in at 512 → record samples 512..1024
        let block = vec![1.0f32; 1024];
        session.write_block(&block, 0, 1024).unwrap();
        assert_eq!(session.frames_written, 512);

        // Block 1: pos 1024..2048, punch-out at 1536 → record samples 0..512
        session.write_block(&block, 1024, 1024).unwrap();
        assert_eq!(session.frames_written, 1024); // 512 + 512
        assert!(!session.active);

        // Block 2: pos 2048..3072 → after punch-out, nothing recorded
        session.write_block(&block, 2048, 1024).unwrap();
        assert_eq!(session.frames_written, 1024); // unchanged

        let result = session.finalize().unwrap();
        assert_eq!(result.frames_recorded, 1024);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_recording_result_to_region() {
        let result = RecordingResult {
            output_path: PathBuf::from("/tmp/test.wav"),
            frames_recorded: 48000,
            channels: 2,
            sample_rate: 48000,
            start_position: 96000,
        };

        let region = result.to_region();
        assert_eq!(region.position_samples, 96000);
        assert_eq!(region.clip.duration_samples, 48000);
    }
}

//! State and finite-data helpers for the fullscreen spectrum presentation.
use sotf_audio_player::SpectrumData;
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct SpectrumViewState {
    pub hold: bool,
    pub held_frame: Option<super::super::playback::HeldSpectrumFrame>,
    pub smoothed: bool,
    pub cursor_fraction: Option<f32>,
    pub details_open: bool,
}

impl SpectrumViewState {
    /// Hold is idempotent and captures only a valid available frame.
    pub fn set_hold(
        &mut self,
        hold: bool,
        data: Option<&Arc<SpectrumData>>,
        sample_rate: Option<u32>,
    ) {
        if !hold {
            self.hold = false;
            self.held_frame = None;
            return;
        }
        if self.hold {
            return;
        }
        if let Some(data) = data.filter(|frame| Self::frequency_range(frame).is_some()) {
            self.held_frame = Some(super::super::playback::HeldSpectrumFrame {
                data: data.clone(),
                sample_rate,
            });
            self.hold = true;
        }
    }

    /// Display bands are equally spaced in log frequency. Recover their edges
    /// from published centers rather than assuming a fixed 20 Hz–20 kHz range.
    pub fn frequency_range(data: &SpectrumData) -> Option<(f32, f32)> {
        let frequencies = &data.frequencies;
        if frequencies.len() < 2
            || frequencies.len() != data.magnitudes.len()
            || data.magnitudes.iter().any(|level| !level.is_finite())
            || frequencies.iter().any(|f| !f.is_finite() || *f <= 0.0)
            || frequencies.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return None;
        }
        let last = frequencies.len() - 1;
        let low = frequencies[0] / (frequencies[1] / frequencies[0]).sqrt();
        let high = frequencies[last] * (frequencies[last] / frequencies[last - 1]).sqrt();
        (low.is_finite() && high.is_finite() && low > 0.0 && high > low).then_some((low, high))
    }

    /// Optional five-band display smoothing, shared by the bars and inspection.
    /// This changes only the display and does not change audio or analyzer setup.
    pub fn magnitudes(data: &SpectrumData, smoothed: bool) -> Arc<[f32]> {
        if data.magnitudes.iter().any(|level| !level.is_finite()) {
            return Arc::from([]);
        }
        if !smoothed {
            return data.magnitudes.clone();
        }
        (0..data.magnitudes.len())
            .map(|index| {
                let start = index.saturating_sub(2);
                let end = (index + 3).min(data.magnitudes.len());
                let values = &data.magnitudes[start..end];
                let sum: f64 = values.iter().map(|value| f64::from(*value)).sum();
                (sum / values.len() as f64) as f32
            })
            .collect::<Vec<_>>()
            .into()
    }

    pub fn inspected_band(&self, count: usize) -> Option<usize> {
        let fraction = self.cursor_fraction.filter(|f| f.is_finite())?;
        (count > 0).then(|| ((fraction.clamp(0.0, 1.0) * count as f32) as usize).min(count - 1))
    }

    pub fn inspect_frequency(&mut self, data: &SpectrumData, frequency_hz: f64) {
        if !frequency_hz.is_finite() || frequency_hz <= 0.0 {
            return;
        }
        if let Some((index, _)) = data
            .frequencies
            .iter()
            .enumerate()
            .filter(|(_, f)| f.is_finite() && **f > 0.0)
            .min_by(|(_, a), (_, b)| {
                (f64::from(**a) / frequency_hz)
                    .ln()
                    .abs()
                    .total_cmp(&(f64::from(**b) / frequency_hz).ln().abs())
            })
        {
            self.cursor_fraction = Some((index as f32 + 0.5) / data.frequencies.len() as f32);
        }
    }
}

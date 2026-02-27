//! Scan controller — unified interface for background scan managers.

use crate::{BlissScanManager, ReplayGainScanManager, WaveformScanManager};

#[derive(Debug)]
pub struct ScanController {
    pub replay_gain_manager: ReplayGainScanManager,
    pub waveform_manager: WaveformScanManager,
    pub bliss_manager: BlissScanManager,
}

impl Default for ScanController {
    fn default() -> Self {
        Self::new()
    }
}

impl ScanController {
    pub fn new() -> Self {
        Self {
            replay_gain_manager: ReplayGainScanManager::new(),
            waveform_manager: WaveformScanManager::new(),
            bliss_manager: BlissScanManager::new(),
        }
    }

    pub fn start_replay_gain_scan(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        self.replay_gain_manager.start_scan()
    }

    pub fn start_waveform_scan(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.waveform_manager.start_scan()
    }

    pub fn start_bliss_scan(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        self.bliss_manager.start_scan()
    }

    /// Update all scan managers (poll for progress).
    pub fn update_all(&mut self) {
        self.replay_gain_manager.update();
        self.waveform_manager.update();
        self.bliss_manager.update();
    }

    /// Stop all running scans.
    pub fn stop_all(&mut self) {
        self.replay_gain_manager.stop();
        self.waveform_manager.stop();
        self.bliss_manager.stop();
    }

    /// Whether any scan is currently in progress.
    pub fn any_in_progress(&self) -> bool {
        self.replay_gain_manager.in_progress
            || self.waveform_manager.in_progress
            || self.bliss_manager.in_progress
    }
}

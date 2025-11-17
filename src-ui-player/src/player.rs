use sotf_audio::manager::AudioStreamingManager;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Player {
    manager: Arc<Mutex<AudioStreamingManager>>,
}

impl Player {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(Mutex::new(AudioStreamingManager::new())),
        }
    }

    pub async fn load_and_play(&self, path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = self.manager.lock().await;

        // Stop current playback if any
        manager.stop_playback().await?;

        // Load the new file
        manager.load_file(&path).await?;

        // Start playback with default settings (no plugins, stereo output)
        let plugins = vec![];
        let output_channels = 2;

        manager.start_playback(None, plugins, output_channels).await?;

        Ok(())
    }

    pub async fn pause(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = self.manager.lock().await;
        manager.pause().await?;
        Ok(())
    }

    pub async fn resume(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = self.manager.lock().await;
        manager.resume().await?;
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = self.manager.lock().await;
        manager.stop_playback().await?;
        Ok(())
    }

    pub async fn set_volume(&self, volume: f32) -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = self.manager.lock().await;
        manager.set_volume(volume).await?;
        Ok(())
    }

    pub async fn get_position(&self) -> Result<f64, Box<dyn std::error::Error>> {
        let manager = self.manager.lock().await;
        Ok(manager.get_position().await)
    }

    pub async fn is_playing(&self) -> Result<bool, Box<dyn std::error::Error>> {
        let manager = self.manager.lock().await;
        Ok(manager.is_playing().await)
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

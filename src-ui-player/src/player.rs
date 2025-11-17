use sotf_audio::engine::PluginConfig;
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

    pub async fn load_and_play(
        &self,
        path: PathBuf,
        plugins: Vec<PluginConfig>,
        output_channels: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = self.manager.lock().await;

        // Stop current playback if any
        manager.stop()?;

        // Load the new file
        manager.load_file(&path)?;

        // Start playback with plugins
        manager.start_playback(None, plugins, output_channels)?;

        Ok(())
    }

    pub async fn update_plugins(
        &self,
        plugins: Vec<PluginConfig>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let manager = self.manager.lock().await;
        // Ignore error if engine not running - plugins will be applied on next playback
        let _ = manager.update_plugin_chain(plugins);
        Ok(())
    }

    pub async fn pause(&self) -> Result<(), Box<dyn std::error::Error>> {
        let manager = self.manager.lock().await;
        manager.pause()?;
        Ok(())
    }

    pub async fn resume(&self) -> Result<(), Box<dyn std::error::Error>> {
        let manager = self.manager.lock().await;
        manager.resume()?;
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = self.manager.lock().await;
        manager.stop()?;
        Ok(())
    }

    pub async fn set_volume(&self, volume: f32) -> Result<(), Box<dyn std::error::Error>> {
        let manager = self.manager.lock().await;
        manager.set_volume(volume)?;
        Ok(())
    }

    pub async fn get_position(&self) -> Result<f64, Box<dyn std::error::Error>> {
        let manager = self.manager.lock().await;
        Ok(manager.get_position())
    }

    pub async fn is_playing(&self) -> Result<bool, Box<dyn std::error::Error>> {
        let manager = self.manager.lock().await;
        let state = manager.get_state();
        Ok(matches!(state, sotf_audio::manager::StreamingState::Playing))
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

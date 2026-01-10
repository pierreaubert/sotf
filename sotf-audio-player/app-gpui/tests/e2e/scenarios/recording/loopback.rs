use crate::driver::AppDriver;
use crate::pages::recording::RecordingPage;
use crate::runner::{E2ERunner, TestScenario};
use sotf_audio_player_gpui::app::types::{SpeakerConfiguration, RecordingSignalType, ChannelRecordingState};
use sotf_audio_player_gpui::ui::PlayerView;
use gpui::{WindowHandle, VisualTestContext};
use std::time::Duration;
use std::error::Error;

pub struct LoopbackScenario;

impl TestScenario for LoopbackScenario {
    fn name(&self) -> &'static str {
        "Recording Loopback"
    }

    fn execute(&self, cx: &mut VisualTestContext, view: WindowHandle<PlayerView>) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, view);
        let mut page = RecordingPage::new(&mut driver);

        // 1. Navigate to Recording Wizard
        page.navigate_to_recording();
        
        // 2. Scan and Find Loopback Device
        page.scan_audio_devices();
        page.run_until_parked();
        
        let loopback_name = page.find_loopback_device();
        
        if loopback_name.is_none() {
            eprintln!("WARNING: No loopback device (BlackHole, Loopback, etc.) found. Skipping recording test.");
            return Ok(());
        }
        let device_name = loopback_name.unwrap();
        eprintln!("Found loopback device: {}", device_name);
        
        // Select the device for both input and output
        page.select_devices(&device_name);
        
        // Get device capability
        let max_channels = page.get_device_max_channels();
        if max_channels == 0 {
             eprintln!("Could not determine max channels for device (0). Skipping.");
             return Ok(());
        }
        eprintln!("Device supports up to {} channels", max_channels);

        // 3. Iterate through all speaker configurations
        for config in SpeakerConfiguration::all() {
            let required_channels = config.channel_count();
            
            if required_channels > max_channels {
                eprintln!("Skipping config {:?} (needs {} channels, device has {})", config, required_channels, max_channels);
                continue;
            }

            eprintln!("Testing Config: {:?} ({} channels)", config, required_channels);
            
            // A. Set Speaker Config
            page.set_speaker_config(*config);
            
            // B. Configure Signal (Short sweep for speed)
            // 1.0s duration to run fast but be valid
            page.configure_signal(RecordingSignalType::Sweep, 1.0, -12.0);
            
            // C. Start Recording
            page.start_recording_all();
            
            // D. Wait for completion
            // Timeout based on channel count * (duration + ~1.5s overhead)
            let total_duration = required_channels as f64 * 2.5; 
            let timeout = Duration::from_secs_f64(total_duration + 5.0); 
            
            // Poll for completion
            let start_time = std::time::Instant::now();
            loop {
                if start_time.elapsed() > timeout {
                    return Err(format!("Timeout waiting for recording completion of {:?}", config).into());
                }
                
                let all_done = page.is_all_channels_recorded();
                
                if all_done {
                    break;
                }
                
                // We assume real time passes for the audio engine.
                // In GPUI tests, we often need to let the loop run.
                page.run_until_parked();
                std::thread::sleep(Duration::from_millis(100));
            }
            
            // E. Verify Results
            page.verify_recording_results();
            
            eprintln!("Config {:?} PASSED", config);
        }
        
        Ok(())
    }
}

#[gpui::test]
async fn test_recording_loopback_all_configs(cx: &mut gpui::TestAppContext) {
    let scenario = LoopbackScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;
    
    if let Err(e) = &result {
        eprintln!("Test failed: {}", e);
    }
    assert!(result.is_ok());
}

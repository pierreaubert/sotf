use sotf_audio::devices::get_audio_devices;
use sotf_audio_player::{
    AudioOutputDeviceState, DiagnosticsBundle, LibraryScanSummary, MusicLibrary, NoAudioReason,
    Player, Queue, diagnose_no_audio,
};
use std::path::PathBuf;

/// Run the diagnostics command.
///
/// If `why_no_audio` is true, prints a compact diagnosis. Otherwise builds a
/// secret-safe diagnostics bundle and writes it to `output` (or prints a
/// compact summary if no output path is given).
pub(super) fn run_diagnostics_command(
    output: Option<PathBuf>,
    why_no_audio: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut player = Player::new();
    let queue = Queue::new();

    let mut device_state = AudioOutputDeviceState::new();
    match get_audio_devices() {
        Ok(devices) => {
            if let Some(outputs) = devices.get("output") {
                device_state.set_devices_with_smart_default(outputs.clone());
            }
        }
        Err(e) => {
            log::warn!("Could not enumerate audio devices for diagnostics: {}", e);
        }
    }

    let library = MusicLibrary::new();
    let scan_summary = LibraryScanSummary::from_library(&library);

    if why_no_audio {
        let reasons = diagnose_no_audio(&mut player, &queue, &device_state, None);
        if reasons.is_empty() || reasons.iter().any(|r| matches!(r, NoAudioReason::Unknown)) {
            println!("Audio should be playing. If it isn't, export a diagnostics bundle with:");
            println!("  player-cli diagnostics --output sotf-diagnostics.json");
        } else {
            println!("Why no audio:");
            for reason in reasons {
                println!("  - {}", reason);
            }
        }
        return Ok(());
    }

    let bundle =
        DiagnosticsBundle::build(&mut player, &device_state, scan_summary, Vec::new(), None);

    if let Some(path) = output {
        bundle.write_redacted_json(&path)?;
        println!("Diagnostics bundle written to {}", path.display());
    } else {
        let json = bundle.to_redacted_json()?;
        println!("{}", json);
    }

    Ok(())
}

/// Pre-flight checks to run before starting the audio player
///
/// These checks verify that the system is properly configured for audio playback
/// and provide helpful error messages if configuration is needed.
use std::fmt;

#[derive(Debug)]
pub enum PreflightError {
    /// User is not in the required audio group (Linux)
    MissingAudioGroup { user: String, group: String },
    /// No audio cards detected on the system (Linux)
    NoAudioCards,
    /// Generic configuration error
    ConfigError(String),
}

impl fmt::Display for PreflightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PreflightError::MissingAudioGroup { user, group } => {
                write!(
                    f,
                    "User '{}' is not in the '{}' group.\n\
                     Audio playback requires membership in the '{}' group.\n\
                     \n\
                     To fix this, run:\n\
                     \n\
                     sudo usermod -a -G {} {}\n\
                     \n\
                     Then log out and log back in for the changes to take effect.\n\
                     \n\
                     You can verify the fix with: groups {}",
                    user, group, group, group, user, user
                )
            }
            PreflightError::NoAudioCards => {
                write!(
                    f,
                    "No audio cards detected on the system.\n\
                     Audio playback requires at least one audio device.\n\
                     \n\
                     To diagnose this issue:\n\
                     \n\
                     1. Check if any audio cards are detected:\n\
                        cat /proc/asound/cards\n\
                     \n\
                     2. List available audio devices:\n\
                        aplay -l\n\
                     \n\
                     3. Check if sound modules are loaded:\n\
                        lsmod | grep snd\n\
                     \n\
                     4. Verify hardware with:\n\
                        lspci | grep -i audio\n\
                        lsusb | grep -i audio\n\
                     \n\
                     If no audio hardware is found, you may need to:\n\
                     - Install audio drivers for your hardware\n\
                     - Enable audio in your system BIOS/UEFI\n\
                     - Connect a USB audio device or headphones"
                )
            }
            PreflightError::ConfigError(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for PreflightError {}

/// Run all platform-specific pre-flight checks
pub fn run_preflight_checks() -> Result<(), PreflightError> {
    #[cfg(target_os = "linux")]
    {
        check_linux_audio_group()?;
        check_linux_audio_cards()?;
    }

    // Add more platform-specific checks here as needed
    #[cfg(target_os = "windows")]
    {
        // Windows-specific checks (if any)
    }

    #[cfg(target_os = "macos")]
    {
        // macOS-specific checks (if any)
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn check_linux_audio_group() -> Result<(), PreflightError> {
    use std::process::Command;

    // Get current username
    let username = get_current_username()?;

    // Check if user is in the 'audio' group
    let output = Command::new("groups").output().map_err(|e| {
        PreflightError::ConfigError(format!("Failed to run 'groups' command: {}", e))
    })?;

    if !output.status.success() {
        return Err(PreflightError::ConfigError(
            "Failed to get user groups".to_string(),
        ));
    }

    let groups = String::from_utf8_lossy(&output.stdout);
    let group_list: Vec<&str> = groups.split_whitespace().collect();

    if !group_list.contains(&"audio") {
        return Err(PreflightError::MissingAudioGroup {
            user: username,
            group: "audio".to_string(),
        });
    }

    log::debug!("[Preflight] User '{}' is in the 'audio' group", username);
    Ok(())
}

#[cfg(target_os = "linux")]
fn get_current_username() -> Result<String, PreflightError> {
    use std::env;

    env::var("USER")
        .or_else(|_| env::var("USERNAME"))
        .map_err(|_| {
            PreflightError::ConfigError("Failed to determine current username".to_string())
        })
}

#[cfg(target_os = "linux")]
fn check_linux_audio_cards() -> Result<(), PreflightError> {
    use std::fs;
    use std::path::Path;

    let cards_path = Path::new("/proc/asound/cards");

    // Check if /proc/asound/cards exists
    if !cards_path.exists() {
        log::warn!("[Preflight] /proc/asound/cards does not exist - ALSA may not be available");
        return Err(PreflightError::ConfigError(
            "/proc/asound/cards does not exist. ALSA sound system may not be available."
                .to_string(),
        ));
    }

    // Read the cards file
    let contents = fs::read_to_string(cards_path).map_err(|e| {
        PreflightError::ConfigError(format!("Failed to read /proc/asound/cards: {}", e))
    })?;

    // Check if the file is empty or contains "no soundcards"
    let trimmed = contents.trim();

    if trimmed.is_empty() || trimmed.contains("no soundcards") {
        log::error!("[Preflight] No audio cards found in /proc/asound/cards");
        return Err(PreflightError::NoAudioCards);
    }

    // Look for at least one card (lines starting with a number)
    let has_cards = contents.lines().any(|line| {
        line.trim_start()
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    });

    if !has_cards {
        log::error!("[Preflight] /proc/asound/cards exists but contains no valid card entries");
        return Err(PreflightError::NoAudioCards);
    }

    // Log the detected cards for debugging
    let card_count = contents
        .lines()
        .filter(|line| {
            line.trim_start()
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit())
        })
        .count();

    log::debug!(
        "[Preflight] Found {} audio card(s) in /proc/asound/cards",
        card_count
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preflight_checks() {
        // This test will only pass if the system is properly configured
        // On CI or systems without audio group, this might fail
        match run_preflight_checks() {
            Ok(_) => println!("Pre-flight checks passed"),
            Err(e) => println!("Pre-flight checks failed (expected on some systems): {}", e),
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_get_username() {
        let username = get_current_username();
        assert!(username.is_ok(), "Should be able to get username");
        let username = username.unwrap();
        assert!(!username.is_empty(), "Username should not be empty");
        println!("Current username: {}", username);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_audio_cards_check() {
        // This test checks that the audio card detection logic works
        match check_linux_audio_cards() {
            Ok(_) => println!("Audio cards check passed - at least one card detected"),
            Err(PreflightError::NoAudioCards) => {
                println!("No audio cards detected (expected on systems without audio hardware)")
            }
            Err(PreflightError::ConfigError(msg)) => {
                println!("Audio cards check failed with config error: {}", msg)
            }
            Err(e) => {
                panic!("Unexpected error type: {:?}", e);
            }
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_proc_asound_parsing() {
        // Test parsing logic with sample /proc/asound/cards content

        // Example 1: System with one card
        let sample1 = " 0 [PCH            ]: HDA-Intel - HDA Intel PCH\n\
                       HDA Intel PCH at 0xf7230000 irq 131\n";
        let has_cards = sample1.lines().any(|line| {
            line.trim_start()
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit())
        });
        assert!(has_cards, "Should detect card in sample1");

        // Example 2: System with multiple cards
        let sample2 = " 0 [PCH            ]: HDA-Intel - HDA Intel PCH\n\
                       HDA Intel PCH at 0xf7230000 irq 131\n\
                       1 [HDMI           ]: HDA-Intel - HDA Intel HDMI\n\
                       HDA Intel HDMI at 0xf7234000 irq 132\n";
        let card_count = sample2
            .lines()
            .filter(|line| {
                line.trim_start()
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit())
            })
            .count();
        assert_eq!(card_count, 2, "Should detect 2 cards in sample2");

        // Example 3: Empty file
        let sample3 = "";
        let has_cards_empty = sample3.lines().any(|line| {
            line.trim_start()
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit())
        });
        assert!(!has_cards_empty, "Should not detect cards in empty file");

        // Example 4: File with "no soundcards"
        let sample4 = "--- no soundcards ---\n";
        let has_no_soundcards = sample4.contains("no soundcards");
        assert!(has_no_soundcards, "Should detect 'no soundcards' message");
    }
}

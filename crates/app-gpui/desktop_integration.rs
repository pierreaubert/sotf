//! Linux desktop integration (freedesktop .desktop file + icon).
//!
//! On first launch, installs:
//! - `~/.local/share/applications/org.spinorama.sotf.desktop`
//! - `~/.local/share/icons/hicolor/256x256/apps/org.spinorama.sotf.png`
//!
//! This gives KDE/GNOME proper window-to-app association (taskbar icon,
//! app menu entry, media type associations).
//!
//! Re-installs automatically when the binary path changes (e.g. after an update).

use std::path::{Path, PathBuf};

const APP_ID: &str = "org.spinorama.sotf";
const DESKTOP_FILENAME: &str = "org.spinorama.sotf.desktop";
const ICON_FILENAME: &str = "org.spinorama.sotf.png";

/// Install desktop integration files if needed.
/// Called once at startup — fast no-op if already installed with the current binary.
pub fn ensure_desktop_integration(icon_png: Option<&[u8]>) {
    if let Err(e) = try_install(icon_png) {
        log::warn!("Desktop integration failed: {}", e);
    }
}

fn try_install(icon_png: Option<&[u8]>) -> Result<(), String> {
    let data_dir = get_data_dir()?;
    let apps_dir = data_dir.join("applications");
    let icons_dir = data_dir.join("icons/hicolor/256x256/apps");

    let desktop_path = apps_dir.join(DESKTOP_FILENAME);
    let icon_path = icons_dir.join(ICON_FILENAME);

    let exe_path = std::env::current_exe()
        .map_err(|e| format!("cannot determine binary path: {}", e))?;
    let exe_str = exe_path.display().to_string();

    // Check if already installed with the current binary path
    if is_up_to_date(&desktop_path, &exe_str) && icon_path.exists() {
        log::debug!("Desktop integration already up to date");
        return Ok(());
    }

    log::info!("Installing desktop integration for {}", exe_str);

    // Install .desktop file
    std::fs::create_dir_all(&apps_dir)
        .map_err(|e| format!("cannot create {}: {}", apps_dir.display(), e))?;

    let desktop_content = generate_desktop_file(&exe_str);
    std::fs::write(&desktop_path, desktop_content)
        .map_err(|e| format!("cannot write {}: {}", desktop_path.display(), e))?;

    // Install icon
    if let Some(png_data) = icon_png {
        std::fs::create_dir_all(&icons_dir)
            .map_err(|e| format!("cannot create {}: {}", icons_dir.display(), e))?;
        std::fs::write(&icon_path, png_data)
            .map_err(|e| format!("cannot write {}: {}", icon_path.display(), e))?;
    }

    // Update desktop database (best-effort)
    let _ = std::process::Command::new("update-desktop-database")
        .arg(&apps_dir)
        .status();

    // Update icon cache (best-effort)
    let hicolor_dir = data_dir.join("icons/hicolor");
    let _ = std::process::Command::new("gtk-update-icon-cache")
        .args(["-f", "-t"])
        .arg(&hicolor_dir)
        .status();

    log::info!("Desktop integration installed successfully");
    Ok(())
}

/// Check if the .desktop file exists and points to the current binary.
fn is_up_to_date(desktop_path: &Path, exe_str: &str) -> bool {
    match std::fs::read_to_string(desktop_path) {
        Ok(content) => {
            let expected_exec = format!("Exec={} %F", exe_str);
            content.contains(&expected_exec)
        }
        Err(_) => false,
    }
}

fn generate_desktop_file(exe_path: &str) -> String {
    format!(
        "\
[Desktop Entry]
Type=Application
Name=SotF Player
GenericName=Audio Player
Comment=High-quality audio player with advanced EQ and upmixing
Exec={exe_path} %F
Icon={APP_ID}
Categories=Audio;AudioVideo;Player;Music;
Terminal=false
MimeType=audio/flac;audio/mpeg;audio/ogg;audio/wav;audio/x-wav;audio/mp4;audio/aac;
Keywords=audio;music;player;eq;equalizer;
StartupWMClass={APP_ID}
"
    )
}

fn get_data_dir() -> Result<PathBuf, String> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return Ok(PathBuf::from(xdg));
    }
    if let Ok(home) = std::env::var("HOME") {
        return Ok(PathBuf::from(home).join(".local/share"));
    }
    Err("cannot determine data directory (HOME not set)".into())
}

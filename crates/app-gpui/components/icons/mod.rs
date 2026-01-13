//! Icon component for Lucide SVG icons
//!
//! This module provides a type-safe way to use Lucide icons in the GPUI player.

use gpui::prelude::*;
use gpui::*;

/// Available icon names from the Lucide icon pack
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconName {
    // Playback controls
    Play,
    Pause,
    Square, // Stop
    SkipForward,
    SkipBack,
    FastForward,
    Rewind,
    Shuffle,
    Repeat,

    // Volume & Audio
    Volume2,
    VolumeX,
    Speaker,

    // Navigation
    Home,
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    ChevronDown,

    // Actions
    Plus,
    Minus,
    X,
    Check,
    Search,
    Settings,

    // Music
    Music,
    Disc,
    Album,
    ListMusic,
    Library,
    AudioWaveform,

    // General
    Folder,
    Heart,
    Plug,
    SlidersHorizontal,
    User,
    PenTool,
}

impl IconName {
    /// Get the asset path for this icon
    pub fn path(&self) -> &'static str {
        match self {
            IconName::Play => "icons/play.svg",
            IconName::Pause => "icons/pause.svg",
            IconName::Square => "icons/square.svg",
            IconName::SkipForward => "icons/skip-forward.svg",
            IconName::SkipBack => "icons/skip-back.svg",
            IconName::FastForward => "icons/fast-forward.svg",
            IconName::Rewind => "icons/rewind.svg",
            IconName::Shuffle => "icons/shuffle.svg",
            IconName::Repeat => "icons/repeat.svg",
            IconName::Volume2 => "icons/volume-2.svg",
            IconName::VolumeX => "icons/volume-x.svg",
            IconName::Speaker => "icons/speaker.svg",
            IconName::Home => "icons/home.svg",
            IconName::ChevronLeft => "icons/chevron-left.svg",
            IconName::ChevronRight => "icons/chevron-right.svg",
            IconName::ChevronUp => "icons/chevron-up.svg",
            IconName::ChevronDown => "icons/chevron-down.svg",
            IconName::Plus => "icons/plus.svg",
            IconName::Minus => "icons/minus.svg",
            IconName::X => "icons/x.svg",
            IconName::Check => "icons/check.svg",
            IconName::Search => "icons/search.svg",
            IconName::Settings => "icons/settings.svg",
            IconName::Music => "icons/music.svg",
            IconName::Disc => "icons/disc.svg",
            IconName::Album => "icons/album.svg",
            IconName::ListMusic => "icons/list-music.svg",
            IconName::Library => "icons/library.svg",
            IconName::AudioWaveform => "icons/audio-waveform.svg",
            IconName::Folder => "icons/folder.svg",
            IconName::Heart => "icons/heart.svg",
            IconName::Plug => "icons/plug.svg",
            IconName::SlidersHorizontal => "icons/sliders-horizontal.svg",
            IconName::User => "icons/user.svg",
            IconName::PenTool => "icons/pen-tool.svg",
        }
    }
}

/// Icon size variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconSize {
    /// Extra small (12px)
    Xs,
    /// Small (16px)
    Sm,
    /// Medium (20px, default)
    #[default]
    Md,
    /// Large (24px)
    Lg,
    /// Extra large (32px)
    Xl,
    /// Extra extra large (36px)
    Xxl,
}

impl IconSize {
    /// Get the pixel size
    pub fn px(&self) -> Pixels {
        match self {
            IconSize::Xs => px(12.0),
            IconSize::Sm => px(16.0),
            IconSize::Md => px(20.0),
            IconSize::Lg => px(24.0),
            IconSize::Xl => px(32.0),
            IconSize::Xxl => px(36.0),
        }
    }
}

/// Icon component that renders Lucide SVG icons
#[derive(IntoElement)]
pub struct Icon {
    name: IconName,
    size: IconSize,
    color: Option<Rgba>,
}

impl Icon {
    /// Create a new icon with the given name
    pub fn new(name: IconName) -> Self {
        Self {
            name,
            size: IconSize::default(),
            color: None,
        }
    }

    /// Set the icon size
    pub fn size(mut self, size: IconSize) -> Self {
        self.size = size;
        self
    }

    /// Set extra small size
    pub fn xs(mut self) -> Self {
        self.size = IconSize::Xs;
        self
    }

    /// Set small size
    pub fn small(mut self) -> Self {
        self.size = IconSize::Sm;
        self
    }

    /// Set medium size (default)
    pub fn medium(mut self) -> Self {
        self.size = IconSize::Md;
        self
    }

    /// Set large size
    pub fn large(mut self) -> Self {
        self.size = IconSize::Lg;
        self
    }

    /// Set extra large size
    pub fn xl(mut self) -> Self {
        self.size = IconSize::Xl;
        self
    }

    /// Set extra extra large size
    pub fn xxl(mut self) -> Self {
        self.size = IconSize::Xxl;
        self
    }

    /// Set the icon color
    pub fn color(mut self, color: impl Into<Rgba>) -> Self {
        self.color = Some(color.into());
        self
    }
}

impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let size = self.size.px();

        let mut el = svg().path(self.name.path()).size(size);

        if let Some(color) = self.color {
            el = el.text_color(color);
        }

        el
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn test_icon_paths() {
//         assert_eq!(IconName::Play.path(), "icons/play.svg");
//         assert_eq!(IconName::Pause.path(), "icons/pause.svg");
//         assert_eq!(IconName::Settings.path(), "icons/settings.svg");
//     }
//
//     #[test]
//     fn test_icon_sizes() {
//         assert_eq!(IconSize::Xs.px(), px(12.0));
//         assert_eq!(IconSize::Sm.px(), px(16.0));
//         assert_eq!(IconSize::Md.px(), px(20.0));
//         assert_eq!(IconSize::Lg.px(), px(24.0));
//         assert_eq!(IconSize::Xl.px(), px(32.0));
//     }
// }

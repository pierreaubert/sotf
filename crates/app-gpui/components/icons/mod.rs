//! Icon component for application and Lucide SVG icons
//!
//! This module provides a type-safe way to use Lucide icons in the GPUI player.

use gpui::prelude::*;
use gpui::{App, Pixels, Rems, Rgba, Window, px, rems, svg};

/// Available application and Lucide icon names
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconName {
    // EQ response shapes
    FilterPeak,
    FilterLowShelf,
    FilterHighShelf,
    FilterLowPass,
    FilterHighPass,
    FilterBandPass,
    FilterNotch,
    FilterAllPass,

    // Plugin-specific artwork
    PluginAae,
    PluginAbCompare,
    PluginAec,
    PluginAmbisonics,
    PluginAnalogCompressor,
    PluginAnalogEq,
    PluginAnalogLimiter,
    PluginBandMerge,
    PluginBandSplit,
    PluginBeamformer,
    PluginBinaural,
    PluginChannelMuteSolo,
    PluginConvolution,
    PluginCrossfeed,
    PluginCrossover,
    PluginDeEsser,
    PluginDeclick,
    PluginDelay,
    PluginDenoiser,
    PluginDither,
    PluginDownmix,
    PluginDynamicEq,
    PluginEq,
    PluginGain,
    PluginGate,
    PluginHalInput,
    PluginHalOutput,
    PluginHissReducer,
    PluginLimiter,
    PluginLinearPhaseEq,
    PluginLoudnessCompensation,
    PluginMatrix,
    PluginMonoToStereo,
    PluginMultibandCompressor,
    PluginMultibandExpander,
    PluginPnd,
    PluginResampler,
    PluginSaturation,
    PluginSpectralCompressor,
    PluginSpeechDenoiser,
    PluginStereoImager,
    PluginTransientShaper,
    PluginUpmixer,
    PluginXtc,

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
    Headphones,
    Brain,

    // Navigation
    Home,
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    ChevronDown,
    StudioRack,
    StudioGraph,
    Measure,
    Calibrate,
    Learning,

    // Actions
    Plus,
    Minus,
    X,
    Check,
    Lock,
    GripVertical,
    Search,
    Settings,

    // Music
    Music,
    Disc,
    Album,
    ListMusic,
    Library,
    AudioWaveform,
    Radio,
    ChartLine,
    Ear,

    // Actions (extra)
    RotateCw,
    LayoutGrid,
    List,
    Funnel,

    // Settings
    Languages,
    Keyboard,

    // General
    Folder,
    Heart,
    HeartFilled,
    Plug,
    SlidersHorizontal,
    User,
    PenTool,

    // Intelligence / settings
    Cog,
}

impl IconName {
    /// Get the asset path for this icon
    #[must_use]
    #[expect(
        clippy::too_many_lines,
        reason = "Keep the ordered UI workflow and its state updates in one operation"
    )]
    pub fn path(&self) -> &'static str {
        match self {
            IconName::FilterPeak => "icons/eq-filter-peak.svg",
            IconName::FilterLowShelf => "icons/eq-filter-low-shelf.svg",
            IconName::FilterHighShelf => "icons/eq-filter-high-shelf.svg",
            IconName::FilterLowPass => "icons/eq-filter-low-pass.svg",
            IconName::FilterHighPass => "icons/eq-filter-high-pass.svg",
            IconName::FilterBandPass => "icons/eq-filter-band-pass.svg",
            IconName::FilterNotch => "icons/eq-filter-notch.svg",
            IconName::FilterAllPass => "icons/eq-filter-all-pass.svg",
            IconName::PluginAae => "icons/sotf-plugin-aae.svg",
            IconName::PluginAbCompare => "icons/sotf-plugin-ab-compare.svg",
            IconName::PluginAec => "icons/sotf-plugin-aec.svg",
            IconName::PluginAmbisonics => "icons/sotf-plugin-ambisonics.svg",
            IconName::PluginAnalogCompressor => "icons/sotf-plugin-analog-compressor.svg",
            IconName::PluginAnalogEq => "icons/sotf-plugin-analog-eq.svg",
            IconName::PluginAnalogLimiter => "icons/sotf-plugin-analog-limiter.svg",
            IconName::PluginBandMerge => "icons/sotf-plugin-band-merge.svg",
            IconName::PluginBandSplit => "icons/sotf-plugin-band-split.svg",
            IconName::PluginBeamformer => "icons/sotf-plugin-beamformer.svg",
            IconName::PluginBinaural => "icons/sotf-plugin-binaural.svg",
            IconName::PluginChannelMuteSolo => "icons/sotf-plugin-channel-mute-solo.svg",
            IconName::PluginConvolution => "icons/sotf-plugin-convolution.svg",
            IconName::PluginCrossfeed => "icons/sotf-plugin-crossfeed.svg",
            IconName::PluginCrossover => "icons/sotf-plugin-crossover.svg",
            IconName::PluginDeEsser => "icons/sotf-plugin-de-esser.svg",
            IconName::PluginDeclick => "icons/sotf-plugin-declick.svg",
            IconName::PluginDelay => "icons/sotf-plugin-delay.svg",
            IconName::PluginDenoiser => "icons/sotf-plugin-denoiser.svg",
            IconName::PluginDither => "icons/sotf-plugin-dither.svg",
            IconName::PluginDownmix => "icons/sotf-plugin-downmix.svg",
            IconName::PluginDynamicEq => "icons/sotf-plugin-dynamic-eq.svg",
            IconName::PluginEq => "icons/sotf-plugin-eq.svg",
            IconName::PluginGain => "icons/sotf-plugin-gain.svg",
            IconName::PluginGate => "icons/sotf-plugin-gate.svg",
            IconName::PluginHalInput => "icons/sotf-plugin-hal-input.svg",
            IconName::PluginHalOutput => "icons/sotf-plugin-hal-output.svg",
            IconName::PluginHissReducer => "icons/sotf-plugin-hiss-reducer.svg",
            IconName::PluginLimiter => "icons/sotf-plugin-limiter.svg",
            IconName::PluginLinearPhaseEq => "icons/sotf-plugin-linear-phase-eq.svg",
            IconName::PluginLoudnessCompensation => "icons/sotf-plugin-loudness-compensation.svg",
            IconName::PluginMatrix => "icons/sotf-plugin-matrix.svg",
            IconName::PluginMonoToStereo => "icons/sotf-plugin-mono-to-stereo.svg",
            IconName::PluginMultibandCompressor => "icons/sotf-plugin-multiband-compressor.svg",
            IconName::PluginMultibandExpander => "icons/sotf-plugin-multiband-expander.svg",
            IconName::PluginPnd => "icons/sotf-plugin-pnd.svg",
            IconName::PluginResampler => "icons/sotf-plugin-resampler.svg",
            IconName::PluginSaturation => "icons/sotf-plugin-saturation.svg",
            IconName::PluginSpectralCompressor => "icons/sotf-plugin-spectral-compressor.svg",
            IconName::PluginSpeechDenoiser => "icons/sotf-plugin-speech-denoiser.svg",
            IconName::PluginStereoImager => "icons/sotf-plugin-stereo-imager.svg",
            IconName::PluginTransientShaper => "icons/sotf-plugin-transient-shaper.svg",
            IconName::PluginUpmixer => "icons/sotf-plugin-upmixer.svg",
            IconName::PluginXtc => "icons/sotf-plugin-xtc.svg",
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
            IconName::Headphones => "icons/headphones.svg",
            IconName::Brain => "icons/brain.svg",
            IconName::Home => "icons/home.svg",
            IconName::ChevronLeft => "icons/chevron-left.svg",
            IconName::ChevronRight => "icons/chevron-right.svg",
            IconName::ChevronUp => "icons/chevron-up.svg",
            IconName::ChevronDown => "icons/chevron-down.svg",
            IconName::StudioRack => "icons/studio-rack.svg",
            IconName::StudioGraph => "icons/studio-graph.svg",
            IconName::Measure => "icons/measure.svg",
            IconName::Calibrate => "icons/calibrate.svg",
            IconName::Learning => "icons/learning.svg",
            IconName::Plus => "icons/plus.svg",
            IconName::Minus => "icons/minus.svg",
            IconName::X => "icons/x.svg",
            IconName::Check => "icons/check.svg",
            IconName::Lock => "icons/lock.svg",
            IconName::GripVertical => "icons/grip-vertical.svg",
            IconName::Search => "icons/search.svg",
            IconName::Settings => "icons/settings.svg",
            IconName::Music => "icons/music.svg",
            IconName::Disc => "icons/disc.svg",
            IconName::Album => "icons/album.svg",
            IconName::ListMusic => "icons/list-music.svg",
            IconName::Library => "icons/library.svg",
            IconName::AudioWaveform => "icons/audio-waveform.svg",
            IconName::Radio => "icons/radio.svg",
            IconName::ChartLine => "icons/chart-line.svg",
            IconName::Ear => "icons/ear.svg",
            IconName::RotateCw => "icons/rotate-cw.svg",
            IconName::LayoutGrid => "icons/layout-grid.svg",
            IconName::List => "icons/list.svg",
            IconName::Funnel => "icons/funnel.svg",
            IconName::Languages => "icons/languages.svg",
            IconName::Keyboard => "icons/keyboard.svg",
            IconName::Folder => "icons/folder.svg",
            IconName::Heart => "icons/heart.svg",
            IconName::HeartFilled => "icons/heart-filled.svg",
            IconName::Plug => "icons/plug.svg",
            IconName::SlidersHorizontal => "icons/sliders-horizontal.svg",
            IconName::User => "icons/user.svg",
            IconName::PenTool => "icons/pen-tool.svg",
            IconName::Cog => "icons/cog.svg",
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
    /// Get the pixel size (for APIs that require Pixels).
    ///
    /// Prefer [`IconSize::to_rems`] so sizes scale with `window.rem_size`
    /// (driven by the `IncreaseFontSize` / `DecreaseFontSize` actions). This
    /// absolute-pixel variant is retained for the few APIs that require
    /// `Pixels` and for legacy tests; new call sites should render an
    /// [`Icon`] or use `.to_rems()` directly.
    #[deprecated(
        note = "Use `IconSize::to_rems()` (or the `Icon` component) so icons scale with font zoom. \
                Raw pixel sizes bypass `window.rem_size` and break zoom consistency."
    )]
    #[must_use]
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

    /// Get the rem-based size (scales with window for responsive design)
    #[must_use]
    pub fn to_rems(&self) -> Rems {
        match self {
            IconSize::Xs => rems(0.75),  // 12px at 16px rem
            IconSize::Sm => rems(1.0),   // 16px at 16px rem
            IconSize::Md => rems(1.25),  // 20px at 16px rem
            IconSize::Lg => rems(1.5),   // 24px at 16px rem
            IconSize::Xl => rems(2.0),   // 32px at 16px rem
            IconSize::Xxl => rems(2.25), // 36px at 16px rem
        }
    }
}

/// Icon component that renders Lucide SVG icons
#[derive(IntoElement, Debug)]
pub struct Icon {
    name: IconName,
    size: IconSize,
    color: Option<Rgba>,
}

impl Icon {
    /// Create a new icon with the given name
    #[must_use]
    pub fn new(name: IconName) -> Self {
        Self {
            name,
            size: IconSize::default(),
            color: None,
        }
    }

    /// Set the icon size
    #[must_use]
    pub fn size(mut self, size: IconSize) -> Self {
        self.size = size;
        self
    }

    /// Set extra small size
    #[must_use]
    pub fn xs(mut self) -> Self {
        self.size = IconSize::Xs;
        self
    }

    /// Set small size
    #[must_use]
    pub fn small(mut self) -> Self {
        self.size = IconSize::Sm;
        self
    }

    /// Set medium size (default)
    #[must_use]
    pub fn medium(mut self) -> Self {
        self.size = IconSize::Md;
        self
    }

    /// Set large size
    #[must_use]
    pub fn large(mut self) -> Self {
        self.size = IconSize::Lg;
        self
    }

    /// Set extra large size
    #[must_use]
    pub fn xl(mut self) -> Self {
        self.size = IconSize::Xl;
        self
    }

    /// Set extra extra large size
    #[must_use]
    pub fn xxl(mut self) -> Self {
        self.size = IconSize::Xxl;
        self
    }

    /// Set the icon color
    #[must_use]
    pub fn color(mut self, color: impl Into<Rgba>) -> Self {
        self.color = Some(color.into());
        self
    }
}

impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // Use rem-based sizing so icons scale with the responsive rem size
        let size = self.size.to_rems();

        // Icons are fixed-format controls. Prevent flex reflow from shrinking
        // their SVG bounds to zero during screen/theme transitions.
        let mut el = svg().path(self.name.path()).size(size).flex_none();

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

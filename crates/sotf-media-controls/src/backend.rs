//! Per-platform backend dispatch.

use crate::{Error, MediaControlEvent, MediaMetadata, MediaPlayback, PlatformConfig};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
mod mpris;
mod stub;

pub(crate) type EventHandler = Box<dyn FnMut(MediaControlEvent) + Send + 'static>;

#[allow(clippy::large_enum_variant)]
pub(crate) enum Backend {
    #[cfg(target_os = "macos")]
    Macos(macos::MacosBackend),
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    Mpris(mpris::MprisBackend),
    Stub(stub::StubBackend),
}

impl Backend {
    pub(crate) fn new(config: PlatformConfig<'_>) -> Result<Self, Error> {
        #[cfg(target_os = "macos")]
        {
            return macos::MacosBackend::new(&config).map(Backend::Macos);
        }

        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        {
            return mpris::MprisBackend::new(&config).map(Backend::Mpris);
        }

        #[allow(unreachable_code)]
        {
            let _ = config;
            stub::StubBackend::new().map(Backend::Stub)
        }
    }

    pub(crate) fn attach(&mut self, handler: EventHandler) -> Result<(), Error> {
        match self {
            #[cfg(target_os = "macos")]
            Backend::Macos(b) => b.attach(handler),
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            Backend::Mpris(b) => b.attach(handler),
            Backend::Stub(b) => b.attach(handler),
        }
    }

    pub(crate) fn set_metadata(&mut self, metadata: MediaMetadata<'_>) -> Result<(), Error> {
        match self {
            #[cfg(target_os = "macos")]
            Backend::Macos(b) => b.set_metadata(metadata),
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            Backend::Mpris(b) => b.set_metadata(metadata),
            Backend::Stub(b) => b.set_metadata(metadata),
        }
    }

    pub(crate) fn set_playback(&mut self, playback: MediaPlayback) -> Result<(), Error> {
        match self {
            #[cfg(target_os = "macos")]
            Backend::Macos(b) => b.set_playback(playback),
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            Backend::Mpris(b) => b.set_playback(playback),
            Backend::Stub(b) => b.set_playback(playback),
        }
    }
}

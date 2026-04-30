//! No-op backend for platforms without OS media controls (Windows, iOS,
//! tvOS, ...). Constructing it returns `Err(Unsupported)` so callers fall
//! back without panics.

use crate::{Error, MediaMetadata, MediaPlayback, backend::EventHandler};

pub(crate) struct StubBackend {
    _private: (),
}

impl StubBackend {
    pub(crate) fn new() -> Result<Self, Error> {
        Err(Error::Unsupported)
    }

    pub(crate) fn attach(&mut self, _handler: EventHandler) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    pub(crate) fn set_metadata(&mut self, _metadata: MediaMetadata<'_>) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    pub(crate) fn set_playback(&mut self, _playback: MediaPlayback) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
}

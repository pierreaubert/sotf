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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MediaControlEvent;

    #[test]
    fn new_reports_unsupported() {
        assert!(matches!(StubBackend::new(), Err(Error::Unsupported)));
    }

    #[test]
    fn methods_report_unsupported() {
        let mut backend = StubBackend { _private: () };

        assert!(matches!(
            backend.attach(Box::new(|_: MediaControlEvent| {})),
            Err(Error::Unsupported)
        ));
        assert!(matches!(
            backend.set_metadata(MediaMetadata::default()),
            Err(Error::Unsupported)
        ));
        assert!(matches!(
            backend.set_playback(MediaPlayback::Stopped),
            Err(Error::Unsupported)
        ));
    }
}

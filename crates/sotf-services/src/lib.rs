mod service;

#[cfg(feature = "spotify")]
pub mod spotify;

#[cfg(feature = "tidal")]
pub mod tidal;

pub use service::{
    AudioQuality, PcmStream, ServiceCredentials, ServiceError, ServiceTrack, StreamingService,
};

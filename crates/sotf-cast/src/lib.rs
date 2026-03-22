mod discovery;

#[cfg(feature = "airplay")]
pub mod airplay;

#[cfg(feature = "chromecast")]
pub mod chromecast;

pub use discovery::{CastDevice, CastDeviceType, CastDiscovery};

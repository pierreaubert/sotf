//! Internationalization (i18n) system for the GPUI audio player.
//!
//! Provides translations for multiple languages.

mod desktop;
mod language;
mod pseudo;
mod runtime_messages;
mod translations;

pub use desktop::*;
pub use language::*;
pub use pseudo::*;
pub use runtime_messages::*;
pub use translations::*;

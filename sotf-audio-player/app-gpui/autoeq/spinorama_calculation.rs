//! Spinorama room correction calculations - GPUI re-export
//!
//! Re-exports the spinorama calculation types from the common library.
//! The actual implementation is in sotf_audio_player::autoeq::spinorama.

pub use sotf_audio_player::autoeq::{
    calculate_room_correction, MeasurementCurve, RoomCorrectionInput, RoomMeasurement,
};

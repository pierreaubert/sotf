//! Device profiles for common MIDI hardware
//!
//! This module provides pre-configured device profiles for popular audio hardware,
//! making it easy to control devices without manually mapping MIDI messages.

pub mod rme_totalmix;
pub mod genelec_glm;
pub mod xone_k2;
pub mod launch_control_xl;

pub use rme_totalmix::{RMETotalMixProfile, TotalMixControl, TotalMixRow};
pub use genelec_glm::{GenelecGLMProfile, GLMControl};
pub use xone_k2::XoneK2Profile;
pub use launch_control_xl::{LaunchControlXLProfile, LCXLTemplate};

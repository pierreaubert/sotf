//! Device profiles for common MIDI hardware
//!
//! This module provides pre-configured device profiles for popular audio hardware,
//! making it easy to control devices without manually mapping MIDI messages.

pub mod genelec_glm;
pub mod launch_control_xl;
pub mod rme_totalmix;
pub mod xone_k2;

pub use genelec_glm::{GLMControl, GenelecGLMProfile};
pub use launch_control_xl::{LCXLTemplate, LaunchControlXLProfile};
pub use rme_totalmix::{RMETotalMixProfile, TotalMixControl, TotalMixRow};
pub use xone_k2::XoneK2Profile;

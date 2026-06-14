//! Multiband Compressor Plugin UI Component
//!
//! Layout (3-column):
//! +------------------+--------------------------------------------+------------------+
//! | GLOBAL           | BAND VIEW                                  | OUTPUT           |
//! |                  |                                            |                  |
//! | [Bands]    knob  | [Global] [1] [2] [3] ... tabs              | [Mix]      knob  |
//! | [XOver 1]  knob  | Per band:                                  | [Link Ch]  tog   |
//! | [XOver 2]  knob  | [Thresh] [Ratio] [Attack] [Release]        |                  |
//! | [XOver 3]  knob  | [Knee] [Makeup] [Solo] [Bypass]            |                  |
//! | [XOver 4]  knob  |                                            |                  |
//! +------------------+--------------------------------------------+------------------+

mod misc;
mod types;

pub use types::*;

//! Multiband Expander Plugin UI Component
//!
//! Layout (3-column):
//! +------------------+--------------------------------------------+------------------+
//! | GLOBAL           | BAND VIEW                                  | OUTPUT           |
//! |                  |                                            |                  |
//! | [Bands]    knob  | [Global] [1] [2] [3] ... tabs              | [Mix]      knob  |
//! | [XOver 1]  knob  | Per band:                                  | [Link Ch]  tog   |
//! | [XOver 2]  knob  | [Thresh] [Ratio] [Knee]                    |                  |
//! | [XOver 3]  knob  | [Attack] [Release] [Hold] [Range] [Hyst]   |                  |
//! | [XOver 4]  knob  | [Solo] [Bypass]                            |                  |
//! +------------------+--------------------------------------------+------------------+

mod misc;
mod types;

pub use types::*;

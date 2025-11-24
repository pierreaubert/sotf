#![doc = include_str!("../README.md")]

/// Conditional println macro that only prints when not in QA mode
#[macro_export]
macro_rules! qa_println {
    // Without args parameter - always print (for contexts without args access)
    // This pattern must come first to match string literals
    ($fmt:literal) => {
        println!($fmt);
    };
    ($fmt:literal, $($arg:expr),* $(,)?) => {
        println!($fmt, $($arg),*);
    };
    // With args parameter - conditional printing
    ($args:expr, $fmt:literal) => {
        if $args.qa.is_none() {
            println!($fmt);
        }
    };
    ($args:expr, $fmt:literal, $($arg:expr),* $(,)?) => {
        if $args.qa.is_none() {
            println!($fmt, $($arg),*);
        }
    };
}

// Re-export external crate functionality
pub use autoeq_cea2034 as cea2034;
pub use autoeq_de as de;
pub use autoeq_iir as iir;

// Re-export Curve from CEA2034 crate to ensure type compatibility
pub use autoeq_cea2034::Curve;

/// Common CLI argument definitions shared across binaries
pub mod cli;
/// Constraint functions for optimization
pub mod constraints;
/// Sobol initialisation
pub mod init_sobol;
/// Smart initial guess generation
pub mod initial_guess;
/// Loss functions for optimization
pub mod loss;
/// Optimization algorithms and objective functions
pub mod optim;
/// AutoEQ DE-specific optimization code
pub mod optim_de;
/// Metaheuristics-specific optimization code
pub mod optim_mh;
/// NLOPT-specific optimization code
#[cfg(feature = "nlopt")]
pub mod optim_nlopt;
/// Parameter vector utilities for different PEQ models
pub mod param_utils;
/// Plotting and visualization functions
pub mod plot;
/// Data reading and parsing functions
pub mod read;
/// Signal processing utilities
pub mod signal;
/// Shared workflow steps used by binaries
pub mod workflow;
/// Mapping
pub mod x2peq;

// Re-export commonly used items
pub use cli::*;
pub use loss::{HeadphoneLossData, LossType, SpeakerLossData};
pub use optim::*;
pub use plot::*;
pub use read::*;
pub use workflow::*;
pub use x2peq::x2peq;

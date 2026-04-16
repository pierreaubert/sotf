//! Cranelift-based JIT compiler for Emacs Lisp bytecode.
//!
//! Compiles hot bytecode functions to native machine code.
//! Feature-gated behind the `jit` feature flag.
//!
//! Architecture:
//! - A [`Profiler`] tracks invocation counts per bytecode function.
//!   When a function crosses the hot threshold, it is handed to the
//!   [`JitCompiler`] for compilation.
//! - The compiler lowers each bytecode opcode to Cranelift IR, emitting
//!   type-guard checks on the fast path and deoptimization exits that
//!   fall back to the bytecode VM when assumptions are violated (e.g.
//!   a function is redefined at runtime).

#[cfg(feature = "jit")]
mod compiler;
mod profiler;

#[cfg(feature = "jit")]
pub use compiler::{JitCompiler, NativeResult};
pub use profiler::Profiler;

#[cfg(not(feature = "jit"))]
pub enum NativeResult {
    Ok(u64),
    Deoptimize,
}

/// No-op JIT compiler stub when the `jit` feature is disabled.
/// Allows callers to unconditionally hold a `JitCompiler` without
/// feature-gating every use site.
#[cfg(not(feature = "jit"))]
pub struct JitCompiler;

#[cfg(not(feature = "jit"))]
impl JitCompiler {
    pub fn new() -> Self {
        JitCompiler
    }

    /// Returns `false` -- native compilation is not available.
    pub fn is_available() -> bool {
        false
    }
}

#[cfg(not(feature = "jit"))]
impl Default for JitCompiler {
    fn default() -> Self {
        Self::new()
    }
}

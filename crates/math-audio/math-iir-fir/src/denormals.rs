//! Utilities for handling floating-point denormals (subnormals).
//!
//! This module provides a scoped guard to enable "Flush-to-Zero" (FTZ) and
//! "Denormals-Are-Zero" (DAZ) modes on supported architectures.
//!
//! On x86/x86_64 the guard sets both FTZ and DAZ in MXCSR. DAZ is available on
//! SSE2-era hardware, which covers x86_64 targets; on unsupported hardware the
//! bit is ignored by the processor. On aarch64 the guard sets FPCR.FZ. Other
//! architectures use a no-op guard so audio processing code can keep one call
//! site across targets.
//!
//! # Usage
//!
//! ```rust
//! use math_audio_iir_fir::denormals::ScopedFlushToZero;
//!
//! {
//!     let _guard = ScopedFlushToZero::new();
//!     // ... heavy floating point processing ...
//!     // Denormals will be treated as zero here
//! }
//! // Previous state restored
//! ```

// MXCSR bit masks - defined here for cross-platform compatibility
// (not all platforms expose these constants in std::arch)
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
const FTZ_ON: u32 = 0x8000; // Flush-to-Zero bit (bit 15)
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
const DAZ_ON: u32 = 0x0040; // Denormals-Are-Zero bit (bit 6)

#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[inline(always)]
unsafe fn get_mxcsr() -> u32 {
    let mut val: u32 = 0;
    unsafe {
        std::arch::asm!("stmxcsr [{}]", in(reg) &mut val, options(nostack, preserves_flags));
    }
    val
}

#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[inline(always)]
unsafe fn set_mxcsr(val: u32) {
    unsafe {
        std::arch::asm!("ldmxcsr [{}]", in(reg) &val, options(nostack, preserves_flags));
    }
}

/// A guard that enables Flush-to-Zero (FTZ) and Denormals-Are-Zero (DAZ)
/// when instantiated, and restores the previous state when dropped.
///
/// This is useful for optimizing audio processing loops where denormal numbers
/// (extremely small values close to zero) can cause significant performance penalties.
pub struct ScopedFlushToZero {
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    original_csr: u32,
    #[cfg(target_arch = "aarch64")]
    original_fpcr: u64,
    #[cfg(not(any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64")))]
    _dummy: (),
}

impl ScopedFlushToZero {
    /// Create a new guard that enables FTZ/DAZ mode on the current thread.
    pub fn new() -> Self {
        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        unsafe {
            let original_csr = get_mxcsr();
            let new_csr = original_csr | FTZ_ON | DAZ_ON;
            set_mxcsr(new_csr);

            ScopedFlushToZero { original_csr }
        }

        #[cfg(target_arch = "aarch64")]
        unsafe {
            let original_fpcr: u64;
            std::arch::asm!("mrs {}, fpcr", out(reg) original_fpcr);

            // Bit 24 is FZ (Flush-to-Zero)
            let new_fpcr = original_fpcr | (1 << 24);

            std::arch::asm!("msr fpcr, {}", in(reg) new_fpcr);

            ScopedFlushToZero { original_fpcr }
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64")))]
        {
            ScopedFlushToZero { _dummy: () }
        }
    }
}

impl Drop for ScopedFlushToZero {
    fn drop(&mut self) {
        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        unsafe {
            set_mxcsr(self.original_csr);
        }

        #[cfg(target_arch = "aarch64")]
        unsafe {
            std::arch::asm!("msr fpcr, {}", in(reg) self.original_fpcr);
        }
    }
}

impl Default for ScopedFlushToZero {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scoped_flush_to_zero_creation() {
        let _guard = ScopedFlushToZero::new();
        // Just verify it doesn't crash on the current architecture
    }

    // It's hard to unit test the actual effect without generating denormals,
    // which the compiler might optimize away or handle differently.
    // We rely on the register manipulation logic being correct.
}

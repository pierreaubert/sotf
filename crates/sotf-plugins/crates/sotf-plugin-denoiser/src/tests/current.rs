#[cfg(target_arch = "x86_64")]
pub(super) fn current_fpu_control() -> u64 {
    let mut mxcsr = 0_u32;
    unsafe {
        std::arch::asm!(
            "stmxcsr [{}]",
            in(reg) &mut mxcsr,
            options(nostack, preserves_flags)
        );
    }
    mxcsr as u64
}

#[cfg(target_arch = "aarch64")]
pub(super) fn current_fpu_control() -> u64 {
    let fpcr: u64;
    unsafe {
        std::arch::asm!("mrs {}, fpcr", out(reg) fpcr);
    }
    fpcr
}

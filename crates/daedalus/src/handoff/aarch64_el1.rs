//! Handoff mechanism for aarch64 at EL1 to LionsOS

/// Hands control to the payload staged at `address`
///
/// This is specifically for `aarch64` architecture running at EL1
///
/// # Safety
///
/// Must be the last thing Daedalus ever does.
///
/// All `Daedalus` IRQ's Memory etc. must be toredown
pub unsafe fn aarch64_el1_handoff(address: usize) -> ! {
    // # Safety
    //
    // As per the header comment, this is the last thing that
    // `Daedalus` ever does, and does not return from this..
    unsafe {
        core::arch::asm!(
            // Mask DAIF (no more IRQ's)
            "msr daifset, #0xf",

            // Drop our vector table, so if seL4 unmasks or something
            // it doesn't go to our bootloader stuff.
            "msr vbar_el1, xzr",
            "isb",
            "dsb sy",

            // Jump to the handoff address
            "br {addr}",
            addr = in(reg) address,
            options(noreturn, nostack)
        );
    }
}

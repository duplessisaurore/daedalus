//! The specific handoff mechanism for a
//! target platform is chosen from here
//!
//! This runs after all the daedalus teardown.

cfg_if::cfg_if! {
    if #[cfg(target_arch = "aarch64")] {
        mod aarch64_el1;
        mod aarch64_el2;

        /// Call correct corresponding handoff function based on CurrentEL
        /// 
        /// This handles all proper aarch64 handoff configurations.
        pub unsafe fn arch_handoff(address: usize) -> ! {
            let current_el: u64;
            unsafe {
                core::arch::asm!("mrs {}, CurrentEL", out(reg) current_el, options(nomem, nostack));
                match (current_el >> 2) & 3 {
                    1 => aarch64_el1::aarch64_el1_handoff(address),
                    2 => aarch64_el2::aarch64_el2_handoff(address),
                    _ => unreachable!("invalid kernel handoff EL"),
                }
            }
        }
    } else {
        compile_error!("no target platform selected with support for handoff; enable an option (or add one if it doesn't exist!)");
    }
}
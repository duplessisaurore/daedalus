//! All implemented platform specific startup
//! sequences are included here depending on the platform.

use core::arch::global_asm;

cfg_if::cfg_if! {
    // zynqmp, always the same startup regardless.
    if #[cfg(all(target_arch = "aarch64", feature = "zynqmp"))] {
        global_asm!(include_str!("../startup/zynqmp/startup.S"));
    } else {
        compile_error!("no target platform selected; enable an option (or add one if it doesn't exist!)");
    }
}

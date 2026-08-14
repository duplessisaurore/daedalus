//! The specific handoff mechanism for a
//! target platform is chosen from here
//!
//! This runs after all the daedalus teardown.

cfg_if::cfg_if! {
    // zynqmp is el2 aarch64
    if #[cfg(all(target_arch = "aarch64", feature = "platform-zynqmp"))] {
        mod aarch64_el2;
        pub use aarch64_el2::aarch64_el2_handoff as arch_handoff;
    } else {
        compile_error!("no target platform selected with support for handoff; enable an option (or add one if it doesn't exist!)");
    }
}

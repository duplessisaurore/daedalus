//! All implemented architectures lie in this
//! submodule for IRQ ops
//! 
//! This module exports based on the target_arch and
//! sometimes the controller the IRQ ops.

cfg_if::cfg_if! {
    if #[cfg(all(target_arch = "aarch64", feature = "gicv2"))] {
        mod gicv2;
        pub use gicv2::GICv2 as TargetIRQArch;
    } else {
        compile_error!("no target IRQ architecture selected when IRQs were attempted to be used; enable an option");
    }
}
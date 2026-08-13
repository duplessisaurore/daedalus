//! All implemented platform specific things
//! exist within this submodule
//!
//! Generally some more generic higher level
//! `IrqArch` can be implemented in `archs`,
//! but the platform specific things are here.

cfg_if::cfg_if! {
    // GICv2
    // This requires some base address which is platform specific.
    //
    // each platform must export GIC_DISTRIBUTOR_BASE and GIC_CPU_INTERFACE_BASE
    // as usize consts.
    if #[cfg(all(target_arch = "aarch64", feature = "irq-gicv2", feature = "platform-zynqmp"))] {
        mod zynqmp;
        pub use zynqmp::GICD_BASE as GIC_DISTRIBUTOR_BASE;
        pub use zynqmp::GICC_BASE as GIC_CPU_INTERFACE_BASE;
    } else {
        compile_error!("no target IRQ platform selected when IRQs were attempted to be used; enable an option (or add one if it doesn't exist!)");
    }
}

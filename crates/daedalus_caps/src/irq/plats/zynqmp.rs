//! Platform support for the:
//! Xilinx Zynq UltraScale+ MPSoC (ZynqMP)
//!
//! Example boards:
//!     - ZCU106
//!

/// Base address of the GICD distributor
pub const GICD_BASE: usize = 0xF901_0000;

/// Base address of the GICC cpu interface
pub const GICC_BASE: usize = 0xF902_0000;

/// Size of the GICD MMIO region
pub const GICD_SIZE: usize = 0x1_0000;

/// Size of the GICC MMIO region
pub const GICC_SIZE: usize = 0x1_000;

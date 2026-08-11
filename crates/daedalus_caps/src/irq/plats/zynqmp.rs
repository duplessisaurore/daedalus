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

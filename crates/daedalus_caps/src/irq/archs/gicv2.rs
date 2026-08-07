//! The architecture specific IRQ operations
//! for `aarch64`, specifically `GICv2` as the interrupt
//! controller.

use crate::irq::arch::IrqArch;

/// First valid interrupt ID
///
/// 0-15 are software.
const FIRST_DEVICE_INTERRUPT_ID: u32 = 16;

/// First special IntIDS for GICv2, this is the
/// upper bound inherently for interrupt ids
const FIRST_SPECIAL_INTERRUPT_ID: u32 = 1020;

/// The `GICv2` memory operations
///
/// These are for `ARM 64-bit` platforms
/// that have specifically a `GICv2` interrupt
/// controller such as the ZCU106.
pub struct GICv2;

impl IrqArch for GICv2 {
    fn is_valid_irq(id: u32) -> bool {
        Self::is_valid_irq(id)
    }
}

impl GICv2 {
    pub const fn is_valid_irq(id: u32) -> bool {
        FIRST_DEVICE_INTERRUPT_ID <= id && id < FIRST_SPECIAL_INTERRUPT_ID
    }
}

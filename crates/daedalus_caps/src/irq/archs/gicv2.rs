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
    
    type InterruptState = InterruptState;
    
    unsafe fn disable_interrupts() -> Self::InterruptState {
        // The DAIF is the state we need to preserve
        // as we disable interrupts through masking everything
        let daif: u64;

       unsafe {
            // Store the current state of the DAIF reg
            core::arch::asm!("mrs {}, daif", out(reg) daif, options(nomem, nostack));

            // Set bit 1 (IRQ bit) so we mask all IRQ's
            core::arch::asm!("msr daifset, #2", options(nomem, nostack));
        }

        // This state will be restored later.
        InterruptState(daif)
    }
    
    unsafe fn restore_interrupts(state: Self::InterruptState) {
        unsafe {
            // Restore the daif state back
            core::arch::asm!("msr daif, {}", in(reg) state.0, options(nomem, nostack));
        }
    }
}

impl GICv2 {
    pub const fn is_valid_irq(id: u32) -> bool {
        FIRST_DEVICE_INTERRUPT_ID <= id && id < FIRST_SPECIAL_INTERRUPT_ID
    }
}

/// The interrupt state for a `GICv2` controller. This is the
/// state of the DAIF which we mask everything to disable interrupts
struct InterruptState(u64);
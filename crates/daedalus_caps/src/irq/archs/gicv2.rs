//! The architecture specific IRQ operations
//! for `aarch64`, specifically `GICv2` as the interrupt
//! controller.

use crate::irq::{
    arch::IrqArch,
    plats::{GIC_CPU_INTERFACE_BASE, GIC_DISTRIBUTOR_BASE},
};

/// First valid interrupt ID
///
/// 0-15 are software.
const FIRST_DEVICE_INTERRUPT_ID: u32 = 16;

/// First special IntIDS for GICv2, this is the
/// upper bound inherently for interrupt ids
const FIRST_SPECIAL_INTERRUPT_ID: u32 = 1020;

/// The first shared peripheral interrupt's ID
///
/// Below this is private peripheral interrupts,
/// which are per-core/CPU.
const FIRST_SHARED_INTERRUPT_ID: u32 = 32;

// Distributer registers
pub enum GICDRegisters {
    /// Control register
    ///
    /// Enables forwarding of interrupts from the
    /// distributor to CPU interfaces
    CTLR,

    /// Information about the GIC
    ///
    /// This includes how many lines the GIC actually supports
    /// (up to the max)
    TYPER,

    /// provide a Set-enable bit for each interrupt supported by the GIC.
    ///
    /// Writing 1 to a Set-enable bit enables forwarding of
    /// the corresponding interrupt from the Distributor to
    /// the CPU interfaces.
    ///
    /// Reading a bit identifies whether the interrupt is enabled.
    ISENABLER,

    /// provide a Clear-enable bit for each interrupt supported by the GIC.
    ///
    /// Writing 1 to a Clear-enable bit disables forwarding of
    /// the corresponding interrupt from the Distributor to
    /// the CPU interfaces.
    ///
    /// Reading a bit identifies whether the interrupt is enabled.
    ICENABLER,

    /// provide a Clear-pending bit for each interrupt supported by the GIC.
    ///
    /// Writing 1 to a Clear-pending bit clears the pending state
    /// of the corresponding peripheral interrupt.
    ///
    /// Reading a bit identifies whether the interrupt is pending.
    ICPENDR,

    /// provide a Clear-active bit for each interrupt that the GIC supports.
    ///
    /// Writing to a Clear-active bit Deactivates the corresponding
    /// interrupt.
    ///
    /// These registers are used when preserving and restoring GIC state.
    ICACTIVER,

    /// provide an 8-bit priority field for each interrupt supported by the GIC.
    ///
    /// This field stores the priority of the corresponding interrupt.
    IPRIORITYR,

    /// provide an 8-bit CPU targets field for each interrupt supported by the GIC.
    ///
    /// This field stores the list of target processors for the interrupt.
    ///
    /// That is, it holds the list of CPU interfaces to which the Distributor
    /// forwards the interrupt if it is asserted and has sufficient priority.
    ITARGETSR,

    /// provide a 2-bit Int_config field for each interrupt supported by the GIC.
    ///
    /// This field identifies whether the corresponding interrupt
    /// is edge-triggered or level-sensitive
    ICFGR,
}

/// CPU Interface Registers
pub enum GICCRegisters {
    /// Control register
    ///
    /// Enables the signaling of interrupts by the CPU interface to
    /// the connected processor
    CTLR,

    /// Interrupt priority filter.
    ///
    /// Only interrupts with a higher priority than the value in this
    /// register are signaled to the core/CPU.
    PMR,

    /// Interrupt acknowledge register.
    ///
    /// The processor reads this register to obtain the interrupt ID
    /// of the signaled interrupt.
    ///
    /// This read acts as an acknowledge for the interrupt (that
    /// it has begun processing it)
    IAR,

    /// We write to this register to inform the CPU
    /// interface to drop the priority level back down
    /// bcz acknowledging the interrupt raises the prio level
    /// and to actually clear the active state so it can be
    /// delivered again.
    EOIR,
}

/// The `GICv2` memory operations
///
/// These are for `ARM 64-bit` platforms
/// that have specifically a `GICv2` interrupt
/// controller such as the ZCU106.
pub struct GICv2;

impl IrqArch for GICv2 {
    fn is_valid_irq_wrapper(id: u32) -> bool {
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
pub struct InterruptState(u64);

impl GICCRegisters {
    /// Converts this GICC register type
    /// to it's actual offset off the GICC_BASE/GIC_CPU_INTERFACE_BASE
    pub const fn to_offset(&self) -> usize {
        match self {
            GICCRegisters::CTLR => 0x0,
            GICCRegisters::PMR => 0x4,
            GICCRegisters::IAR => 0xC,
            GICCRegisters::EOIR => 0x10,
        }
    }
}

impl GICDRegisters {
    /// Converts this GICD register type
    /// to it's actual offset off the GICD_BASE/GIC_DISTRIBUTOR_BASE
    pub const fn to_offset(&self) -> usize {
        match self {
            GICDRegisters::CTLR => 0x0,
            GICDRegisters::TYPER => 0x4,
            GICDRegisters::ISENABLER => 0x100,
            GICDRegisters::ICENABLER => 0x180,
            GICDRegisters::ICPENDR => 0x280,
            GICDRegisters::ICACTIVER => 0x380,
            GICDRegisters::IPRIORITYR => 0x400,
            GICDRegisters::ITARGETSR => 0x800,
            GICDRegisters::ICFGR => 0xC00,
        }
    }
}
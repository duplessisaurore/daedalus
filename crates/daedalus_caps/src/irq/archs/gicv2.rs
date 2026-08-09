//! The architecture specific IRQ operations
//! for `aarch64`, specifically `GICv2` as the interrupt
//! controller.

use daedalus_program::{InterruptPriority, InterruptTrigger};

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

/// The initial PMR priority mask value.
///
/// The PMR registser disallows anything below its set value
/// of priority, which is initialised initially to 0 (so everything
/// explodes!) we set it to 0xFF to permit everything
const PMR_PRIORITY_MASK: u32 = 0xFF;

/// We assume a single-core environment
///
/// This means our bootloader should be running on core zero,
/// which is targetted by setting ITARGETSR bit 0x01.
const TARGET_CPU0: u8 = 0x01;

/// The field to read from GICD_TYPER to read
/// out the number of interrupt lines
const GICD_TYPER_ITLINESNUMBER_MASK: u32 = 0x1F;

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

/// Mapping from `InterruptTrigger` onto GICTrigger levels.
pub enum GICTriggerMapping {
    Level,
    Edge,
}

/// Mapping from `InterruptPriority` onto GICv2 Prior levels.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GICInterruptPriority(u8);

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

    /// Read this CPU interface register
    ///
    /// # Safety
    ///
    /// the GIC must be like actually there lol
    ///
    /// Whenever the `IAR` register is read, this can cause
    /// a side effect, which actively acknowledges the interrupt and
    /// moves it into the `active` state (the cpu is assumed to be
    /// "dealing with it").
    pub unsafe fn read(&self) -> u32 {
        unsafe {
            core::ptr::read_volatile((GIC_CPU_INTERFACE_BASE + self.to_offset()) as *const u32)
        }
    }

    /// Write to this CPU interface register
    ///
    /// # Safety
    ///
    /// the GIC must be like actually there lol
    pub unsafe fn write(&self, value: u32) {
        unsafe {
            core::ptr::write_volatile(
                (GIC_CPU_INTERFACE_BASE + self.to_offset()) as *mut u32,
                value,
            )
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

    /// Read this distributer register
    ///
    /// # Safety
    ///
    /// the GIC must be like actually there lol
    pub unsafe fn read(&self) -> u32 {
        unsafe { core::ptr::read_volatile((GIC_DISTRIBUTOR_BASE + self.to_offset()) as *const u32) }
    }

    /// Write to this distributer register
    ///
    /// # Safety
    ///
    /// the GIC must be like actually there lol
    pub unsafe fn write(&self, value: u32) {
        unsafe {
            core::ptr::write_volatile((GIC_DISTRIBUTOR_BASE + self.to_offset()) as *mut u32, value)
        }
    }
}

impl GICTriggerMapping {
    /// Converts this value to the ICFGR bit value for
    /// this trigger
    pub const fn to_bit_value(&self) -> u32 {
        match self {
            GICTriggerMapping::Level => 0b00,
            GICTriggerMapping::Edge => 0b10,
        }
    }
}

impl From<InterruptTrigger> for GICTriggerMapping {
    fn from(value: InterruptTrigger) -> Self {
        match value {
            InterruptTrigger::Level => Self::Level,
            InterruptTrigger::Edge => Self::Edge,
        }
    }
}

impl From<InterruptPriority> for GICInterruptPriority {
    fn from(value: InterruptPriority) -> Self {
        // Map the priority down.
        //
        // The direction is the same here (0 is higher prio)
        // but a newtype makes sure we use the correct priority
        // and not some other IRQArchs
        //
        // The GICv2 automatically "compresses" it down into
        // the actual hardware levels too so not much we have to do here.
        GICInterruptPriority(value.0)
    }
}

/// How many interrupt lines this distributor actually implements
/// on the platform.
///
/// This returns the max number of SPI interrupt ids
/// (shared peripheral interrupts) as the 32 * (value from the field + 1)
///
/// We clamp the number of lines to FIRST_SPECIAL_INTERRUPT_ID to avoid
/// accidentally obliterating those oopsies ><
///
/// # Safety
///
/// The GIC must actually exist lol.
unsafe fn line_count() -> u32 {
    // # Safety
    // Safety precondition is preserved by header
    let it_lines = unsafe { GICDRegisters::TYPER.read() } & GICD_TYPER_ITLINESNUMBER_MASK;
    ((it_lines + 1) * 32).min(FIRST_SPECIAL_INTERRUPT_ID)
}

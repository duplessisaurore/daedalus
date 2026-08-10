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
/// of priority (with 0 being "highest" and 255 being "lowest"),
/// which is initialised initially to 0 (so everything explodes!)
/// we set it to 0xFF to permit everything
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

    unsafe fn setup() {
        
    }

    unsafe fn teardown() {
        // # Safety
        // 
        // This is called exactly once, so it doesn't really matter,
        // and theres no more irq arch ops so its like okay ^_^
        //
        // This is also called after `setup` and only ran on one core,
        // so these ops are safe.
        unsafe {
            core::arch::asm!("msr daifset, #2", options(nomem, nostack));
            
            // reset state for all lines
            let lines = line_count();

            for interrupt_id in FIRST_DEVICE_INTERRUPT_ID..lines {
                // This is a bit-per-interrupt register.
                // Un-enable all the interrupts.
                GICDRegisters::ICENABLER.write_interrupt_bit(interrupt_id);
            
                // This is a bit-per-interrupt register.
                // Clear the pending state of all interrupts.
                GICDRegisters::ICPENDR.write_interrupt_bit(interrupt_id);
            
                // This is a bit-per-interrupt register.
                // Clear all of the active state interrupts too,
                // this is bcz the active prior is set
                GICDRegisters::ICACTIVER.write_interrupt_bit(interrupt_id);
            }

            // Clear all the CTLR to stop signaling for everything
            GICCRegisters::CTLR.write(0);
            GICDRegisters::CTLR.write(0);
        }
    }

    unsafe fn configure(interrupt_id: u32, trigger: InterruptTrigger, priority: InterruptPriority) {
        // Write the trigger mode out for this interrupt
        //
        // # Safety
        //
        // The gic must be setup because this function
        // requires that `setup` has ran first.
        //
        // ICFGR is a two-bit-per-interrupt register.
        unsafe {
            GICDRegisters::ICFGR.write_interrupt_two_bit_register(
                interrupt_id,
                GICTriggerMapping::from(trigger).to_bit_value(),
            );
        }

        // Write the priority out for this interrupt.
        //
        // # Safety
        //
        // The gic must be setup because this function
        // requires that `setup` has ran first.
        //
        // IPRIORITYR is a byte-per-interrupt register.
        unsafe {
            GICDRegisters::IPRIORITYR.write_interrupt_byte_register(
                interrupt_id,
                GICInterruptPriority::from(priority).0,
            );
        }

        // Write the target out for this interrupt, so
        // it targets our cpu.
        //
        // # Safety
        //
        // The gic must be setup because this function
        // requires that `setup` has ran first.
        //
        // ITARGETSR is a byte-per-interrupt register.
        unsafe {
            GICDRegisters::ITARGETSR.write_interrupt_byte_register(interrupt_id, TARGET_CPU0);
        }
    }

    unsafe fn mask(interrupt_id: u32) {
        // # Safety
        //
        // The ICENABLER is a clear-enable bit, so we are clearing it.
        // which disables the interrupt (its an interrupt bit)
        //
        // gic enabled by the setup, which preconditions of this function
        // require first.
        unsafe {
            GICDRegisters::ICENABLER.write_interrupt_bit(interrupt_id);
        }
    }

    unsafe fn unmask(interrupt_id: u32) {
        // # Safety
        //
        // The ISENABLER is a set-enable bit, so we are setting it.
        // which enables the interrupt (its an interrupt bit)
        //
        // gic enabled by the setup, which preconditions of this function
        // require first.
        unsafe {
            GICDRegisters::ISENABLER.write_interrupt_bit(interrupt_id);
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
        unsafe { self.read_with_offset(0) }
    }

    /// Read this distributer register with some offset
    ///
    /// # Safety
    ///
    /// the GIC must be like actually there lol
    unsafe fn read_with_offset(&self, offset: usize) -> u32 {
        unsafe {
            core::ptr::read_volatile(
                (GIC_DISTRIBUTOR_BASE + self.to_offset() + offset) as *const u32,
            )
        }
    }

    /// Write to this distributer register
    ///
    /// # Safety
    ///
    /// the GIC must be like actually there lol
    pub unsafe fn write(&self, value: u32) {
        unsafe {
            self.write_with_offset(0, value);
        }
    }

    /// Write to this distributer register with some offset
    ///
    /// # Safety
    ///
    /// the GIC must be like actually there lol
    unsafe fn write_with_offset(&self, offset: usize, value: u32) {
        unsafe {
            core::ptr::write_volatile(
                (GIC_DISTRIBUTOR_BASE + self.to_offset() + offset) as *mut u32,
                value,
            )
        }
    }

    /// This essentailly writes a byte to a byte-per-interrupt
    /// GICD register.
    ///
    /// To do this we need to read out the register's value (so
    /// we can use the consistent form of `GICDRegisters:;read`), update
    /// the byte, write it back out (also consistently using `GICDRegisters::write`).
    ///
    /// I do think GICv2 lets you do byte accesses but eh what the hell.
    ///
    /// This nukes whatever value was in the byte !!!
    ///
    /// # Safety
    ///
    /// The gic must be like actually there and mapped in.
    ///
    /// The register we are writing to must be an actual byte-per-interrupt
    /// GICD register.
    pub unsafe fn write_interrupt_byte_register(&self, interrupt_id: u32, byte_value: u8) {
        // round down index to nearest 4
        // since we grab a 32 bit chunk at once (4 interrupt ids)
        let index = (interrupt_id & !0x3) as usize;

        // this is the shift to the specific byte in the register
        // that this interrupt id corresponds to
        let shift = (interrupt_id & 0x3) * 8;

        // Read the full chunk
        //
        // # Safety
        //
        // precondition preserved by safety header of this function.
        let mut chunk = unsafe { self.read_with_offset(index) };

        // Clear out the existing byte in the chunk
        chunk &= !(0xFF << shift);

        // Put the new value byte
        chunk |= u32::from(byte_value) << shift;

        // Write back out
        //
        // # Safety
        //
        // precondition preserved by safety header of this function.
        unsafe {
            self.write_with_offset(index, chunk);
        }
    }

    /// This essentailly writes a bit to a bit-per-interrupt
    /// GICD register.
    ///
    /// These are just a write 1 to set register, so we only
    /// need to write the corresponding bit at the interrupt ID.
    ///
    /// # Safety
    ///
    /// The gic must be like actually there and mapped in.
    ///
    /// The register we are writing to must be an actual bit-per-interrupt
    /// GICD register.
    unsafe fn write_interrupt_bit(&self, interrupt_id: u32) {
        unsafe {
            // Get the index and bit for this specific interrupt id
            let index = ((interrupt_id & !0x1F) as usize) >> 3;
            let bit = 1u32 << (interrupt_id & 0x1F);

            self.write_with_offset(index, bit);
        }
    }

    /// This essentailly writes a two-bit field to a two-bits-per-interrupt
    /// GICD register.
    ///
    /// This is similar to `write_byte_register` because the value is held
    /// there rather than having a set/clear bit-wise variant.
    ///
    /// This nukes whatever value was in the byte !!!
    ///
    /// # Safety
    ///
    /// The gic must be like actually there and mapped in.
    ///
    /// The register we are writing to must be an actual
    /// two-bits-per-interrupt GICD register.
    unsafe fn write_interrupt_two_bit_register(&self, interrupt_id: u32, two_bit_value: u8) {
        // 16 interrupts per 32-bit register.
        let index = ((interrupt_id & !0xF) as usize) >> 2;

        // Two bits, this is the shift to our specific interrupt
        let shift = (interrupt_id & 0xF) * 2;

        // Read the full chunk
        //
        // # Safety
        //
        // precondition preserved by safety header of this function.
        let mut chunk = unsafe { self.read_with_offset(index) };

        // Clear out the existing field in the chunk
        chunk &= !(0b11 << shift);

        // Put the new field value in (ensure its only the first 2 bits)
        chunk |= u32::from(two_bit_value & 0b11) << shift;

        // Write back out
        //
        // # Safety
        //
        // precondition preserved by safety header of this function.
        unsafe {
            self.write_with_offset(index, chunk);
        }
    }
}

impl GICTriggerMapping {
    /// Converts this value to the ICFGR bit value for
    /// this trigger
    pub const fn to_bit_value(&self) -> u8 {
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

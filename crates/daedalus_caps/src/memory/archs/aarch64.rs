//! The architecture specific memory operations
//! for `aarch64`.
//!
//! This supplies the necessary underlying
//! arch specific ops for memory caps.

use crate::memory::{
    __dram_end,
    arch::{AccessWidth, MemoryArch},
};

/// Right shift of the CTR_EL0 register required
/// for reading `DMinLine` (min size of d-cache line)
const DMINLINE_SHIFT: u64 = 16;

/// Right shift of the CTR_EL0 register required
/// for reading `IMinLine` (min size of i-cache line)
const IMINLINE_SHIFT: u64 = 0;

/// Mask for the actual line size for the IMINLINE/DMINLINE shifts
const LINE_MASK: u64 = 0xf;

/// This is the `I` flag of the `SCTLR_EL1` register, setting this
/// to 1 will enable the instruction cache.
const SCTLR_INSTRUCTION_CACHE: u64 = 1 << 12;

/// This is the E2H bit of the `HCR_EL2` register
///
/// This enables a configuration where a Host Operating System is running at EL2,
/// and the Host Operating System's applications are running at EL0. (VHE)
const HCR_EL2_E2H: u64 = 1 << 34;

/// This is the TGE bit of the `HCR_EL2` register
///
/// This routes all exceptions from EL1 to EL2 if set to 1.
const HCR_EL2_TGE: u64 = 1 << 27;

/// The `AArch64` memory operations
///
/// These are for `ARM 64-bit` platforms
/// such as the ZCU106.
pub struct Aarch64;

/// Force a non-VHE EL2 configuration.
///
/// We want to force VHE off, as else EL1 register accesses are
/// redirected to their EL2 ones, which are bad because they have
/// different formats!
///
/// # Safety
///
/// Must run at EL2, before any memory/interrupt setup.
unsafe fn configure_hcr_el2() {
    unsafe {
        // Read current value to preserve fields.
        let mut hcr: u64;
        core::arch::asm!("mrs {}, hcr_el2", out(reg) hcr, options(nomem, nostack));

        // Get rid of E2H and TGE from hcr_el2.
        hcr &= !(HCR_EL2_E2H | HCR_EL2_TGE);

        // Write back to update.
        core::arch::asm!(
            "msr hcr_el2, {hcr}",
            "isb",
            hcr = in(reg) hcr,
            options(nomem, nostack, preserves_flags)
        );
    }
}

/// This will assert that the current ELx level is EL2.
///
/// This upholds that `Daedalus` should always be running at EL2.
fn assert_el2() {
    let current_el: u64;

    // # Safety:
    //
    // `CurrentEL` is readable at every exception level.
    unsafe {
        core::arch::asm!("mrs {}, CurrentEL", out(reg) current_el, options(nomem, nostack));
    }

    // CurrentEL holds the exception level in bits [3:2].
    let el = (current_el >> 2) & 0b11;
    assert!(
        el == 2,
        "daedalus must run at EL2, instead it was running at EL{el}!"
    );
}

impl MemoryArch for Aarch64 {
    unsafe fn read(pointer: *const u8, width: AccessWidth) -> u64 {
        // The `read` function invariants specify that this must be a safe
        // aligned access.
        unsafe {
            match width {
                AccessWidth::U8 => u64::from(pointer.read_volatile()),
                AccessWidth::U16 => u64::from((pointer as *const u16).read_volatile()),
                AccessWidth::U32 => u64::from((pointer as *const u32).read_volatile()),
                AccessWidth::U64 => (pointer as *const u64).read_volatile(),
            }
        }
    }

    unsafe fn write(pointer: *mut u8, width: AccessWidth, value: u64) {
        // The `write` function invariants specify that this must be a safe
        // aligned access.
        unsafe {
            match width {
                AccessWidth::U8 => pointer.write_volatile(value as u8),
                AccessWidth::U16 => (pointer as *mut u16).write_volatile(value as u16),
                AccessWidth::U32 => (pointer as *mut u32).write_volatile(value as u32),
                AccessWidth::U64 => (pointer as *mut u64).write_volatile(value),
            }
        }
    }

    unsafe fn copy(source: *const u8, destination: *mut u8, len: usize) {
        // Backwards-case, see comment of `copy`
        if (destination as usize) > (source as usize)
            && (destination as usize) < (source as usize) + len
        {
            for i in (0..len).rev() {
                unsafe {
                    destination
                        .add(i)
                        .write_volatile(source.add(i).read_volatile())
                };
            }

            return;
        }

        // Forward-case, see comment of `copy`
        for i in 0..len {
            unsafe {
                destination
                    .add(i)
                    .write_volatile(source.add(i).read_volatile())
            };
        }
    }

    unsafe fn fill(destination: *mut u8, value: u8, len: usize) {
        for i in 0..len {
            unsafe { destination.add(i).write_volatile(value) };
        }
    }

    /// We need to basically ensure that the kernel image we wrote
    /// can be visible to any non-cache coherent observer.
    ///
    /// same for the instruction yoinker :3c so we can actually execute it
    unsafe fn flush_range(pointer: *mut u8, len: usize) {
        // Get the cache type/info, this is stored in the CTR_EL0 register
        let cache_type: u64;
        let ptr_usize = pointer as usize;

        unsafe {
            core::arch::asm!("mrs {}, ctr_el0", out(reg) cache_type, options(nomem, nostack, preserves_flags));
        }

        // Read `DMinLine` and `IMinLine` for both cache types for flushing
        let dmin_line = 4usize << ((cache_type >> DMINLINE_SHIFT) & LINE_MASK);
        let imin_line = 4usize << ((cache_type >> IMINLINE_SHIFT) & LINE_MASK);

        // Round down pointer to the nearest starting/ending lines for d/i cache
        let dstart = ptr_usize & !(dmin_line - 1);
        let istart = ptr_usize & !(imin_line - 1);

        let end = ptr_usize + len;
        let dend = (end + dmin_line - 1) & !(dmin_line - 1);
        let iend = (end + imin_line - 1) & !(imin_line - 1);

        unsafe {
            // Clear each d-cache line between the start & end
            // we use cvac so non-cpu guys can see it too :) (all observers)
            for address in (dstart..dend).step_by(dmin_line) {
                core::arch::asm!("dc cvac, {}", in(reg) address, options(nostack, preserves_flags));
            }

            // ensure all previous memory ops before this barrier are done
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));

            // clear each i-cache line
            for address in (istart..iend).step_by(imin_line) {
                core::arch::asm!("ic ivau, {}", in(reg) address, options(nostack, preserves_flags));
            }

            // ensure ops completed
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));

            // flush the processor pipeline so that we fetch new instructions
            core::arch::asm!("isb", options(nostack, preserves_flags));
        }
    }

    /// The setup for aarch64 involves these steps:
    ///
    /// Generally we can't exactly trust the stage before `Daedalus`
    /// to not accidentally leave things in cache while `Daedalus` operates
    /// in a state with the MMU off.
    ///
    /// The issue with this is that `Daedalus`'s stores because we r currently
    /// in Device-nGnRnE memory everywhere we just bypass all caches and write
    /// straight out to DRAM.
    ///
    /// Later when the cache evicts the lines from the prior stage, this overwrites
    /// our stuff ! bad :(
    ///
    /// So we need to first clean and invalidate all cache lines before `Daedalus`
    /// and then we can begin with our execution!
    ///
    /// To do this we follow these steps:
    ///
    /// We do the braindead but simple approach of just going over all possible cache
    /// lines and clean + invalidating them.
    ///
    /// This is the almost the same as the `flush_range` but for all addresses on the
    /// system as we don't know which ones have data left in them.
    ///
    /// We also flush the entirety of the instruction cache with `ic ialluis`
    /// and then set the `I` field in `SCTLR_EL1` to enable the instruction cache such
    /// that our interpreter yoinking can be a lot faster execution wise.
    unsafe fn setup() {
        // Assert that we are actually running at EL2 on aarch64, as this is expected
        assert_el2();

        // Get rid of VHE,
        //
        // # Safety
        //
        // we've asserted we're in el2 and
        // this is at the top of memory setup which runs first.
        unsafe { configure_hcr_el2() };

        // We assume that __dram_end is literally the end of memory, so we
        // invalidate up to then :)
        let end = (&raw const __dram_end) as usize;

        // We cant flush range because we dont want to just clean,
        // we want to also invalidate!
        let cache_type: u64;

        unsafe {
            core::arch::asm!("mrs {}, ctr_el0", out(reg) cache_type, options(nomem, nostack, preserves_flags));
        }

        // Same steps as `flush_range`, read dmin_line to iterate over line size.
        let dmin_line = 4usize << ((cache_type >> DMINLINE_SHIFT) & LINE_MASK);

        unsafe {
            // Invalidate every address... PERFORMANCE BE DAMNED
            for address in (0..end).step_by(dmin_line) {
                core::arch::asm!(
                    "dc civac, {}",
                    in(reg) address,
                    options(nostack, preserves_flags)
                );
            }

            // Same as `flush_range`, but we invalidate the entire instruction cache
            // instead of only the ones in range, as its one instruction only.
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            core::arch::asm!("ic ialluis", options(nostack, preserves_flags));
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            core::arch::asm!("isb", options(nostack, preserves_flags));
        }

        // Enable instruction caching with `SCTLR_EL1`
        let mut system_control: u64;

        unsafe {
            // Read the current value out, as we only want to enable
            // the instruction cache
            core::arch::asm!(
                "mrs {}, sctlr_el1",
                out(reg) system_control,
                options(nomem, nostack, preserves_flags)
            );

            // Set flag and write back
            system_control |= SCTLR_INSTRUCTION_CACHE;

            core::arch::asm!(
                "msr sctlr_el1, {}",
                "isb",
                in(reg) system_control,
                options(nostack, preserves_flags)
            );
        }
    }

    /// Idk lol leave everything from setup 😂😂😂😂😂😂😂😂😂
    unsafe fn teardown() {
        // 😂😂😂😂😂   😂😂😂😂
        // 😂                 😂
        // 😂                😂
        // 😂😂😂😂😂       😂
        // 😂      😂       😂
        // 😂      😂      😂
        // 😂😂😂😂😂     😂
    }
}

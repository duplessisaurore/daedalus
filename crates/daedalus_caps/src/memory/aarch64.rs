//! The architecture specific memory operations
//! for `aarch64`.
//!
//! This supplies the necessary underlying
//! arch specific ops for memory caps.

use crate::memory::arch::{AccessWidth, MemoryArch};

/// Right shift of the CTR_EL0 register required
/// for reading `DMinLine` (min size of d-cache line)
const DMINLINE_SHIFT: u64 = 16;

/// Right shift of the CTR_EL0 register required
/// for reading `IMinLine` (min size of i-cache line)
const IMINLINE_SHIFT: u64 = 0;

/// Mask for the actual line size for the IMINLINE/DMINLINE shifts
const LINE_MASK: u64 = 0xf;

/// The `AArch64` memory operations
///
/// These are for `ARM 64-bit` platforms
/// such as the ZCU106.
pub struct Aarch64;

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
            let mut i = len;
            while i > 0 {
                i -= 1;
                unsafe {
                    destination
                        .add(i)
                        .write_volatile(source.add(i).read_volatile())
                };
            }

            return;
        }

        // Forward-case, see comment of `copy`
        let mut i = 0usize;
        while i < len {
            unsafe {
                destination
                    .add(i)
                    .write_volatile(source.add(i).read_volatile())
            };
            i += 1;
        }
    }

    unsafe fn fill(destination: *mut u8, value: u8, len: usize) {
        let mut i = 0usize;

        while i < len {
            unsafe { destination.add(i).write_volatile(value) };
            i += 1;
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
            let mut address = dstart;
            while address < dend {
                core::arch::asm!("dc cvac, {}", in(reg) address, options(nostack, preserves_flags));
                address += dmin_line;
            }

            // ensure all previous memory ops before this barrier are done
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));

            // clear each i-cache line
            address = istart;
            while address < iend {
                core::arch::asm!("ic ivau, {}", in(reg) address, options(nostack, preserves_flags));
                address += imin_line;
            }

            // ensure ops completed
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));

            // flush the processor pipeline so that we fetch new instructions
            core::arch::asm!("isb", options(nostack, preserves_flags));
        }
    }

    unsafe fn setup() {
        todo!()
    }

    unsafe fn teardown() {
        todo!()
    }
}

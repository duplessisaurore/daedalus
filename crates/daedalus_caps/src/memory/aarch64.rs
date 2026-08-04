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

/// Right shift of the CLIDR_EL1 register for reading
/// out the level of coherency (first coherent level)
const LOC_SHIFT: u64 = 24;

/// Mask for the level of coherency field in CLIDR_EL1
const LOC_MASK: u64 = 0x7;

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
    /// Clearning RAM is too large (2GB of vaddr to clear by on ZCU106), cache is a lot
    /// smaller but theres no magical clear all d-cache instruction.
    ///
    /// Instead we need to traverse all levels of cache up to the level of coherency and
    /// essentailly clean and invalidate every slot in the cache (rather than DRAM) as that
    /// would be way too many instructions for DRAM.
    ///
    /// Caches are indexed as sets x ways, and we can clean and invalidate each
    /// set x way index using `dc cisw` (clean invalidate set/way).
    ///
    /// the main issue is we need to traverse all the cache levels up, find all their shapes
    /// with sets/ways, plug that into `dc cisw`.
    ///
    /// The cache info is stored in `CLIDR_EL1` (what caches exist/level of coherency) and
    /// then we need to grab each individual caches info using `CSSELR_EL1` to select
    /// the cache and then `CCSIDR_EL1` for that cache. (isb to sync update)
    ///
    /// once we get all of the sets and ways for this, we can then just iterate over all
    /// pairs and clean/invalidate them with `dc cisw`
    ///
    /// Then we clear the instruction cache too, with `ic iallu`, `isb`.
    ///
    /// Next, we set `SCTLR_EL1` at `I` to 1, which enables the instruction cache. (after
    /// we reset the old cache) which is really nice for our interpreter :)
    unsafe fn setup() {
        // Read out CLIDR_EL1
        let cache_level_id: u64;

        // this is readable at EL1 which is where we should've been dropped of at.
        unsafe {
            core::arch::asm!(
                "mrs {}, clidr_el1",
                out(reg) cache_level_id,
                options(nomem, nostack, preserves_flags)
            );
        }

        // read out the level of coherency, we go through all non-coherent levels
        let level_of_coherency = ((cache_level_id >> LOC_SHIFT) & LOC_MASK) as u32;

        // we now need to clear each level
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

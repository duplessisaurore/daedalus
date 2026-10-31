//! The architecture specific memory operations
//! for `aarch64`.
//!
//! This supplies the necessary underlying
//! arch specific ops for memory caps.

use crate::memory::arch::{AccessWidth, MemoryArch};


#[path = "aarch64_mmu.rs"]
mod mmu;

/// right shift required for reading the level of coherency
/// out of the CLIDR_EL1 register
const CLIDR_LOC_SHIFT: u64 = 24;

/// mask for reading the LOC out of CLIDR
const CLIDR_LOC_MASK: u64 = 0x7;

/// right shift required for reading the FEAT_CCIDX
/// support section out of the MMFR2 register
const FEAT_CCIDX_SHIFT: u64 = 20;

/// mask for reading the FEAT_CCIDX value out of MMFR2
const FEAT_CCIDX_MASK: u64 = 0xF;

/// value when FEAT_CCIDX (aka CSSIDR_EL1 is in 64-bit format)
const FEAT_CCIDX_ENABLED: u64 = 0b0001;

/// number of bits per cache level in CLIDR_EL1
const BITS_PER_CTYPE: usize = 3;

// mask for a CTYPE in CLIDR_EL1
const CTYPE_MASK: u64 = 0x7;

// the shift for associativity (regardless of FEAT_CCIDX)
const ASSOCIATIVITY_SHIFT: u64 = 3;

// the mask for associativity with FEAT_CCIDX [23:3]
const CCIDX_ASSOCIATIVITY_MASK: u64 = 0x1f_ffff;

// the mask for associativity without FEAT_CCIDX [12:3]
const ASSOCIATIVITY_MASK: u64 = 0x3ff;

// the shift for NumSets with FEAT_CCIDX [55:32]
const CCIDX_NUM_SETS_SHIFT: u64 = 32;

// the shift for NumSets without FEAT_CCIDX [27:13]
const NUM_SETS_SHIFT: u64 = 13;

// the mask for NumSets with FEAT_CCIDX [55:32]
const CCIDX_NUM_SETS_MASK: u64 = 0xff_ffff;

// the mask for NumSets without FEAT_CCIDX [27:13]
const NUM_SETS_MASK: u64 = 0x7fff;

// the mask for the LineSize field in CCSIDR
const LINE_SIZE_MASK: u64 = 0x7;

/// Right shift of the CTR_EL0 register required
/// for reading `DMinLine` (min size of d-cache line)
const DMINLINE_SHIFT: u64 = 16;

/// Right shift of the CTR_EL0 register required
/// for reading `IMinLine` (min size of i-cache line)
const IMINLINE_SHIFT: u64 = 0;

/// Mask for the actual line size for the IMINLINE/DMINLINE shifts
const LINE_MASK: u64 = 0xf;

/// This is the `I` flag of the `SCTLR_ELx` registers, setting this
/// to 1 will enable the instruction cache.
const SCTLR_INSTRUCTION_CACHE: u64 = 1 << 12;

/// This is the E2H bit of the `HCR_EL2` register
///
/// This enables a configuration where a Host Operating System is running at EL2,
/// and the Host Operating System's applications are running at EL0. (VHE)
///
/// We don't want daedalus to be a VHE host.
const HCR_EL2_E2H: u64 = 1 << 34;

/// This is the TGE bit of the `HCR_EL2` register
///
/// This routes all exceptions from EL1 to EL2 if set to 1. (broad VHE exception trapping
/// behaviour), which we don't really want or care for.
const HCR_EL2_TGE: u64 = 1 << 27;

/// This controls physical IRQ routing, we want to set this to 1
/// when VHE is disabled so that physical IRQs are routed to EL2
/// for our EVT to route them to `Daedalus`.
const HCR_EL2_IMO: u64 = 1 << 4;

/// The mask on SCTLR_ELx If the MMU is enabled or
/// not at the current ELx level
const SCTLR_MMU: u64 = 1;

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
/// We also then set `HCR_EL2_IMO` such that physical IRQs get
/// routed to EL2 and we can handle them.
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

        // Route physical IRQs to EL2 so we get them for our IRQ handling.
        hcr |= HCR_EL2_IMO;

        // Write back to update.
        core::arch::asm!(
            "msr hcr_el2, {hcr}",
            "isb",
            hcr = in(reg) hcr,
            options(nomem, nostack, preserves_flags)
        );
    }
}

/// Returns the CurrentEL level in aarch64
fn read_current_el() -> u64 {
    let current_el: u64;

    // # Safety:
    //
    // `CurrentEL` is readable at every exception level.
    unsafe {
        core::arch::asm!("mrs {}, CurrentEL", out(reg) current_el, options(nomem, nostack));
    }

    // CurrentEL holds the exception level in bits [3:2].
    (current_el >> 2) & 0b11
}

/// This will run a special routine based on the current ELx level.
///
/// This will only permit the specifically allowed daedalus ELx levels.
fn assert_elx() {
    let current_el: u64 = read_current_el();

    match current_el {
        2 => {
            // Get rid of VHE,
            //
            // # Safety
            //
            // we've asserted we're in el2 and
            // this is at the top of memory setup which runs first.
            unsafe { configure_hcr_el2() };
        }

        // Nothing to do at EL1
        1 => {}

        _ => panic!("daedalus must run at EL2 or EL1, instead it was running at EL{current_el}!"),
    }
}

/// The mmu must not be enabled at the entry point to daedalus at
/// the EL level that daedalus is executing at
///
/// Else partially through building the translation map we may explode
/// because of old mappings (uh oh)
/// 
/// TODO: make daedalus support MMU enabled on entry, out of time... 
fn assert_mmu_off() {
    let sctlr: u64;

    unsafe {
        match read_current_el() {
            1 => core::arch::asm!("mrs {}, sctlr_el1", out(reg) sctlr, options(nomem, nostack)),
            2 => core::arch::asm!("mrs {}, sctlr_el2", out(reg) sctlr, options(nomem, nostack)),
            _ => unreachable!(),
        }
    }

    assert!(sctlr & SCTLR_MMU == 0, "daedalus must be ran with the MMU disabled on entry");
}

fn set_sctlr_elx_icache() {
    // Enable instruction caching with `SCTLR_ELx`
    let mut system_control: u64;

    // The specific register needs to be set based on current_el
    let current_el: u64 = read_current_el();

    match current_el {
        3 => unsafe {
            // Read the current value out, as we only want to enable
            // the instruction cache
            core::arch::asm!(
                "mrs {}, sctlr_el3",
                out(reg) system_control,
                options(nomem, nostack, preserves_flags)
            );
        },

        2 => unsafe {
            core::arch::asm!(
                "mrs {}, sctlr_el2",
                out(reg) system_control,
                options(nomem, nostack, preserves_flags)
            );
        },

        _ => unsafe {
            core::arch::asm!(
                "mrs {}, sctlr_el1",
                out(reg) system_control,
                options(nomem, nostack, preserves_flags)
            );
        },
    }

    // Set flag and write back
    system_control |= SCTLR_INSTRUCTION_CACHE;

    match current_el {
        3 => unsafe {
            core::arch::asm!(
                "msr sctlr_el3, {}",
                "isb",
                in(reg) system_control,
                options(nostack, preserves_flags)
            );
        },

        2 => unsafe {
            core::arch::asm!(
                "msr sctlr_el2, {}",
                "isb",
                in(reg) system_control,
                options(nostack, preserves_flags)
            );
        },

        _ => unsafe {
            core::arch::asm!(
                "msr sctlr_el1, {}",
                "isb",
                in(reg) system_control,
                options(nostack, preserves_flags)
            );
        },
    }
}

/// Clean and invalidate all the data caches at all layers down
/// to the LOC through sets / ways of each cache level.
/// 
/// This will also invalidate all instruction caches 
unsafe fn clean_invalidate_all_caches() {
    // cache level id, for identifying the LOC
    let clidr: u64;

    // we also need to check if FEAT_CCIDX is enabled
    // which means that cssidr_el1 contains a different format
    // for num sets https://arm.jonpalmisc.com/latest_sysreg/AArch64-ccsidr_el1
    let mmfr2: u64;

    // store the old csselr to reset back after our setup (just incase)
    let old_csselr: u64;

    // read out our registers
    unsafe {
        // initial dsb sy to sync everything because we are about to explode the cache
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));

        core::arch::asm!("mrs {}, clidr_el1", out(reg) clidr, options(nomem, nostack, preserves_flags));
        core::arch::asm!("mrs {}, id_aa64mmfr2_el1", out(reg) mmfr2, options(nomem, nostack, preserves_flags));
        core::arch::asm!("mrs {}, csselr_el1", out(reg) old_csselr, options(nomem, nostack, preserves_flags));
    }

    // level of coherency, we need to clear down to this
    let loc = ((clidr >> CLIDR_LOC_SHIFT) & CLIDR_LOC_MASK) as usize;

    // check if CCIDX is enabled
    // this is when id_aa64mmfr2_el1 bits [23:20] == 0b0001
    // https://fooptrvoid.github.io/arm-mra-2024-12-sysreg/AArch64-id_aa64mmfr2_el1.html#fieldset_0-23_20
    let ccidx = ((mmfr2 >> FEAT_CCIDX_SHIFT) & FEAT_CCIDX_MASK) == FEAT_CCIDX_ENABLED;

    // we need to clear all levels down to level of coherency
    for level in 0..loc {
        // from bit 0 in clidr, we have 3 bits per cache lvel
        let cache_type = (clidr >> (level * BITS_PER_CTYPE)) & CTYPE_MASK;

        // Ctype<n>	Meaning
        // 0b000	No cache.
        // 0b001	Instruction cache only.
        // 0b010	Data cache only.
        // 0b011	Separate instruction and data caches.
        // 0b100	Unified cache.
        if !(0b010..=0b100).contains(&cache_type) {
            continue;
        }

        // select cache at this level (gets put in csselr_el1 at [3:1])
        let selection = (level as u64) << 1;

        // read out cssidr, this will tell us the number of
        // sets in this specific level
        let ccsidr: u64;

        unsafe {
            // select level, we isb so further
            // instructions referes to this new cache level
            core::arch::asm!(
                "msr csselr_el1, {}",
                "isb",
                in(reg) selection,
                options(nostack, preserves_flags)
            );

            core::arch::asm!("mrs {}, ccsidr_el1", out(reg) ccsidr, options(nomem, nostack, preserves_flags));
        }

        // the associativity/numsets at this level
        // these are the values - 1
        let (associativity, num_sets) = if ccidx {
            (
                ((ccsidr >> ASSOCIATIVITY_SHIFT) & CCIDX_ASSOCIATIVITY_MASK) as u32,
                ((ccsidr >> CCIDX_NUM_SETS_SHIFT) & CCIDX_NUM_SETS_MASK) as u32,
            )
        } else {
            (
                ((ccsidr >> ASSOCIATIVITY_SHIFT) & ASSOCIATIVITY_MASK) as u32,
                ((ccsidr >> NUM_SETS_SHIFT) & NUM_SETS_MASK) as u32,
            )
        };

        // now we have to do the utter insanity that
        // is the dc cisw arguments:

        // SetWay, bits [31:4]

        // Contains two fields:

        //     Way, bits[31:32-A], the number of the way to operate on.
        //     Set, bits[B-1:L], the number of the set to operate on.

        // Bits[L-1:4] are RES0.

        // A = Log2(ASSOCIATIVITY), L = Log2(LINELEN), B = (L + S), S = Log2(NSETS).

        // ASSOCIATIVITY, LINELEN (line length, in bytes), and NSETS (number of sets) have their usual meanings and are the values for the cache level being operated on. The values of A and S are rounded up to the next integer.
        // Level, bits [3:1]

        // Cache level to operate on, minus 1. For example, this field is 0 for operations on L1 cache, or 1 for operations on L2 cache.
        // Bit [0]

        // Reserved, RES0.

        // we shift the set L left, which is log2(linelen),
        // line size in CSSIDR is log2(line bytes) - 4
        let set_shift = ((ccsidr & LINE_SIZE_MASK) as u32) + 4;

        // A = ceil(log2(number of ways))
        // amount to shift way = 32 - A
        // which is essentially leading number of bits in associativity
        // because 32 - (bits in number of ways)
        let way_shift = associativity.leading_zeros();

        // clear all sets and ways
        for set in (0..=num_sets).rev() {
            for way in (0..=associativity).rev() {
                // Way, bits[31:32-A]
                let way_bits = (way as u64) << way_shift;

                // Set, bits[B-1:L]
                let set_bits = (set as u64) << set_shift;

                // Level, bits [3:1], same as for putting in
                // CSSELR_EL1 so nothing to do there
                let set_ways = way_bits | set_bits | selection;

                unsafe {
                    core::arch::asm!("dc cisw, {}", in(reg) set_ways, options(nostack, preserves_flags));
                }
            }
        }

        // make sure all previous mem instructions are complete before
        // next cache level
        unsafe {
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }

    unsafe {
        // restore original csselr_el1
        core::arch::asm!(
            "msr csselr_el1, {}",
            "isb",
            in(reg) old_csselr,
            options(nostack, preserves_flags)
        );

        // ensure everything is complete
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));

        // invalidate instruction cache too everywhere
        core::arch::asm!("ic ialluis", options(nostack, preserves_flags));

        // final syncs
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::arch::asm!("isb", options(nostack, preserves_flags));
    }

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

    unsafe fn read_n<'a, I>(pointer: *const u8, width: AccessWidth, n: usize, buffer: I)
    where
        I: Iterator<Item = &'a mut u64>,
    {
        let step = width.bytes();

        for (offset, value) in buffer.take(n).enumerate() {
            // # Safety
            //
            // The invariants of this function are a superset of read,
            // and guarantee the read are valid (or at least forward req to caller)
            //
            // buffer writing won't panic because we require buffer to at least have space for n elements
            unsafe { *value = Self::read(pointer.wrapping_add(offset * step), width) }
        }
    }

    unsafe fn write_n<'a, I>(pointer: *mut u8, width: AccessWidth, n: usize, buffer: I)
    where
        I: Iterator<Item = &'a u64> {
        let step = width.bytes();

        // # Safety
        //
        // The invariants of this function are a superset of write,
        // and guarantee the write are valid (or at least forward req to caller)
        //
        // value[offset] won't panic because we require value to at least have n elements
        for (offset, value) in buffer.take(n).enumerate() {
            unsafe {
                Self::write(pointer.wrapping_add(offset * step), width, *value);
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

    unsafe fn setup() {
        // Assert that we are actually running at a valid ELx on aarch64
        // and that the MMU is off so when we do our own setup things dont explode.
        assert_elx();

        
        assert_mmu_off();

        // Previous cache from prior boot stages
        // may invalidate daedalus's memory on flush or something,
        // so we first invalidate/clean all of those
        unsafe { clean_invalidate_all_caches(); }

        // Set icache to be active for faster performance.
        set_sctlr_elx_icache();
    }

    unsafe fn post_bss_setup() {
        // Enable the MMU
        //
        // this is required else performance is a big :thumbs_down: 
        // (probably 30x slower.)
        
        unsafe { mmu::enable(); }
    }

    unsafe fn teardown() {
        unsafe {
            // stop any interupts from occuring so they dont interrupt our teardown process.
            core::arch::asm!("msr daifset, #0xf", options(nostack, preserves_flags));

            // clean and invalidate all data caches to flush everytihng and make sure its
            // all visible before we disable the mmu
            clean_invalidate_all_caches();

            // disable MMU as handoff policy to next stage
            
            mmu::disable();

            // flush instruction cache one last time to make sure everything is visible
            // and nothing stale remains
            core::arch::asm!(
                "ic ialluis",
                "dsb sy",
                "isb",
                options(nostack, preserves_flags),
            );
        }
    }
}

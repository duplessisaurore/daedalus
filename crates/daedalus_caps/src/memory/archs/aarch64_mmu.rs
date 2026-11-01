//! AARCH64 Stage 1 MMU setup
//! 
//! we essentially just make identity mappings
//! using `aarch64-paging` over all the grants
//! plus daedalus's memory area and all the sepcial
//! areas like GIC/anything given by plats.
//! 
//! Generally all memory should be cacheable other than
//! explicit device areas through grants/above.
//! 
//! This isn't technically rqeuired but performance sucks
//! without it ig fair enough since no d-cache can exist
//! with Device-nGnRnE memory, which is what all memory is when
//! MMU is disabled.`

use alloc::{boxed::Box, vec::Vec};
use aarch64_paging::{
    descriptor::{El1Attributes, El23Attributes},
    idmap::IdMap,
    paging::{El1And0, El2, MemoryRegion},
};
use daedalus_program::{ALL_PROGRAMS, RegionMemKind};

use crate::{
    irq::plats::{
        GIC_CPU_INTERFACE_BASE, GIC_CPU_INTERFACE_SIZE, GIC_DISTRIBUTOR_BASE,
        GIC_DISTRIBUTOR_SIZE,
    }, memory::{
        GrantBounds, plats::{get_extra_device_spans_for_platform}, resolve_grant_bounds,
    },
};

use super::read_current_el;

unsafe extern "C" {
    static __daedalus_start: u8;
    static __daedalus_end: u8;
    static __dram_start: u8;
    static __dram_end: u8;
}

// page size for our pages in our translation scheme
const PAGE_SIZE: usize = 4096;

// enable MMU
const SCTLR_M: u64 = 1 << 0;

// enable caching
const SCTLR_C: u64 = 1 << 2;

// enable instruction caching
const SCTLR_I: u64 = 1 << 12;

// Write execute never, which may be leftover from a prior
// boot phase or something MMU, we need to disable this because
// daedalus memory is writeable to but we also want to execute it!
const SCTLR_WXN: u64 = 1 << 19;

// unironically this register is incomprehensible
// but whatever see: https://dflund.se/~getz/ARM/SysReg/AArch64-mair_el1.html
//
// Each attribute gets 8 bits from 0-64 bits containing 8 attributes.
// [0;7] - attribute #0 (we use this for RegionMemKind::Device)
// [8;15] - attrbitue #1 (we use this for RegionMemKind::Memory)
//
// for device memory we map it in as Device-nGnRnE 0b00000000
// for attribute 1:
//      oooo and iiii are non-zero so it's normal memory
//          for oooo: 0b11RW	Normal memory, Outer Write-Back Non-transient
//          for iiii: 0b11RW	Normal memory, Inner Write-Back Non-transient
//              where RW: Allocate for Inner read/write
const MAIR: u64 = 0x00 | (0xff << 8);

/// T0SZ
/// The size offset of the memory region addressed by TTBR0_ELx. The region size is 2(64-T0SZ) bytes.
/// 
/// Basically how much memory does our page table cover, which in this case because we use a 4-level
/// page table we have 48 bits addressible (which is the max PA range allowed, as below seen)
const T0SZ: u64 = 16;

/// Root level of the page table,
/// basically if level 1 then only 3 level etc. etc. at level 0 we can use fully a 4-level page table
/// which covers all possible PA range,
/// 
/// Level 0 requires T0SZ to be 16.
const ROOT_LEVEL: usize = 0;

/// IRGN0 [9;8]
/// Inner Cacheability attribute for memory associated with translation table walks
/// 
/// 0b01 = Normal memory, Inner Write-Back Read-Allocate Write-Allocate Cacheable.
const IRGN0: u64 = 0b01 << 8;

/// ORGN0 [11:10]
/// Outer Cacheability attribute for memory associated with translation table walks
/// 
/// 0b01 = Normal memory, Inner Write-Back Read-Allocate Write-Allocate Cacheable.
const ORGN0: u64 = 0b01 << 8;

/// SH0 [13:12]
/// Shareability attribute for memory associated with translation table walks 
/// 
/// 0b11 = Inner Shareable.
const SH0: u64 = 0b11 << 12;

/// All the cacheability and shareability attributes combined
const CACHEABILITY_AND_SHAREABILITY: u64 = IRGN0 | ORGN0 | SH0;

/// EPD1
/// 
/// A TLB miss on an address that is translated using TTBR1_EL1 generates a Translation fault. 
/// No translation table walk is performed (we dont have no TTBR1_EL1)
const EPD1: u64 = 0b1 << 23;

/// One span of memory with a resolved start and
/// end (exclusive)
/// 
/// The `kind` dictates whether we map this as normal
/// memory that's cacheable or Device-nGnRnE (for MMIO)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Span {
    base: usize,
    end: usize,
    kind: RegionMemKind,
}

/// Span mappings for device/normal memory types
/// which we will set translation up for in our MMU
/// 
/// DEVICE MAPPINGS MUST BE MAPPED AFTER NORMAL
/// SUCH THAT THEY OVERWRITE NORMAL MAPPINGS !!!
struct DaedalusSpans {
    device: Vec<Span>,
    normal: Vec<Span>
}

impl Span {
    /// Creates a new span with some `base`, `len` and of
    /// the kind provided.
    /// 
    /// # Panics
    /// 
    /// The end should be within the range of the `usize` of
    /// the system, otherwise this function will panic!
    fn new(base: usize, len: usize, kind: RegionMemKind) -> Self {
        Self {
            base,
            end: base.checked_add(len).expect("Span provided to MMU does not overflow end with given start and length"),
            kind,
        }
    }

    /// Returns `true` if this span overlaps with some `other` span,
    /// otherwise `false``.
    fn overlaps(self, other: Self) -> bool {
        self.base < other.end && other.base < self.end
    }

    /// Returns a version of this span wihch is page aligned to
    /// the nearest `PAGE_SIZE` page.
    fn page_aligned(self) -> Self {
        // we drop any offset into the page
        let base = self.base & !(PAGE_SIZE - 1);

        // end should be the nearest page after end rather than
        // before the page
        let offset_into_next_page = self.end.checked_add(PAGE_SIZE - 1)
            .expect("Span provided to MMU does not overflow end with given start and length");
        
        // drop offset too
        let end = offset_into_next_page & !(PAGE_SIZE - 1);
        Self { base, end, ..self }
    }
}

/// Coalesce spans together to a (potentially) smaller
/// set of spans which merges together overlapping spans.
/// 
/// for ex.
/// 
///             | ----- |
///                 |--------|
/// 
/// are two spans of the same kind, will be merged into:
/// 
///             |------------|
/// 
fn coalesce(mut spans: Vec<Span>) -> Vec<Span> {
    // sort, so then any span after can check the prior span only
    // instead of all spans
    spans.sort_unstable_by_key(|range| range.base);
    
    // output merged set
    let mut merged: Vec<Span> = Vec::new();

    for span in spans {
        // no point putting this region/span in as it occupies 0 memory
        if span.base == span.end {
            continue;
        }

        // if there was a previous span and the end of the previous span
        // (since it's ordered) overlaps with the base of the current span
        //
        // etc.
        //       previous.end
        //           v
        //  |--------|
        //        |--------|
        //        ^
        //    span.base
        //
        // then we can merge them together into one span by
        // changing previous.end to span.end (if the kind matches)
        if let Some(previous) = merged.last_mut() {
            if previous.kind == span.kind && span.base <= previous.end {
                previous.end = previous.end.max(span.end);
                continue;
            }
        }

        // no merging, push.
        merged.push(span);
    }
    merged
}

struct GrantSpanMapping {
    program_name: &'static str,
    role: &'static str,
    span: Span
}

/// Collect all spans for device & normal/memory type spans throughout
/// all of daedalus's programs.
/// 
/// This will validate all spans overlap (now known)
fn collect_spans() -> DaedalusSpans {
    let mut grants = Vec::new();

    // Validate each grant from each program does not overlap other grants
    for program in ALL_PROGRAMS {

        // for all grants of the current program
        for grant in program.grants {

            // resolve to real underlying grant with keywords in mind
            let GrantBounds { base, len } = resolve_grant_bounds(grant)
                .unwrap_or_else(|err| panic!("program `{}` grant `{}`: {err}", program.name, grant.role));

            // this grant doesn't matter because it has no length.
            if len == 0 {
                continue;
            }

            let span = Span::new(base, len, grant.kind);
            
            // validate against all other programs
            for GrantSpanMapping { program_name: other_program, role: other_role, span: other_span } in &grants {
                
                // it must not overlap with a different type 
                assert!(
                    grant.kind == other_span.kind || !span.overlaps(*other_span),
                    "incompatible grants found during MMU seutp! program `{}` grant `{}` ({:?}) and program `{}` grant `{}` ({:?}) differ in kind but overlap!",
                    program.name, grant.role, span.kind,
                    other_program, other_role, other_span.kind,
                );
            }
            grants.push(GrantSpanMapping { program_name: program.name, role: grant.role, span, });
        }
    }

    // known dram start <-> end is not literal all ram but a safe size given by linker
    let dram_start = (&raw const __dram_start) as usize;
    let dram_end = (&raw const __dram_end) as usize;

    // mapped in as memory
    let dram = Span::new(
        dram_start,
        dram_end.checked_sub(dram_start).expect("valid __dram ranges provided by linker .ld for current platform"),
        RegionMemKind::Memory,
    );

    // same for daedalus (which may be outside of __dram for some reason)..
    // but as no program overlaps we know this is safe 
    let daedalus_start = (&raw const __daedalus_start) as usize;
    let daedalus_end = (&raw const __daedalus_end) as usize;

    let daedalus = Span::new(
        daedalus_start,
        daedalus_end.checked_sub(daedalus_start).expect("valid __daedalus ranges provided by linker .ld for current platform"),
        RegionMemKind::Memory,
    );

    // build span mappings now for each type of grant aligned to pages
    let mut normal = alloc::vec![dram.page_aligned(), daedalus.page_aligned()];
    let mut device = Vec::new();
    for GrantSpanMapping { program_name: _, role: _, span } in grants {
        let span = span.page_aligned();
        match span.kind {
            RegionMemKind::Memory => normal.push(span),
            RegionMemKind::Device => device.push(span),
        }
    }

    // GIC must be device mapped, since daedalus uses it, even regardless
    // of program grants
    device.push(
        Span::new(GIC_DISTRIBUTOR_BASE, GIC_DISTRIBUTOR_SIZE, RegionMemKind::Device)
            .page_aligned(),
    );
    device.push(
        Span::new(GIC_CPU_INTERFACE_BASE, GIC_CPU_INTERFACE_SIZE, RegionMemKind::Device)
            .page_aligned(),
    );

    // All extra platform device mappings
    for extra_span in get_extra_device_spans_for_platform() {
        device.push(
            Span::new(extra_span.base, extra_span.len, RegionMemKind::Device)
                .page_aligned()
        )
    }

    // Coalesce both types of spans, grouping shared things together (liekly with programs)
    DaedalusSpans { 
        normal: coalesce(normal), 
        device: coalesce(device) 
    }
}

// ensure span is within the range of the actual page-table, and map
// to aarch64_paging::MemoryRegion
fn check_range(span: Span, table_size: usize) -> MemoryRegion {
    assert!(span.end <= table_size, "MMU provided grant-region {span:?} exceeds underlying aarch64 page-table VA range! kaboom!");
    MemoryRegion::new(span.base, span.end)
}

/// Builds the identity mapping for EL1 page table
fn build_el1(normal: &[Span], device: &[Span]) -> IdMap<El1And0> {
    let mut map = IdMap::with_asid(0, ROOT_LEVEL, El1And0);

    // map in all of the normal spans
    for span in normal {
        let flags = El1Attributes::VALID
            | El1Attributes::ACCESSED
            | El1Attributes::INNER_SHAREABLE
            | El1Attributes::ATTRIBUTE_INDEX_1
            | El1Attributes::UXN;
            
        map.map_range(&check_range(*span, map.size()), flags)
            .expect("map normal memory at EL1 succeeds");
    }

    // map in all of the device spans
    for span in device {
        let flags = El1Attributes::VALID
            | El1Attributes::ACCESSED
            | El1Attributes::PXN
            | El1Attributes::ATTRIBUTE_INDEX_0
            | El1Attributes::UXN;
        
        map.map_range(&check_range(*span, map.size()), flags)
            .expect("map device memory at EL1 succeeds");
    }

    map
}

/// Builds the identity mapping for EL2 page table
fn build_el2(normal: &[Span], device: &[Span]) -> IdMap<El2> {
    let mut map = IdMap::new(ROOT_LEVEL, El2);

    // map in all of the normal spans
    for span in normal {
        let flags = El23Attributes::VALID
            | El23Attributes::ACCESSED
            | El23Attributes::INNER_SHAREABLE
            | El23Attributes::ATTRIBUTE_INDEX_1;

        map.map_range(&check_range(*span, map.size()), flags)
            .expect("map normal memory at El2 succeeds");
    }

    // map in all of the device spans
    for span in device {
        let flags = El23Attributes::VALID 
            | El23Attributes::ACCESSED 
            | El23Attributes::ATTRIBUTE_INDEX_0
            | El23Attributes::XN;

        map.map_range(&check_range(*span, map.size()), flags)
            .expect("map device memory at El2 succeeds");
    }

    map
}

/// Read out the Physical Address range supported by this
/// processor/configuration
/// 
/// see: https://arm.jonpalmisc.com/latest_sysreg/AArch64-id_aa64mmfr0_el1#fieldset_0-3_0
fn physical_address_bits_encoding() -> u64 {
    let mmfr0: u64;

    // # Safety
    //
    // id_aa64mmfr0_el1 is readable at EL1 and EL2
    unsafe {
        core::arch::asm!(
            "mrs {}, id_aa64mmfr0_el1",
            out(reg) mmfr0,
            options(nomem, nostack)
        );
    }

    let parange = mmfr0 & 0xf;

    // we only support <=48-bit physical addressing, which doesn't require extra FEAT_LPA or FEAT_D128 support
    // supporting 52bit/56bit is extra work im not really seeing the point of doing (and aarch64-paging doesn't support it!)
    match parange {
        0..=5 => parange,
        6 | 7 => 5,
        _ => panic!("unsupported AArch64 PARange encoding found in id_aa64mmfr0_el1"),
    }
}

/// Configure the MAIR register for our page-table attributes
/// 
/// # Safety
/// 
/// Must be before the MMU is enabled/same conditions as `mmu::enable`
unsafe fn configure_mair(el: u64) {
    unsafe {
        match el {
            1 => {
                core::arch::asm!(
                    "msr mair_el1, {}",
                    "isb",
                    in(reg) MAIR,
                    options(nostack, preserves_flags)
                );
            }
            2 => {
                core::arch::asm!(
                    "msr mair_el2, {}",
                    "isb",
                    in(reg) MAIR,
                    options(nostack, preserves_flags)
                );
            }
            _ => unreachable!(),
        }
    }
}

/// Configures the translation control register for our
/// MMU translation setup & page-table setup 
/// 
/// # Safety
/// 
/// Must be before the MMU is enabled/same conditions as `mmu::enable`
unsafe fn configure_tcr(el: u64) {
    // the largest physical address sizing supported by this system
    // basically just take as big as we can get
    let pa_size = physical_address_bits_encoding();

    unsafe {
        match el {
            1 => {
                // T0SZ bits [5;0]
                let tcr = T0SZ
                    | CACHEABILITY_AND_SHAREABILITY
                    | EPD1

                    // physical address size used in translation
                    | (pa_size << 32);

                core::arch::asm!(
                    "msr tcr_el1, {}",
                    "isb",
                    in(reg) tcr,
                    options(nostack, preserves_flags)
                );
            }
            2 => {
                // T0SZ bits [5;0]
                let tcr = T0SZ 
                    | CACHEABILITY_AND_SHAREABILITY 

                    // physical address size used in translation
                    | (pa_size << 16);

                core::arch::asm!(
                    "msr tcr_el2, {}",
                    "isb",
                    in(reg) tcr,
                    options(nostack, preserves_flags)
                );
            }
            _ => unreachable!(),
        }
    }
}

/// Enable the MMU and all of it's mappings for `daedalus` for performance
///
/// # Safety
///
/// The caller must ensure that the MMU must is disabled. 
/// 
/// The caller must ensure that `Daedalus` only ever runs on one core at
/// once.
/// 
/// The caller must ensure that the heap allocator is setup for `Daedalus`
/// at this point.
/// 
/// The caller must ensure this only runs once in the lifecycle of `Daedalus`.
pub unsafe fn enable() {
    let el = read_current_el();
    let DaedalusSpans { device, normal } = collect_spans();

    // Configure MAIR for attributes & 
    // # Safety
    //
    // This is before the MMU is enabled
    unsafe {
        configure_mair(el);
        configure_tcr(el);
    }

    // Enable/activate the translation scheme we are using for this ELx,
    // 
    // while daedalus is alive this should never die, so we dont really
    // care if this memory is leaked.
    // 
    // # Safety
    // 
    // We ensure that daedalus's mappings are in the page table, therefore ensuring
    // the page table contains mappings for this program (req 1)
    // 
    // we don't make any aliases (direct identity mappings) and we leak the
    // memory for the page table (so it survives as long as daedalus does until handoff) so
    // the page table lives as long as our requirements.
    unsafe { 
        match el {
        1 => {
            Box::leak(Box::new(build_el1(&normal, &device)))
                .activate();
        },
        2 => {
            Box::leak(Box::new(build_el2(&normal, &device)))
                .activate();
        },
        _ => unreachable!()
    }}

    // Invalidate all the old tlb entires now that we have our new page
    // table installed, before activating MMU
    unsafe {
        match el {
            1 => {
                core::arch::asm!(
                    "dsb sy", 
                    "tlbi vmalle1", 
                    "dsb sy", 
                    "isb",
                    options(nostack, preserves_flags)
                );
            },
            2 => {
                core::arch::asm!(
                    "dsb sy", 
                    "tlbi alle2", 
                    "dsb sy", 
                    "isb",
                    options(nostack, preserves_flags)
                );
            },
            _ => unreachable!()
        }
    }

    // Get the current value of sctlr out
    let mut sctlr: u64;

    unsafe {
        match el {
            1 => {
                core::arch::asm!(
                    "mrs {}, sctlr_el1",
                    out(reg) sctlr,
                    options(nomem, nostack)
                );
            },
            2 => {
                core::arch::asm!(
                    "mrs {}, sctlr_el2",
                    out(reg) sctlr,
                    options(nomem, nostack)
                );
            },
            _ => unreachable!()
        }
    }

    // enable mmu, caching, i-cache, and disable writeable never executable
    sctlr = (sctlr | SCTLR_M | SCTLR_C | SCTLR_I) & !SCTLR_WXN;

    // put back
    unsafe {
        match el {
            1 => {
                core::arch::asm!(
                    "msr sctlr_el1, {}",
                    "isb",
                    in(reg) sctlr,
                    options(nostack, preserves_flags)
                );
            }
            2 => {
                core::arch::asm!(
                    "msr sctlr_el2, {}", 
                    "isb",
                    in(reg) sctlr,
                    options(nostack, preserves_flags)
                );
            }
            _ => unreachable!(),
        }
    }
}

/// Turn off and teardown the MMU setup, disabling it.
/// 
/// # Safety
///
/// The caller must ensure that `Daedalus` only ever runs on one core at
/// once.
/// 
/// The caller must ensure this only runs once in the lifecycle of `Daedalus`.
/// 
/// The caller must already have cleaned/invalidated the data caches.
pub unsafe fn disable() {
    unsafe {
        // just for safety, dsb sy so anything prior is done before we disable
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        
        // read current SCTLR 
        let mut sctlr: u64;
        match read_current_el() {
            1 => {
                core::arch::asm!("mrs {}, sctlr_el1", out(reg) sctlr, options(nomem, nostack));
            }
            2 => {
                core::arch::asm!("mrs {}, sctlr_el2", out(reg) sctlr, options(nomem, nostack));
            }
            _ => unreachable!(),
        }

        // disable MMU and d-caches
        sctlr &= !(SCTLR_M | SCTLR_C);

        match read_current_el() {
            1 => {
                // write back sctlr
                core::arch::asm!(
                    "msr sctlr_el1, {}",
                    "isb",
                    in(reg) sctlr, 
                    options(nomem, nostack));
                
                // invalidate cached translation table entries
                core::arch::asm!(
                    "tlbi vmalle1", 
                    "dsb sy", 
                    "isb",
                    options(nostack, preserves_flags)
                );
            }
            2 => {
                // write back sctlr
                core::arch::asm!(
                    "msr sctlr_el2, {}",
                    "isb",
                    in(reg) sctlr, 
                    options(nomem, nostack));
                
                // invalidate cached translation table entries
                core::arch::asm!(
                    "tlbi alle2", 
                    "dsb sy", 
                    "isb",
                    options(nostack, preserves_flags)
                );
            }
            _ => unreachable!()
        }
    }
}
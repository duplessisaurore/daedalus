//! The entry point is defined here for all platforms.
//!
//! Each platform/arch startup script should lead to `rust_entry`.

use daedalus_caps::{
    irq::{arch::IrqArch, archs::TargetIRQArch},
    memory::{arch::MemoryArch, archs::TargetMemoryArch},
};

use crate::{heap, run::run};

#[cfg(feature = "extra-debug")]
use crate::extra_debug;

unsafe extern "C" {
    // This should be the extent of the .bss section
    static __bss_start: u8;
    static __bss_end: u8;

    // This should be the location of the stack guard.
    static __stack_guard_start: u8;
    static __stack_guard_end: u8;
}

/// This is the pattern we fill the `stack_guard` with (not-zero)
/// because its easy to recognise, and it serves as the barrier between stack & heap.
const STACK_GUARD_PATTERN: u8 = 0x67;

/// Write the guard pattern to the gap between the heap and the stack.
///
/// This serves as an easily identifable pattern we can recognise if either
/// the heap or the stack access out of bounds.
///
/// # Safety
///
/// Must run before the heap allocator is initialised.
///
/// The linker must properly have initialised a __stack_guard_start/end
unsafe fn initialise_stack_guard() {
    let start = (&raw const __stack_guard_start) as usize;
    let end = (&raw const __stack_guard_end) as usize;

    // # Safety
    //
    // As `Daedalus`, we automatically have all permissions in our own memory space,
    // and this has been reserved by the linker as an explicit guard.
    unsafe {
        TargetMemoryArch::fill(start as *mut u8, STACK_GUARD_PATTERN, end - start);
    }
}

/// Zero initialise the entire .bss section, this section contains
/// statically allocated values that have not yet been initialised.
///
/// This is a pretty good thing to do else funky monkey behaviour can
/// occur due to garbage data.
///
/// # Safety
///
/// Must run before any `Daedalus` code reads a static in the .bss section.
unsafe fn initialise_zero_bss() {
    let start = (&raw const __bss_start) as usize;
    let end = (&raw const __bss_end) as usize;

    // # Safety
    //
    // As `Daedalus`, we automatically have all permissions in our own memory space,
    // including the .bss section which we actively use.
    unsafe {
        TargetMemoryArch::fill(start as *mut u8, 0, end - start);
    }
}

/// This is the main entry point to `Daedalus` in rust.
///
/// This should not exit itself, but when daedalus is complete
/// the bootloader should jump to the start point of seL4 for
/// the kernel to start and LionsOS with microkit.
#[unsafe(no_mangle)]
pub extern "C" fn rust_entry(_previous_stage_x0: u64) -> ! {
    // # Safety
    //
    // All of these operations are ran only once, on the entry to `Daedalus`.
    //
    // We assume that `Daedalus` is running in a single-core environment, and
    // the same preconditions for all specific-arch stuff are met.
    unsafe {
        // This is literally the first op, so no memory has been written to yet.
        TargetMemoryArch::setup();

        // Initialise sections (before we initialise the heap allocator)
        initialise_zero_bss();
        initialise_stack_guard();

        // Set up IRQs
        TargetIRQArch::setup();
    }

    // Initialise the heap, so we can use allocated structures
    // (lepton3 uses these heavily.)
    heap::initialise_heap();

    #[cfg(feature = "extra-debug")]
    extra_debug::debug_start();

    run();
}

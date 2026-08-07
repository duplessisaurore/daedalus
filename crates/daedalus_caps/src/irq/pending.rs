//! The pending tracker for IRQ's
//!
//! This tracks all the pending IRQ's between the IRQ context
//! called from the EVT and the normal execution context
//! to actually handle the IRQ (as we don't want to mess up
//! the VM mid-op).

use daedalus_program::TOTAL_POSSIBLE_SIMULTANEOUS_INTERRUPTS;

/// These are every single unique possible interrupt combination.
///
/// It is not possible to exceed
static mut PENDING_INTERRUPTS: [u32; TOTAL_POSSIBLE_SIMULTANEOUS_INTERRUPTS] =
    [0; TOTAL_POSSIBLE_SIMULTANEOUS_INTERRUPTS];

/// The total number of currently pending interrupts in
/// `PENDING_INTERRUPTS` that are actually pending.
///
/// This must always reflect the correct count.
static mut PENDING_COUNT: usize = 0;

/// Records that an interrupt with an `interrupt_id` fired into
/// the set of `PENDING_INTERRUPTS`
///
/// This then will increase `PENDING_COUNT` to reflect these changes.
///
/// # Safety
///
/// This function should only ever be called in a one-core execution
/// environment of `Daedalus` and in the "interrupt-context" (in
/// the interrupt handler for `Daedalus`.)
///
/// This function does not deduplicate entries. Please ensure soundness
/// of not overflowing `PENDING_INTERRUPTS` by ensuring that interupts
/// which are recorded as pending can never fire again until cleared.
///
/// This can be done by masking the interrupt before placing it in
/// `PENDING_INTERRUPTS`.
pub unsafe fn record_pending_interrupt(interrupt_id: u32) {
    unsafe {
        // Read the current number of items in `PENDING_INTERRUPTS`
        let count = (&raw const PENDING_COUNT).read_volatile();
        debug_assert!(count <= TOTAL_POSSIBLE_SIMULTANEOUS_INTERRUPTS);

        // Add to `PENDING_INTERRUPTS`, this is safe because we are only
        // one core, with the assumption of the dedup above
        (&raw mut PENDING_INTERRUPTS)
            .cast::<u32>()
            .add(count)
            .write_volatile(interrupt_id);

        // Add to pending count
        (&raw mut PENDING_COUNT).write_volatile(count + 1);
    }
}

/// Returns whether or not there are any pending interrupts to be
/// read from
///
/// # Safety
///
/// This function should only ever be called in a one-core execution
/// environment of `Daedalus`.
#[must_use]
pub unsafe fn pending_any() -> bool {
    // SAFETY:
    // This is just a simple volatile read of pending count, which should
    // be fine because we are only in one core env
    unsafe { (&raw const PENDING_COUNT).read_volatile() != 0 }
}

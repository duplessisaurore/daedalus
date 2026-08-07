//! The pending tracker for IRQ's
//!
//! This tracks all the pending IRQ's between the IRQ context
//! called from the EVT and the normal execution context
//! to actually handle the IRQ (as we don't want to mess up
//! the VM mid-op).

use daedalus_program::TOTAL_POSSIBLE_SIMULTANEOUS_INTERRUPTS;

use crate::irq::{arch::IrqArch, archs::TargetIRQArch};

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
        debug_assert!(count < TOTAL_POSSIBLE_SIMULTANEOUS_INTERRUPTS);

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

/// This drains the current elements
/// of `PENDING_INTERRUPTS`, clearing the `PENDING_INTERRUPTS`
/// and resetting `PENDING_COUNT` to 0.
///
/// This drains into the `buf` provided, and returns
/// the number of entries drained.
///
/// This drain inherently copies, but should be cheap enough,
/// the main reason we don't run directly on `PENDING` etc.
/// is because we need to disable interrupts while draining,
/// else mid-drain we can explode because of a race condition
/// due to an interrupt handling mid-drain.
///
/// This inherently snapshots the state.
///
/// # Safety
///
/// This should only run on a non-interrupt "normal-context".
///
/// The main reason is that whoever runs this should actually
/// be able to route the interrupts used back to the programs,
/// which is only possible in the normal context!
pub fn drain_pending_into_buf(buf: &mut [u32; TOTAL_POSSIBLE_SIMULTANEOUS_INTERRUPTS]) -> usize {
    // SAFETY: stored state is restored below
    let interrupt_state = unsafe { TargetIRQArch::disable_interrupts() };

    // SAFETY: interrupts masked, so we
    // cant be interrupted mid op and get inconsistent state between
    // `PENDING_COUNT` and `PENDING_INTERRUPTS` :)
    let count = unsafe {
        let count = (&raw const PENDING_COUNT).read_volatile();
        let base = (&raw const PENDING_INTERRUPTS).cast::<u32>();

        for (index, item) in buf.iter_mut().enumerate().take(count) {
            *item = base.add(index).read_volatile();
        }

        (&raw mut PENDING_COUNT).write_volatile(0);
        count
    };

    // SAFETY: restoring the state saved above
    unsafe { TargetIRQArch::restore_interrupts(interrupt_state) };

    count
}

//! This module holds all of the irq
//! handling and creation caps across platforms
//!
//! this module assumes only single core daedalus

use daedalus_program::TOTAL_POSSIBLE_SIMULTANEOUS_INTERRUPTS;
use lepton3::lepton_vm::{
    heap_allocator::HeapAllocator,
    tagger::TagGenerator,
    values::{Tag, Value},
};

use crate::{
    ipc::capabilities::DaedalusVm,
    irq::pending::{drain_pending_into_buf, pending_any},
    program::{CallTag, Message},
};

/// A tag handle to a registered interrupt for a program
/// that the program can use to refer to a specific interrupt.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
pub struct IrqHandle(pub Tag);

/// A registrated binding of an IRQ to a program.
///
/// This should be referred to by some `IRQ` int id.
#[derive(Clone, Copy, Debug)]
pub struct IrqBinding {
    /// Which program receives messages for this
    pub program: &'static str,

    /// The tag that refers to this specific IRQ
    /// binding in the program's space.
    pub irq_handle: IrqHandle,
}

/// The pending map, this is how we tell
/// in the normal context what irqs have been
/// fired from the IRQ context to signal the
/// correct processes with
pub mod pending;

/// Generic abstraction layer trait
/// for architecture specific elements
///
/// if an arch impls this trait, they impl
/// all the `Daedalus` IRQ ops.
pub mod arch;

/// Platform specific details that maybe required
/// by an `IrqArch`.
pub mod plats;

/// The specific architecture that we
/// are using for this build for IRQ
pub mod archs;

/// The actual capabilities themselves which provide IRQ
/// access functionality
pub mod capabilities;

/// For the `virtual_machine` provided, this sends all of the corresponding
/// pending interrupts to the program's inbox that they are bound to.
///
/// # Safety
///
/// This must be called only in a normal context, and only in a
/// single-core/CPU environment.
pub unsafe fn send_irqs<H: HeapAllocator, T: TagGenerator>(virtual_machine: &mut DaedalusVm<H, T>) {
    // # Safety
    //
    // single-core precondition preserved
    let any_pending = unsafe { pending_any() };
    if !any_pending {
        return;
    }

    // Get all of the fired interrupt ids out of the pending tracker
    //
    // # Safety
    //
    // normal context preconditions preserved
    // we are also routing the fired interrupts through to the inboxes.
    let mut fired = [0u32; TOTAL_POSSIBLE_SIMULTANEOUS_INTERRUPTS];
    let num_fired = drain_pending_into_buf(&mut fired);

    for interrupt_id in &fired[..num_fired] {
        // Get the program this interrupt should route to
        let Some(binding) = virtual_machine
            .capability_state
            .irqs
            .get(interrupt_id)
            .copied()
        else {
            // Nobody currently holds a binding to this interrupt, maybe
            // they died before recieving it, but that's lowkey chill so just ignore.
            continue;
        };

        // Message we r gonna send to the inbox
        //
        // The tag is the IrqHandle and the value is
        // the interrupt id, the user should match on handle first.
        let message = Message {
            tag: Some(CallTag(binding.irq_handle.0)),
            args: Value::UInt(u64::from(*interrupt_id)),
        };

        // If it is the running program, simply push back to it's inbox.
        if binding.program == virtual_machine.capability_state.current_program {
            virtual_machine.capability_state.inbox.push_back(message);
        } else if let Some(program) = virtual_machine
            .capability_state
            .programs
            .get_mut(binding.program)
        {
            // Otherwise, just push this as a message to the caller program's inbox,
            // and wake it up if necessary (waiting for a inbox msg)
            program.inbox.push_back(message);

            if program.wake_recv() {
                virtual_machine
                    .capability_state
                    .ready_queue
                    .push_back(binding.program);
            }
        }
    }
}

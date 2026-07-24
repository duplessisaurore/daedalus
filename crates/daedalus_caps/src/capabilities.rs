//! This file contains all of the actual
//! capabilities that provide for IPC
//! between `Daedalus` Program
//!
//! All of the mentioned `Messages` following
//! for IPC follow this format generally:
//!
//!     [<top> `arg`/`payload`, `call_tag`]
//!
//! This `call_tag` typically holds a unique
//! tag allocated in the current program's tag space
//! that can be used to reply to the message.
//!
//! All of the messages only have one `payload`/`arg`
//! field, a program can send multiple args through the
//! usage of an array/object which are cloned over.
//!
//! Generally all `call_tag`'s should be a non-Unit value
//! which is a `Tag`, but this is not always the case!
//!
//! A program which goes to a blocked state (blockonrecv) etc.
//! and then is woken up as the next phase on a `finish` call,
//! will recieve a `call_tag` of Unit.
//!
//! This essentially marks a special case where daedalus
//! wakes up the program, but obviously a Unit `call_tag` cannot
//! be replied to because it doesn't actually come from a program.
//!
//! Whenever a caller recieves back their reply, they also
//! recieve the `Message` in the same format. The `call_tag` the
//! caller recieves in this reply is allocated in the caller's tag space.
//!
//! This is important for the usage of `non_block_call`, which for
//! a caller to match back the reply to some call will need to match
//! by the `call_tag` allocated in `non_block_call` (which will be the
//! same one used in the reply `Message`.)

use core::error::Error;

use alloc::{boxed::Box, string::ToString, vec::Vec};
use daedalus_program::{
    Program, StaticDaedalusImageVariants, StaticSourceLocation, get_phase, get_program,
};
use lepton3::{
    VirtualMachine,
    lepton_vm::{
        heap_allocator::{HeapAllocator, HeapItem},
        tagger::TagGenerator,
        values::Value,
    },
};

use crate::{
    errors::DaedalusCapErrors,
    migrate::migrate,
    program::{
        CallAssociation, CallTag, DaedalusState, InactiveProgram, Message, ProgramState,
        ProgramSwappable,
    },
};

/// a DaedalusVM, this is a VM that daedalus runs.
///
/// We just make a type alias because else it'd be a lot
/// of repeated code TwT
pub type DaedalusVm<H, T> = VirtualMachine<
    'static,
    DaedalusState<StaticDaedalusImageVariants, H, T>,
    StaticSourceLocation,
    H,
    T,
    StaticDaedalusImageVariants,
>;

/// This decodes a program's name as a `Lepton3` value down
/// into the program's name as a &'static str and returns the
/// associated program with this name
///
/// (or a CapabilityError when it could not be found)
fn program_from_value_name<H: HeapAllocator>(
    name_value: &Value,
    heap: &H,
) -> Result<&'static Program<StaticDaedalusImageVariants>, DaedalusCapErrors> {
    // A string is always an array of UInt's
    let Value::Array(index) = name_value else {
        return Err(DaedalusCapErrors::ProgramNameExpected);
    };

    let HeapItem::Array(fields) = heap.get_item(*index) else {
        return Err(DaedalusCapErrors::ProgramNameExpected);
    };

    // Collect all the string bytes and validate them as a utf-8 str
    let mut bytes = Vec::with_capacity(fields.len());
    for field in fields {
        let Value::UInt(byte) = field else {
            return Err(DaedalusCapErrors::ProgramNameExpected);
        };

        let byte = u8::try_from(*byte).map_err(|_| DaedalusCapErrors::ProgramNameExpected)?;
        bytes.push(byte);
    }

    let name = core::str::from_utf8(&bytes).map_err(|_| DaedalusCapErrors::ProgramNameExpected)?;

    // Look up the corresponding program with this name
    get_program(name).ok_or_else(|| DaedalusCapErrors::CouldNotFindProgram {
        looked_up_program_name: name.to_string(),
    })
}

/// Advances the `current_phase` of the DaedalusState of a VM to its successor.
///
/// The `entry_argument` if Some is passed, and the next program is a newly
/// started program, will be passed to the new program as part of it's entry
/// point's arguments. If `None` and the next program is new, then no arg will
/// be passed.
///
/// Regardless of the `entry_argument` if the next program is not new, a new
/// "notification"-style `Message` will be added to it's inbox to signal this.
fn advance_phase<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
    entry_argument: Option<Value>,
) -> Result<(), DaedalusCapErrors> {
    // Have we reached the end or not?
    //
    // todo: handle result with end for jumping to the entry point for running LionsOS
    let next_phase = get_phase(virtual_machine.capability_state.current_phase.next)
        .ok_or(DaedalusCapErrors::EndOfPhases)?;

    // Name of the next program to start
    let name = next_phase.program.name;

    // We have finished the current program => next phase is the same one
    if name == virtual_machine.capability_state.current_program {
        // Deliver through the inbox to that program, as a signal to wake it back up
        // on the advance case that isn't through `finish`
        //
        // We don't wakae up this program here because that should be handled by
        // the fast-path in `block_recv` which checks for a new message before
        // blocking fully
        virtual_machine.capability_state.inbox.push_back(Message {
            tag: None,
            args: Value::Unit,
        });
    } else {
        // Check if the program actually exists in the sets of programs
        // (which means its been ran before and not exited)
        let exists = virtual_machine.capability_state.programs.contains_key(name);

        match entry_argument {
            // If the program doesn't exist, and we have some entry arg,
            // spawn it with the arg
            Some(argument) if !exists => {
                let source_heap = &mut virtual_machine.heap;
                let state = &mut virtual_machine.capability_state;

                let program = InactiveProgram::from_image_with_name_and_arg(
                    next_phase.program.image,
                    name,
                    argument,
                    source_heap,
                );

                state.programs.insert(name, program);
                state.ready_queue.push_back(name);
            }

            // Fresh spawn without an argument, this will spawn it for us
            // and mark it as ready, or if already exists push the "notificaiton"-`Message`
            // and make it ready.
            _ => {
                virtual_machine
                    .capability_state
                    .make_ready(name, next_phase.program.image);
            }
        }
    }

    // Update the phase so we can advance to the next phase when the time comes :3
    virtual_machine.capability_state.current_phase = next_phase;
    Ok(())
}

/// Forcibly advance phases until something is made runnable
/// in the `Ready` queue, this does not pass any entry arguments
///
/// This does not handle inboxes since `block_recv` will have its
/// own fast-path for that
fn ensure_runnable<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), DaedalusCapErrors> {
    while virtual_machine.capability_state.ready_queue.is_empty() {
        advance_phase(virtual_machine, None)?;
    }

    Ok(())
}

/// Swaps the VM to the next program in the ready queue of the `DaedalusState`
///
/// This saves the old program with the state of `save_current` (if Some), this is
/// an `Option` as we may not actually want to save the program, and if `None` the
/// program is simply dropped.
fn run_next_ready<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
    save_current: Option<ProgramState>,
) -> Result<(), DaedalusCapErrors> {
    // Collect gc to reduce unneeded space in storage
    // NOTE: if really req memory, also maybe compress old program popped out of VM?
    virtual_machine.gc_collect();

    // Pick the next program and steal it out of the ready programs
    // so we can hold it's full state to swap with
    let next_program = {
        let state = &mut virtual_machine.capability_state;

        let next_name = state
            .ready_queue
            .pop_front()
            .ok_or(DaedalusCapErrors::NothingToRunDeadLock)?;

        state
            .programs
            .remove(next_name)
            .expect("expected that ready queue programs always exist in programs map, invariant")
    };

    // Swap to the new program and get the old program out with its new state
    let old_program =
        virtual_machine.swap(next_program, save_current.unwrap_or(ProgramState::Ready));

    // If we should save the current program, shove it into the programs
    // as an incative program, or drop it
    let state = &mut virtual_machine.capability_state;
    match save_current {
        None => {}
        Some(program_state) => {
            let previous_name = old_program.name;

            if program_state == ProgramState::Ready {
                state.ready_queue.push_back(previous_name);
            }

            state.programs.insert(previous_name, old_program);
        }
    }

    Ok(())
}

/// Sends a request to another program, taking the arguments
/// from the stack of the VM and then calls a destination program.
///
/// This is done through these steps:
///     - pop [<top> `payload`/`arg`, `destination`], destination here is prog name
///     - this then allocates a new tag in the caller (current) and the destination's space
///     - migrates the `payload/arg` into the `destination`
///     - adds a new message with the dest call_tag and migrataed payload to the destination
///     - records CallAssociation for this program in the destination for it to match replying back with
///     - and wakes the dest program if it was in a `BlockOnRecv` state (as in it's now `Ready`).
///
/// Returns the caller-side tag (that is the tag allocatedd in the current program)
fn send_request<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<CallTag, DaedalusCapErrors> {
    let argument = virtual_machine
        .stack
        .pop()
        .ok_or(DaedalusCapErrors::StackUnderflowExpectedMessageArgPayload)?;
    let name_value = virtual_machine
        .stack
        .pop()
        .ok_or(DaedalusCapErrors::StackUnderflowExpectedProgramName)?;

    // Find the destination program spec/embedded from the value..
    let destination = program_from_value_name(&name_value, &virtual_machine.heap)?.name;

    // Make sure it's not calling itself (shoot yourself in the foot behaviour)
    // and actually is a program that was running
    {
        let state = &virtual_machine.capability_state;

        if destination == state.current_program {
            return Err(DaedalusCapErrors::CallToSelf(destination));
        }

        if !state.programs.contains_key(destination) {
            return Err(DaedalusCapErrors::UnknownDestination(destination));
        }
    }

    // Caller-side tag, this is the tag in the current program's space
    let caller_tag = CallTag(virtual_machine.tagger.allocate_tag());

    let caller_heap = &mut virtual_machine.heap;
    let state = &mut virtual_machine.capability_state;
    let caller_name = state.current_program;

    // Find the inactive destination program we are calling.
    let destination_program = state
        .programs
        .get_mut(destination)
        .expect("checked to exist above");

    // Destination-side tag, this will be used for it to reply back to us (caller)
    let destination_tag = CallTag(destination_program.tagger.allocate_tag());

    // Migrate the argument over into the destination's heap so it's valid.
    let argument = migrate(
        caller_heap,
        &mut destination_program.heap,
        &mut destination_program.tagger,
        argument,
    );

    // Add the CallAssociation so we can match call tags for `non_block_call` and know
    // which program we are actually replying to
    destination_program.pending_replies.insert(
        destination_tag,
        CallAssociation {
            caller_side_tag: caller_tag,
            caller_program: caller_name,
        },
    );

    // Add this message to the inbox of the destination program
    // so if it's blocked on recv it can recv it.
    destination_program.inbox.push_back(Message {
        tag: Some(destination_tag),
        args: argument,
    });

    // Wake up the program if needed
    if destination_program.wake_recv() {
        state.ready_queue.push_back(destination);
    }

    Ok(caller_tag)
}

/// = `finish`
///
/// This capability ends the current phase and the current program,
/// beginning the next phase.
///
/// This capability takes one argument:
///
///     [<top> `arg`]
///
/// If the next phase is `end`, the entire boot process is assumed to
/// have finished. This will raise the `EndOfPhases` error, TODO: actually
/// jump and finish the boot process.
///
/// Otherwise the next phase is loaded, as per `advance_phase`, with
/// the `arg` being provided to the next phase if necessary.
///
/// This expects the `arg` to exist on the stack and will StackUnderflow
/// if the arg doesn't exist
pub fn cap_finish<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    // Get the argument off the stack, this is the argument to the next phase.
    // TODO: on end phase, use this as address to jump to, to start the actual
    // OS
    let argument = virtual_machine
        .stack
        .pop()
        .ok_or(DaedalusCapErrors::StackUnderflowFinishArg)?;

    // Get to the next phase
    advance_phase(virtual_machine, Some(argument))?;

    // The successor may not have become runnable if it's blocked or something,
    // but we don't want to deadlock so ensure we run something.
    ensure_runnable(virtual_machine)?;

    // Run the next ready phase! (we don't save the current since we are `finishing it)
    run_next_ready(virtual_machine, None)?;
    Ok(())
}

/// = `block_recv`
///
/// This capability blocks the current program until a message is
/// received in its inbox, delivering it as:
///
///     [<top> `arg`, `call_tag`]
/// 
/// To the current stack of the program.
/// 
/// This `Message` can either be a request to the current program `call_tag`
/// is a tag, or a "notification" from daedalus with a `Unit` in the `call_tag`.
///
/// If the inbox already holds a message it is delivered immediately
/// with no program switching and state changing (fast-path). 
/// 
/// Otherwise the program's state is changed to `BlockedOnRecv` and the next
/// program runs.
///
/// For requests, the `call_tag` is important and should not be lost as
/// the caller may not ever be woken up with the corresponding reply is never
/// sent back to this caller with the `call_tag`.
pub fn cap_block_recv<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    loop {
        // Check if there is already something in the inbox (fast-path)
        if let Some(message) = virtual_machine.capability_state.inbox.pop_front() {

            // Ok! we have something, add it to our stack and return
            message.deliver_onto(&mut virtual_machine.stack);
            return Ok(());
        }
        
        if !virtual_machine.capability_state.ready_queue.is_empty() {
            break;
        }
 
        // Nothing to receive and nobody to run, in this case we are
        // advancing the phase to the next phase to continue running the boot process
        // (else we will instantly halt)
        advance_phase(virtual_machine, None)?;
    }
    
    // There is no message to be recieved by the program, block it and run the next guy.
    run_next_ready(virtual_machine, Some(ProgramState::BlockedOnRecv))?;
    Ok(())
}

 
/// = `block_call`
///
/// This capability blocks the current program and pushes a message
/// into the inbox of the destination program. the arguments to this
/// are:
///
///     [<top> `arg`, `name`]
///
/// The `name` references which destination program to target for the 
/// message. This `name` must be provided as a `String` in the same format
/// as that provided by the `Boson3` lowerer. (an array of UInts).
/// 
/// and `arg`/`payload` is the argument to the destination program as
/// part of this message, it is cloned over/migrated.
///
/// If the destination program is in the `BlockedOnRecv` state it is
/// rescheduled into the `Ready` state.
/// 
/// The caller then changes state to `BlockedOnReply` with a newly
/// allocated `call_tag` which the destination program must reply to for
/// this program to wake up again.
/// 
/// The returned value from this block_call is guaranteed to be something
/// in this format:
/// 
///     [<top> `ret_arg`, `call_tag`]
/// 
/// Generally this `call_tag` is not really useful, but provided for
/// uniformity with `non_block_call` and can be ignored.
pub fn cap_block_call<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    let caller_tag = send_request(virtual_machine)?;
    
    // Block the current program, run the next ready one (maybe our
    // destination program :) )
    run_next_ready(
        virtual_machine,
        Some(ProgramState::BlockedOnReply { tag: caller_tag }),
    )?;
    Ok(())
}




 

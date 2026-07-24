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

use alloc::{string::ToString, vec::Vec};
use daedalus_program::{Program, StaticDaedalusImageVariants, StaticSourceLocation, get_phase, get_program};
use lepton3::{VirtualMachine, lepton_vm::{
    heap_allocator::{HeapAllocator, HeapItem}, tagger::TagGenerator, values::Value,
}};

use crate::{errors::DaedalusCapErrors, program::{DaedalusState, InactiveProgram, Message}};

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
    get_program(name).ok_or_else(
        || DaedalusCapErrors::CouldNotFindProgram { looked_up_program_name: name.to_string() }
    )
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
        let exists = virtual_machine
            .capability_state
            .programs
            .contains_key(name);
 
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

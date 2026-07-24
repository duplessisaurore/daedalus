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
use daedalus_program::{Program, StaticDaedalusImageVariants, get_program};
use lepton3::lepton_vm::{
    heap_allocator::{HeapAllocator, HeapItem},
    values::Value,
};

use crate::errors::DaedalusCapErrors;

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

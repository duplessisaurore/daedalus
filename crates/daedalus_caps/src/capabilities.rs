//! This file contains all of the actual
//! capabilities that are provide for IPC
//! between `Daedalus` Program's
//! 

 
/// = `finish`
/// 
/// This capability *ends* the current program and phase.
/// 
/// This essentially will load in the next phase following this one,
/// unless `end` is the next phase in which the entire boot process will
/// be assumed to have been finished. 
/// 
/// The current phase will then be swapped out to the next phase, and
/// the old program which we were just executing will be discarded.
/// 
/// The `finish` capability takes one argument which is passed to the
/// next phase's entry or with the `end` phase should be an address that is 
/// jumped to, completing the boot process.
/// 
/// The first phase recieves no arguments.

/// = `block_recv`
/// 
/// This capability blocks the current program until a message
/// is recieved in the inbox of the current program. This is blocked
/// in the `BlockOnRecv` state.
/// 
/// The `block_recv` syscall will produce both a `call_tag` `Tag` and
/// the message's arguments into the stack current program in the order of:
/// 
///     [<top> `arg`, `call_tag`, ...]
/// 
/// This `call_tag` is important and should not be lost as the caller
/// may never wake up if the corresponding `non_block_reply` to this tag
/// is never executed. 
/// 
/// This `call_tag` is essentially a unique marker of the current call 
/// from some caller and is used to reply back to that caller with the return
/// value.
/// 
/// The `arg` is always a single value, the caller may pass multiple parameters
/// through the usage of an array/object which will be migrated into the current program.

/// = `block_call`
/// 
/// This capability blocks the current program and pushes a message
/// into the inbox of the destination program.
/// 
/// If the destination program is in the `BlockedOnRecv` state, it is rescheduled
/// into the `Ready` state, otherwise the message will have to wait for
/// `block_recv` to be called in the destination program.
/// 
/// The arguments to the `block_call` are as follows:
/// 
///     [ <top> `arg`, `name` ]
/// 
/// The `name` references which program to target for the message, `arg` is
/// the argument that is passed to the destination program through this message 
/// (see `block_recv`).




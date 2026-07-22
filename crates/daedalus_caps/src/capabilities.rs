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
/// is recieved in the inbox of the current program.
/// 
/// Once a message is recieved, this program will then be rescheduled
/// to the `Ready` state which can then be picked up and executed by
/// the scheduler.
/// 
/// The `block_recv` syscall will produce both a `ret_call_tag` `Tag` and
/// the message's arguments into the current program in the order of:
/// 
///     [`arg`, `ret_call_tag`]
/// 
/// This `ret_call_tag` is important and should not be lost as the caller
/// may never wake up if the corresponding `non_block_reply` to this tag
/// is never executed. This `ret_call_tag` is essentially a unique marker
/// of the current call from some caller and is used to reply back to that
/// caller.
/// 
/// The `arg` is always a single value, the caller may pass multiple parameters
/// through the usage of an array which will be copied into the current program.


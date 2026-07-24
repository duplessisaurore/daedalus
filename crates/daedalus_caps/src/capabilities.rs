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

/// = `non_block_call`
/// 
/// This capability is similar to `block_call` but it does not block
/// the current program.
/// 
/// The arguments to `block_call` are the same, however the capability
/// pushes a `call_tag` onto the stack of the current program.
/// 
/// This `call_tag` is important as whenever the program yields after
/// doing a `non_block_call`, the specific response from which `non_block_call`
/// will need to be matched by the `call_tag`.


/// = `non_block_reply`
/// 
/// This capability sends a message back to the inbox of some program
/// associated with a `call_tag`, unblocking it if it's blocked on `BlockOnReply`
/// and setting it into the `Ready` state.
/// 
/// This does not block the current program.
/// 
/// The `non_block_reply` capability takes arguments in the following order:
/// 
///     [<top> `ret_arg`, `call_tag`]
/// 
/// The `call_tag` must be one produced by `wait_recv`. This is used to reply
/// back to a specific caller of this program. The `ret_arg` is then pushed onto
/// the stack of the caller, alongside a unique call tag associated with this call,
/// such that in the perspective of the caller the stack is as follows:
/// 
///     [<top> `ret_arg`, `call_tag`]
/// 
/// If the caller send this original `call_tag` through a `non_block_call`, the
/// caller can then match this using their corresponding `call_tag` produced by the
/// `non_block_call`.



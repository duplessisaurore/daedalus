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
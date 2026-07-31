//! These are all the possible errors that
//! can occur during the running of the `Daedalus`
//! capabilities.

use core::{error::Error, fmt::Display};

use alloc::string::String;

use crate::program::CallTag;

#[derive(Debug)]
pub enum DaedalusCapErrors {
    /// Attempted to look up a program under this name,
    /// but none could be found!
    CouldNotFindProgram { looked_up_program_name: String },

    /// A program name was expected here
    /// as per the capability, but a valid
    /// one could not be found
    ProgramNameExpected,

    /// The next phase is `end`
    ///
    /// This will be handled by finishing the boot process
    /// and jumping to the entry point from the final program,
    /// see `finish`.
    EndOfPhases,

    /// The scheduler had nothing runnable that could be found
    /// when trying to run something next...
    ///
    /// This should only happen when everything is blocked in a
    /// deadlock!
    NothingToRunDeadLock,

    /// Expected to pop an `arg`/`payload` to a `Message` capability,
    /// but nothing was found on the stack!
    StackUnderflowExpectedMessageArgPayload,

    /// Expected to pop a program `name` to jump to for a capability,
    /// but nothing was found on the stack!
    StackUnderflowExpectedProgramName,

    /// Expected to pop an `call_tag` for a reply to return back for
    /// a request, but nothing was found!
    StackUnderflowExpectedCallTag,

    /// A call named a program that exists technically, but has not
    /// yet been started in an earlier phase/has been ended so we can't
    /// actually call it!
    UnknownDestination(&'static str),

    /// A call to the current program
    /// This is not good.. do not do this. you are wasting time and should
    /// be instead calling a function in the local state instead of needing
    /// a full IPC call.
    CallToSelf(&'static str),

    /// The next phases's entry argument was expected to be provided on the stack
    /// to the `finish` capability, but nothing was found!
    StackUnderflowFinishArg,

    /// A reply tag was expected here, but a reply tag value was not found instead
    /// some other type was found here.
    ReplyTagExpected { found_type: &'static str },

    /// A valid reply tag was expected here, but this tag passed was unknown to
    /// be a reply tag, was it actually allocated by block_recv?
    UnknownReplyTag(CallTag),

    /// Attempted to reply to this caller, but the caller is now gone ? :(
    /// This is the name of the caller.
    CallerGone(&'static str),

    /// These permissison bits for memory regions were attempted to be
    /// set but they do not currently exist
    InvalidRegionPermission(u64)
}

impl Display for DaedalusCapErrors {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EndOfPhases => {
                write!(f, "daedalus reached the `end` of the boot phases!")
            }

            Self::CouldNotFindProgram {
                looked_up_program_name,
            } => {
                write!(
                    f,
                    "daedalus capability tried to look up program `{looked_up_program_name}`, but could not find any program with that name!"
                )
            }
            Self::ProgramNameExpected => write!(
                f,
                "daedalus capability expected a program name as a value, found an invalid one!"
            ),
            Self::NothingToRunDeadLock => {
                write!(
                    f,
                    "daedalus scheduler could find no runnable program, every program must be blocked in a deadlock!"
                )
            }
            Self::StackUnderflowExpectedMessageArgPayload => {
                write!(
                    f,
                    "daedalus expected to find some payload/arg to a message on the stack, but nothing was found!"
                )
            }
            Self::StackUnderflowExpectedProgramName => {
                write!(
                    f,
                    "daedalus expected to find some program name on the stack but nothing was found!"
                )
            }
            Self::UnknownDestination(name) => {
                write!(
                    f,
                    "daedalus found a call destination `{name:?}`, but it is not currently running/was never started!"
                )
            }
            Self::CallToSelf(name) => {
                write!(
                    f,
                    "daedalus found a program call that called its own program: `{name:?}`, this is not good behaviour and explicitly disallowed!"
                )
            }
            Self::StackUnderflowFinishArg => {
                write!(
                    f,
                    "daedalus expected to find some argument to the next phase's program but nothing was found!"
                )
            }
            Self::StackUnderflowExpectedCallTag => {
                write!(
                    f,
                    "daedalus expected to find some call tag to reply with but nothing was found!"
                )
            }
            Self::ReplyTagExpected { found_type } => {
                write!(
                    f,
                    "daedalus expected to find some call tag to reply with, but instead found a `{found_type}`!"
                )
            }
            Self::UnknownReplyTag(tag) => {
                write!(
                    f,
                    "daedalus attempted to call non_block_reply with tag `{tag:?}`, but this tag has no outstanding request to reply with, did it really get allocated by `block_recv`?"
                )
            }
            Self::CallerGone(name) => {
                write!(
                    f,
                    "daedalus expected to find a reply target `{name:?}`, but there is no longer a running program by the time we are replying to it!"
                )
            }
            Self::InvalidRegionPermission(bits) => {
                write!(
                    f,
                    "daedalus expected a valid set of permission bits in provided permission, however `{bits}` contained invalid permission bits!"
                )
            }
        }
    }
}

impl Error for DaedalusCapErrors {}

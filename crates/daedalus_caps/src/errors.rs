//! These are all the possible errors that
//! can occur during the running of the `Daedalus`
//! capabilities.

use core::{error::Error, fmt::Display};

use alloc::string::String;
use daedalus_program::{InvalidRegionPermissionBitsError, RegionPermissions};

use crate::{memory::RegionHandle, program::CallTag};

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
    InvalidRegionPermission(InvalidRegionPermissionBitsError),

    /// This region overflowed the available memory space and cannot validly
    /// exist in the system
    RegionOverflow { base: usize, len: usize },

    /// This region is attempting to be created in a location in which it
    /// would write over the memory of daedalus, and is not allowed.
    WritableOverDaedalus { base: usize, len: usize },

    /// Attempted to access a region with a permissions
    /// mismatch between the attempted access and the region's
    /// actual permissions.
    PermissionDenied {
        need: RegionPermissions,
        held: RegionPermissions,
    },

    /// This access overflowed the available space on the system such that
    /// the end cannot even exist in the system's memory !! wuehhh
    AccessOverflow { offset: usize, width: usize },

    /// This access exceeded the length of the region
    OutOfRegion {
        offset: usize,
        width: usize,
        len: usize,
    },

    /// This was an unaligned access to a region at
    /// some address, essentially the offset + region base
    /// was not aligned to the specified width of the access.
    Misaligned { address: usize, width: usize },

    /// This access width is an invalid one and does not match
    /// the permitted 1,2,4,8 bytes allowed.
    InvalidAccessWidth { width: usize },

    /// Attempted to create a `Region` from a `Grant` that had a base up to ToStartDaedalus
    /// but the grant exceeded the actual starting address of daedalus!
    GrantBaseAboveDaedalus { role: &'static str, base: usize },

    /// Attempted to create a `Grant` up to the end of memory, except the base
    /// of the grant actually already excceeded the end of memory!
    GrantBaseOutsideMemory { role: &'static str, base: usize },

    /// Attempted to create a new image with name `name`, which requires entering
    /// its entry point function, however this failed! and we could not enter its entry
    /// point function.
    FailedToEnterProgramEntryPoint { name: &'static str },

    /// Expected to pop a `role` for a `mem_grant` call in order
    /// to grab a region from this `role`.
    StackUnderflowExpectedGrantRole,

    /// A grant role name was expected here
    /// as per the capability, but a valid
    /// one could not be found
    GrantRoleExpected,

    /// A grant call looked up a role that does not exist in
    /// the manifest/grants of the current program
    CouldNotFindGrantRole { looked_up_role: String },

    /// Expected a region handle on the stack, but
    /// no region handle could be found!
    StackUnderflowExpectedRegionHandle,

    /// A region handle was expected, but some other value type was
    /// found on the stack instead.
    RegionHandleExpected { found_type: &'static str },

    /// Expected a permissions value here but nothing was found
    StackUnderflowExpectedRegionPermissions,

    /// Expected a permissions value here, but instead
    /// found an unexpected type!
    RegionPermissionsExpected { found_type: &'static str },

    /// Expected an access/derive length at this position
    /// but nothing was found!
    StackUnderflowExpectedRegionAccessDeriveLength,

    /// Expected a access/derive value here, but instead
    /// found an unexpected type!
    RegionAccessDeriveLengthExpected { found_type: &'static str },

    /// The length provided is too large to fit into
    /// a word size on the platform! this cannot be possibly
    /// a region length!
    LengthTooLarge { illegal_length: u64 },

    /// Expected an access/derive offset at this position
    /// but nothing was found!
    StackUnderflowExpectedRegionAccessDeriveOffset,

    /// Expected a access/derive value here, but instead
    /// found an unexpected type!
    RegionAccessDeriveOffsetExpected { found_type: &'static str },

    /// The offset provided is too large to fit into
    /// a word size on the platform! this cannot be possibly
    /// a region offset!
    OffsetTooLarge { illegal_offset: u64 },

    /// A valid region handle was expected here, but this tag passed
    /// was unknown to be a valid region handle for the current program.
    UnknownRegionHandle(RegionHandle),
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
                    "daedalus found attempt to call non_block_reply with tag `{tag:?}`, but this tag has no outstanding request to reply with, did it really get allocated by `block_recv`?"
                )
            }
            Self::CallerGone(name) => {
                write!(
                    f,
                    "daedalus expected to find a reply target `{name:?}`, but there is no longer a running program by the time we are replying to it!"
                )
            }
            Self::InvalidRegionPermission(region_error) => {
                write!(
                    f,
                    "daedalus expected a valid set of permission bits in provided permission, however `{}` contained invalid permission bits!",
                    region_error.0
                )
            }
            Self::RegionOverflow { base, len } => {
                write!(
                    f,
                    "a region was attempted to be created which overflows the available memory of the system, with base `{base}` and length `{len}`."
                )
            }
            Self::WritableOverDaedalus { base, len } => {
                write!(
                    f,
                    "a region was attempted to be created which has `write` permission and overlaps with the memory of daedalus! with base `{base}` and length `{len}`."
                )
            }
            Self::PermissionDenied { need, held } => {
                write!(
                    f,
                    "region access permission denied: attempted to access with permissions `{need}`, actual held region permissions `{held}`."
                )
            }
            Self::AccessOverflow { offset, width } => {
                write!(
                    f,
                    "a access was attempted which overflows the available memory of the system, with offset `{offset}` and width `{width}`."
                )
            }
            Self::OutOfRegion { offset, width, len } => {
                write!(
                    f,
                    "a region access was attempted with offset `{offset}` and width `{width}`, however this exceeds the bounds of the region which has length `{len}`."
                )
            }
            Self::Misaligned { address, width } => {
                write!(
                    f,
                    "a region access was attempted at address `{address}`, however this address is not aligned to the requested access width of `{width}` bytes."
                )
            }
            Self::InvalidAccessWidth { width } => {
                write!(
                    f,
                    "attempted an access with invalid width of `{width}` bytes, must be one of [1, 2, 4, 8]."
                )
            }
            Self::GrantBaseAboveDaedalus { role, base } => {
                write!(
                    f,
                    "the grant with role `{role}` and length `to_start_daedalus` was given base `{base}` which exceeds the starting address of daedalus."
                )
            }
            Self::GrantBaseOutsideMemory { role, base } => {
                write!(
                    f,
                    "the grant with role `{role}` and length `to_end_of_memory` was given base `{base}`, but this is at or past the end of physical memory as specified by daedalus."
                )
            }
            Self::FailedToEnterProgramEntryPoint { name } => {
                write!(
                    f,
                    "expected to be able to enter entry point of program with name `{name}` but this failed!"
                )
            }
            Self::StackUnderflowExpectedGrantRole => {
                write!(
                    f,
                    "daedalus expected to find a grant `role` name on the stack but nothing was found!"
                )
            }
            Self::GrantRoleExpected => {
                write!(
                    f,
                    "daedalus expected a grant `role` name as a `Boson3` string value (UInt Array), found an invalid one!"
                )
            }
            Self::CouldNotFindGrantRole { looked_up_role } => {
                write!(
                    f,
                    "daedalus capability tried to look up grant with role `{looked_up_role}`, but could not find any grants with that role!"
                )
            }
            Self::StackUnderflowExpectedRegionHandle => {
                write!(
                    f,
                    "daedalus expected to find a memory region handle on the stack but nothing was found!"
                )
            }
            Self::RegionHandleExpected { found_type } => {
                write!(
                    f,
                    "daedalus expected a memory region handle, but instead found a `{found_type}`!"
                )
            }
            Self::StackUnderflowExpectedRegionPermissions => {
                write!(
                    f,
                    "daedalus expected to find memory region permissions on the stack but nothing was found!"
                )
            }
            Self::RegionPermissionsExpected { found_type } => {
                write!(
                    f,
                    "daedalus expected memory region permissions, but instead found a `{found_type}`!"
                )
            }
            Self::StackUnderflowExpectedRegionAccessDeriveLength => {
                write!(
                    f,
                    "daedalus expected to find memory length for region access/derive size on the stack but nothing was found!"
                )
            }
            Self::RegionAccessDeriveLengthExpected { found_type } => {
                write!(
                    f,
                    "daedalus expected memory length for region access/derive size, but instead found a `{found_type}`!"
                )
            }
            Self::LengthTooLarge { illegal_length } => {
                write!(
                    f,
                    "daedalus expected memory length for region access/derive size to be within the bounds of `usize`, however it was not! instead got illegal length of `{illegal_length}`!"
                )
            }
            Self::StackUnderflowExpectedRegionAccessDeriveOffset => {
                write!(
                    f,
                    "daedalus expected to find memory offset from base for region access/derive on the stack but nothing was found!"
                )
            }
            Self::RegionAccessDeriveOffsetExpected { found_type } => {
                write!(
                    f,
                    "daedalus expected memory offset from base for region access/derive, but instead found a `{found_type}`!"
                )
            }
            Self::OffsetTooLarge { illegal_offset } => {
                write!(
                    f,
                    "daedalus expected memory offset from base for region access/derive to be within the bounds of `usize`, however it was not! instead got illegal offset of `{illegal_offset}`!"
                )
            }
            Self::UnknownRegionHandle(handle) => {
                write!(
                    f,
                    "daedalus found attempt to invoke region operation with tag `{handle:?}`, but this tag is not a handle to a region, does it really refer to a valid memory region?"
                )
            }
        }
    }
}

impl Error for DaedalusCapErrors {}

impl From<InvalidRegionPermissionBitsError> for DaedalusCapErrors {
    fn from(value: InvalidRegionPermissionBitsError) -> Self {
        Self::InvalidRegionPermission(value)
    }
}

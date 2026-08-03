//! This defines the core architecture specific
//! trait that all architectures must implement
//! to inherently implement the memory operations
//! on that architecture.

use crate::errors::DaedalusCapErrors;

/// A single architecture's memory operations
///
/// This supplies the boundary between arch-generic and arch-specific
/// memory operations. Nothing arch-specific should be at a "higher level"
/// than this.
pub trait MemoryArch {
    /// Read some `width` bytes at the address supplied by `pointer`
    ///
    /// # Safety
    ///
    /// The caller must ensure that the current execution state (program)
    /// has actual access to the data specified by `pointer` with `width`.
    ///
    /// This access must be with `Read` permissions (`R`).
    ///
    /// The `width` and `pointer` must also be correctly aligned.
    unsafe fn read(pointer: *const u8, width: AccessWidth) -> u64;

    /// Write some `width` bytes specified by `value` to the address specified by `pointer`
    ///
    /// # Safety
    ///
    /// The caller must ensure that the current execution state (program)
    /// has actual access to the data specified by `pointer` with `width`.
    ///
    /// This access must be with `Write` permissions (`W`).
    ///
    /// The `width` and `pointer` must also be correctly aligned.
    unsafe fn write(pointer: *mut u8, width: AccessWidth, value: u64);

    /// Copy some `len` bytes over from address at `source` to the address at `destination`
    ///
    /// These two locations are permitted to overlap, and the semantics should be as follows:
    ///
    /// The source values must never be overwritten, if `source` < `destination` < `source + len`
    /// then we get this case:
    ///
    ///     here overwrites
    ///          v
    /// source | <----------(len)----------->
    ///      destination | <----------(len)----------->
    ///                     ^
    ///                   here !
    ///
    /// In this case we copy backwards from source to destination
    /// because else if we copy from the start of source to the
    /// start of destination we are erasing data from source
    ///
    /// Otherwise the data should be copied in a forward manner to preserve the correctness of
    /// the copying as follows:
    ///
    ///               source | <----------(len)----------->
    ///      destination | <----------(len)----------->
    ///
    /// in which case we want to write forward to prevent
    /// overwriting data in the same way.
    ///
    /// # Safety
    ///
    /// The implementation should ensure the accesses are aligned
    /// if they must be, the memory must  be validated to be in
    /// a `Memory`-type region (non-device memory).
    ///
    /// The caller must ensure that the current execution state (program)
    /// has actual access to `len` bytes at `source` and to `len` bytes
    /// at `destination`.
    ///
    /// The source access must be with `Read` permissions (`R`) and the
    /// destination access with `Write` permissions (`W`).
    unsafe fn copy(source: *const u8, destination: *mut u8, len: usize);

    /// Fill some address at `destination` with `len` number of `value`-bytes
    ///
    /// Generally the same semantics as `copy`.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the current execution state (program)
    /// has actual access to `len` bytes at `destination`, with `Write`
    /// permissions (`W`).
    ///
    /// The implementation should ensure the accesses are aligned
    /// if they must be, The memory must be validated to be in a
    /// `Memory`-type region (non-device memory).
    unsafe fn fill(destination: *mut u8, value: u8, len: usize);

    /// Flush a range of memory beginning at `pointer` with length of `len`
    ///
    /// This is for archs which need to flush the written data from the bootloader
    /// before it can be executed as instructions. (say flush from d-cache so i-cache
    /// can read it and execute it)
    ///
    /// # Safety
    ///
    /// The caller must ensure that the current execution state (program)
    /// has actual access to `len` bytes at `pointer`, with at least
    /// `Read` permissions (`R`).
    ///
    /// The range must be addressable memory and must actually be
    /// roundable to a cache line for flushing.
    unsafe fn flush_range(pointer: *mut u8, len: usize);

    /// Setup this specific architecture.
    ///
    /// Some architectures may require arch specific setup/teardown of previous
    /// stage things before `Daedalus` can start.
    ///
    /// # Safety
    ///
    /// The caller must ensure this runs before any memory is ever written
    /// to by `Daedalus` at all!!!
    ///
    /// The caller must ensure that `Daedalus` only ever runs on one core at
    /// once.
    unsafe fn setup();

    /// Teardown this specific architecture.
    ///
    /// This should generally do the inverse of any setup that was required,
    /// such that we tore down anything `Daedalus` setup so that we can hand off
    /// to the OS/kernel plainly.
    ///
    /// # Safety
    ///
    /// The caller must ensure that every write through every possible bootloader
    /// stage has already completed, and the only remaining operations are within
    /// `Daedalus`'s handoff mechanisms.
    ///
    /// There must be no following `MemoryArch` operations.
    ///
    /// The caller must ensure that `Daedalus` only ever runs on one core at
    /// once.
    unsafe fn teardown();
}

/// Different possible access widths for a certain
/// pointer in `MemoryArch` for simple `read/write`'s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessWidth {
    U8,
    U16,
    U32,
    U64,
}

impl AccessWidth {
    /// Return the underlying number of bytes
    /// for an `AccessWidth`
    pub const fn bytes(self) -> usize {
        match self {
            Self::U8 => 1,
            Self::U16 => 2,
            Self::U32 => 4,
            Self::U64 => 8,
        }
    }

    /// Return the corresponding `AccessWidth`
    /// for some `usize` access width (must be 1, 2, 4, 8)
    pub fn from_bytes(bytes: usize) -> Result<Self, DaedalusCapErrors> {
        Ok(match bytes {
            1 => Self::U8,
            2 => Self::U16,
            4 => Self::U32,
            8 => Self::U64,
            _ => Err(DaedalusCapErrors::InvalidAccessWidth { width: bytes })?,
        })
    }

    /// Returns the maximum value storable in a U64 with
    /// this access width.
    pub const fn max_value(self) -> u64 {
        match self {
            Self::U8 => u8::MAX as u64,
            Self::U16 => u16::MAX as u64,
            Self::U32 => u32::MAX as u64,
            Self::U64 => u64::MAX,
        }
    }
}

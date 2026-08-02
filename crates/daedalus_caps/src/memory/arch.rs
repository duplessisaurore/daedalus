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
    unsafe fn read(pointer: *const u8, width: AccessWidth) -> u64;

    /// Write some `width` bytes specified by `value` to the address specified by `pointer`
    unsafe fn write(pointer: *mut u8, width: AccessWidth, value: u64);

    /// Copy some `len` bytes over from address at `source` to the address at `destination`
    unsafe fn copy(source: *const u8, destination: *mut u8, len: usize);

    /// Fill some address at `destination` with `len` number of `value`-bytes
    unsafe fn fill(destination: *mut u8, value: u8, len: usize);

    /// Flush a range of memory beginning at `pointer` with length of `len`
    ///
    /// This is for archs which need to flush the written data from the bootloader
    /// before it can be executed as instructions. (say flush from d-cache so i-cache
    /// can read it and execute it)
    unsafe fn flush_range(pointer: *mut u8, len: usize);

    /// Setup this specific architecture.
    /// 
    /// Some architectures may require arch specific setup/teardown of previous
    /// stage things before `Daedalus` can start.
    unsafe fn setup();

    /// Teardown this specific architecture.
    /// 
    /// This should generally do the inverse of any setup that was required,
    /// such that we tore down anything `Daedalus` setup so that we can hand off
    /// to the OS/kernel plainly.
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

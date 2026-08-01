//! This module holds all of the memory
//! access related capabilities for all archs

use daedalus_program::{RegionMemKind, RegionPermissions};
use lepton3::lepton_vm::values::Tag;

use crate::errors::DaedalusCapErrors;

unsafe extern "C" {
    /// The starting point of daedalus in memory,
    /// this should contain the stack, heap etc.
    static __daedalus_start: u8;

    /// The ending point of daedalus in memory.
    static __daedalus_end: u8;
}

/// Generic abstraction layer trait
/// for architecture specific elements
///
/// if an arch impls this trait, they impl
/// all the `Daedalus` memory ops.
pub mod arch;

/// A unique memory region's tag handle which is
/// associated with some region
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
pub struct RegionHandle(pub Tag);

/// This is a `Region` of memory that
/// a `Daedalus` program has access to.
///
/// This is inherently some chunk of memory
/// with certain access permissions and a
/// certain `Kind` (for future memory setup such
/// that not all memory is just device-ng)
#[derive(Clone, Copy, Debug)]
pub struct Region {
    /// The start point of the memory region in bytes
    base: usize,

    /// The size of the memory region in bytes
    len: usize,

    /// The access permissions to grant to this program
    /// under this region
    perms: RegionPermissions,

    /// The kind of memory belonging to this region
    kind: RegionMemKind,
}

impl Region {
    /// Creates a new memory `Region` starting
    /// at some `base` extending some `len` with
    /// the `perms` and `kind` specified.
    ///
    /// This region must be within the usize
    /// of this system, must not be `W` permission
    /// wise and overlap daedalus.
    ///
    /// If it violates these regards, a `DaedalusCapErrors` is
    /// returned.
    pub fn new(
        base: usize,
        len: usize,
        perms: RegionPermissions,
        kind: RegionMemKind,
    ) -> Result<Self, DaedalusCapErrors> {
        base.checked_add(len)
            .ok_or(DaedalusCapErrors::RegionOverflow { base, len })?;

        if perms.contains(RegionPermissions::W) && overlaps_daedalus(base, len) {
            return Err(DaedalusCapErrors::WritableOverDaedalus { base, len });
        }

        Ok(Region {
            base,
            len,
            perms,
            kind,
        })
    }

    /// Checks whether or not some access at some `offset` with
    /// some `width` and some `need` permissions `Perms` can be
    /// validly done within this `Region`
    fn resolve(
        &self,
        offset: usize,
        width: usize,
        need: RegionPermissions,
    ) -> Result<*mut u8, DaedalusCapErrors> {
        // Make sure at least we have the permissions
        if !self.perms.contains(need) {
            return Err(DaedalusCapErrors::PermissionDenied {
                need,
                held: self.perms,
            });
        }

        // Find the end of the access
        // end must be within region len and offset must be greater than base
        let end = offset
            .checked_add(width)
            .ok_or(DaedalusCapErrors::AccessOverflow { offset, width })?;

        if end > self.len {
            return Err(DaedalusCapErrors::OutOfRegion {
                offset,
                width,
                len: self.len,
            });
        }

        // Ensure the address is aligned at this width we are
        // accessing at (grrr)
        if width != 0 && !(self.base + offset).is_multiple_of(width) {
            return Err(DaedalusCapErrors::Misaligned {
                address: self.base + offset,
                width,
            });
        }

        Ok((self.base + offset) as *mut u8)
    }
}

/// Checks whether or not the provided region starting
/// at `base` and with a length of `len` overlaps daedalus
/// as specified with the __daedalus_start and _end symbols.
fn overlaps_daedalus(base: usize, len: usize) -> bool {
    let start = (&raw const __daedalus_start) as usize;
    let end = (&raw const __daedalus_end) as usize;
    base < end && start < base.saturating_add(len)
}

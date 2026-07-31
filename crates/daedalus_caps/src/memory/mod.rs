//! This module holds all of the memory
//! access related capabilities for all archs

use core::fmt::Display;

use lepton3::lepton_vm::values::Tag;

use crate::errors::DaedalusCapErrors;

unsafe extern "C" {
    /// The starting point of daedalus in memory,
    /// this should contain the stack, heap etc.
    static __daedalus_start: u8;

    /// The ending point of daedalus in memory.
    static __daedalus_end: u8;
}

/// A unique memory region's tag handle which is
/// associated with some region
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
pub struct RegionHandle(pub Tag);

/// Permissions holdable by a region
///
/// R = bit 1
/// W = bit 2
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RegionPermissions(u8);

impl RegionPermissions {
    /// NO permissions are applicable in this region.
    /// This region has no operations that can be done upon it.
    pub const NONE: Self = RegionPermissions(0b00);

    /// This region is only readable from
    pub const R: Self = RegionPermissions(0b01);

    /// This region is only writable to
    pub const W: Self = RegionPermissions(0b10);

    /// This region can be written to/read from
    pub const RW: Self = RegionPermissions(0b11);

    // All known bits in our permissions, we reject anything
    // not in these known bits.
    const KNOWN: u64 = 0b11;

    /// This interprets the 64 bits provided as a
    /// `Perms` and returns the `Perms` if permitted (as
    /// in the bits are all valid known permission bits)
    ///
    /// Otherwise errors with a `DaedalusCapErrors`.
    pub fn from_bits(bits: u64) -> Result<Self, DaedalusCapErrors> {
        if bits & !Self::KNOWN != 0 {
            return Err(DaedalusCapErrors::InvalidRegionPermission(bits));
        }

        Ok(RegionPermissions(bits as u8))
    }

    /// Checks whether or not some `Perms` is
    /// a subset of another sets of `Perms`.
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// The kind of memory held
/// in this region.
///
/// Whether or not essentially its normal free
/// memory or device MIMO memory
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
pub enum MemKind {
    /// This memory belongs to normal
    /// RAM/free memory used for storing things
    /// rather than MIMO/Device memory
    Memory,

    /// This is memory belonging to some MIMO
    /// device registers (such as on AARCH64)
    Device,
}

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
    kind: MemKind,
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
        kind: MemKind,
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
        if width != 0 && (self.base + offset) % width != 0 {
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

impl Display for RegionPermissions {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let r = if self.contains(Self::R) {
            "READ"
        } else {
            "----"
        };
        let w = if self.contains(Self::W) {
            "WRITE"
        } else {
            "-----"
        };
        write!(f, "{} {}", r, w)
    }
}

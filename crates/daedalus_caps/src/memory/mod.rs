//! This module holds all of the memory
//! access related capabilities for all archs

use lepton3::lepton_vm::values::Tag;

use crate::errors::DaedalusCapErrors;

/// A unique memory region's tag handle which is
/// associated with some region
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
pub struct RegionHandle(pub Tag);

/// Permissions holdable by a region
///
/// R = bit 1
/// W = bit 2
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Perms(u8);

impl Perms {
    /// NO permissions are applicable in this region.
    /// This region has no operations that can be done upon it.
    pub const NONE: Self = Perms(0b00);

    /// This region is only readable from
    pub const R: Self = Perms(0b01);

    /// This region is only writable to
    pub const W: Self = Perms(0b10);

    /// This region can be written to/read from
    pub const RW: Self = Perms(0b11);

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

        Ok(Perms(bits as u8))
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
    perms: Perms,

    /// The kind of memory belonging to this region
    kind: MemKind,
}

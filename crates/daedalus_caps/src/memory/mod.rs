//! This module holds all of the memory
//! access related capabilities for all archs

use daedalus_program::{Grant, GrantBase, GrantLen, RegionMemKind, RegionPermissions};
use hashbrown::HashMap;
use lepton3::lepton_vm::{tagger::TagGenerator, values::Tag};

use crate::errors::DaedalusCapErrors;

unsafe extern "C" {
    /// The starting point of daedalus in memory,
    /// this should contain the stack, heap etc.
    static __daedalus_start: u8;

    /// The ending point of daedalus in memory.
    static __daedalus_end: u8;

    /// The ending point of memory
    static __dram_end: u8;
}

/// Generic abstraction layer trait
/// for architecture specific elements
///
/// if an arch impls this trait, they impl
/// all the `Daedalus` memory ops.
pub mod arch;

/// The architecture this build targets for memory operations.
///
/// Generally one is required for compilation
#[cfg(target_arch = "aarch64")]
pub type Arch = aarch64::Aarch64;

#[cfg(target_arch = "aarch64")]
pub mod aarch64;

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
    pub base: usize,

    /// The size of the memory region in bytes
    pub len: usize,

    /// The access permissions to grant to this program
    /// under this region
    pub perms: RegionPermissions,

    /// The kind of memory belonging to this region
    pub kind: RegionMemKind,
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
    ///
    /// This does not check alignment of `width`, see `resolve`
    /// for that.
    pub fn resolve_without_checking_alignment(
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

        Ok((self.base + offset) as *mut u8)
    }

    /// Checks whether or not some access at some `offset` with
    /// some `width` and some `need` permissions `Perms` can be
    /// validly done within this `Region`
    ///
    /// This also checks for alignment of the `width` based on the `offset`
    /// and the `base` of the region.
    pub fn resolve(
        &self,
        offset: usize,
        width: usize,
        need: RegionPermissions,
    ) -> Result<*mut u8, DaedalusCapErrors> {
        // Ensure the address is aligned at this width we are
        // accessing at (grrr)
        if width != 0 && !(self.base + offset).is_multiple_of(width) {
            return Err(DaedalusCapErrors::Misaligned {
                address: self.base + offset,
                width,
            });
        }

        self.resolve_without_checking_alignment(offset, width, need)
    }

    /// Creates a sub-region of this region with the requirements being:
    ///     - Must be smaller than or equal to in size to this region's
    ///     - Permissions must be less than or equal to this region's
    ///
    /// All `kind`'s are inherited by the sub-region. (as its assumed a subset
    /// of that kind remains the same kind.)
    pub fn derive(
        &self,
        offset: usize,
        len: usize,
        perms: RegionPermissions,
    ) -> Result<Region, DaedalusCapErrors> {
        // Ensure the permissions are at least a subset
        if !self.perms.contains(perms) {
            return Err(DaedalusCapErrors::PermissionDenied {
                need: perms,
                held: self.perms,
            });
        }

        // Ensure the size is at least a subset.
        let end = offset
            .checked_add(len)
            .ok_or(DaedalusCapErrors::AccessOverflow { offset, width: len })?;

        if end > self.len {
            return Err(DaedalusCapErrors::OutOfRegion {
                offset,
                width: len,
                len: self.len,
            });
        }

        Region::new(self.base + offset, len, perms, self.kind)
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

/// Resolves a manifest `Grant` into a `Region`
///
/// This can only be done at runtime as the concrete start/end
/// of daedalus cannot be known.
///
/// This will ensure that either the `Region` does not `W` overlap, otherwise
/// erroring if it does.
pub fn resolve_grant(grant: &Grant) -> Result<Region, DaedalusCapErrors> {
    let daedalus_start = (&raw const __daedalus_start) as usize;
    let daedalus_end = (&raw const __daedalus_end) as usize;
    let dram_end = (&raw const __dram_end) as usize;

    // Get the start position of the grant.
    let base = match grant.base {
        GrantBase::Absolute(base) => base,
        GrantBase::AfterDaedalus => daedalus_end,
    };

    // Get the length of the grant
    let len =
        match grant.len {
            GrantLen::Bytes(len) => len,

            // From `base` up to where Daedalus begins.
            GrantLen::ToStartDaedalus => daedalus_start.checked_sub(base).ok_or(
                DaedalusCapErrors::GrantBaseAboveDaedalus {
                    role: grant.role,
                    base,
                },
            )?,

            // From `base` up to `__dram_end`
            GrantLen::ToEndOfMemory => {
                dram_end
                    .checked_sub(base)
                    .ok_or(DaedalusCapErrors::GrantBaseOutsideMemory {
                        role: grant.role,
                        base,
                    })?
            }
        };

    Region::new(base, len, grant.perms, grant.kind)
}

/// The return result of `mint_grants`, this is
/// the set of new freshly minted regions from
/// a set of grants.
pub struct MintedGrantRegions {
    pub regions: HashMap<RegionHandle, Region>,
    pub named_grants: HashMap<&'static str, RegionHandle>,
}

/// Mints the region handles for a program's static grants.
///
/// This will set up all of the corresponding minted regions
/// and their corresponding handles with a name map for a new
/// program.
pub fn mint_grants<T: TagGenerator>(
    grants: &'static [Grant],
    tagger: &mut T,
) -> Result<MintedGrantRegions, DaedalusCapErrors> {
    let mut regions = HashMap::new();
    let mut named_grants = HashMap::new();

    for grant in grants {
        let region = resolve_grant(grant)?;

        // Create a new handle for this grant's region and insert
        // into our two maps.
        let handle = RegionHandle(tagger.allocate_tag());
        regions.insert(handle, region);
        named_grants.insert(grant.role, handle);
    }

    Ok(MintedGrantRegions {
        regions,
        named_grants,
    })
}

/// The actual capabilities themselves which provide memory
/// access functionality
pub mod capabilities;

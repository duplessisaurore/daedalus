//! This file contains all of the memory
//! related capabilities that have underlying
//! architecture-specific operations.

use core::error::Error;

use alloc::{boxed::Box, string::ToString, vec::Vec};
use daedalus_program::{RegionMemKind, RegionPermissions};
use lepton3::lepton_vm::{
    heap_allocator::{HeapAllocator, HeapItem},
    tagger::TagGenerator,
    values::Value,
    virtual_machine::value_type_name,
};

use crate::{
    errors::DaedalusCapErrors,
    ipc::capabilities::DaedalusVm,
    memory::{
        Arch, Region, RegionHandle,
        arch::{AccessWidth, MemoryArch},
    },
};

/// Pops a region handle without resolving it.
///
/// This will validate that it's a tag at least
/// else a RegionHandleExpected error will be thrown
fn pop_region_handle<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<RegionHandle, DaedalusCapErrors> {
    let value = virtual_machine
        .stack
        .pop()
        .ok_or(DaedalusCapErrors::StackUnderflowExpectedRegionHandle)?;

    match value {
        Value::Tag(tag) => Ok(RegionHandle(tag)),
        other => Err(DaedalusCapErrors::RegionHandleExpected {
            found_type: value_type_name(&other),
        }),
    }
}
/// Pops a `RegionHandle` and resolves it in the current programs'
/// region table.
///
/// This will either return the `Region` if it can be found or
/// an error if it could not be found.
fn pop_region<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<Region, DaedalusCapErrors> {
    let handle = pop_region_handle(virtual_machine)?;
    let state = &virtual_machine.capability_state;

    if let Some(region) = state.regions.get(&handle) {
        return Ok(*region);
    }

    Err(DaedalusCapErrors::UnknownRegionHandle(handle))
}

/// Pops an access/derive "length" for a region.
///
/// Errors if none could be found, ensures its within "usize"
fn pop_region_length<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<usize, DaedalusCapErrors> {
    let value = virtual_machine
        .stack
        .pop()
        .ok_or(DaedalusCapErrors::StackUnderflowExpectedRegionAccessDeriveLength)?;

    match value {
        Value::UInt(length_u64) => {
            Ok(
                usize::try_from(length_u64).map_err(|_| DaedalusCapErrors::LengthTooLarge {
                    illegal_length: length_u64,
                })?,
            )
        }
        other => Err(DaedalusCapErrors::RegionAccessDeriveLengthExpected {
            found_type: value_type_name(&other),
        }),
    }
}

/// Pops an access/derive "offset" for a region.
///
/// Errors if none could be found, ensures its within "usize"
fn pop_region_offset<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<usize, DaedalusCapErrors> {
    let value = virtual_machine
        .stack
        .pop()
        .ok_or(DaedalusCapErrors::StackUnderflowExpectedRegionAccessDeriveOffset)?;

    match value {
        Value::UInt(offset_u64) => {
            Ok(
                usize::try_from(offset_u64).map_err(|_| DaedalusCapErrors::OffsetTooLarge {
                    illegal_offset: offset_u64,
                })?,
            )
        }
        other => Err(DaedalusCapErrors::RegionAccessDeriveOffsetExpected {
            found_type: value_type_name(&other),
        }),
    }
}

/// Pops an access/derive "access width" for a region.
///
/// Errors if none could be found, ensures its within "AccessWidth"
fn pop_access_width<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<AccessWidth, DaedalusCapErrors> {
    let value = virtual_machine
        .stack
        .pop()
        .ok_or(DaedalusCapErrors::StackUnderflowExpectedAccessWidth)?;

    match value {
        Value::UInt(access_width_bytes) => Ok(AccessWidth::from_bytes(
            usize::try_from(access_width_bytes).map_err(|_| {
                DaedalusCapErrors::AccessWidthTooLarge {
                    illegal_access_width: access_width_bytes,
                }
            })?,
        )?),
        other => Err(DaedalusCapErrors::AccessWidthExpected {
            found_type: value_type_name(&other),
        }),
    }
}

/// = `mem_grant`
///
/// Returns the handle for a region in the current program's grants with a name
/// from the `role`.
///
/// This takes the `role` as a `Boson3` string (UInt array):
///
///     [<top> `role`]
///
/// And outputs a region handle:
///
///     [<top> `region`]
///
/// This is the main entry point for recieving regions at the beginning of
/// the entire boot flow.
pub fn cap_mem_grant<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    let name_value = virtual_machine
        .stack
        .pop()
        .ok_or(DaedalusCapErrors::StackUnderflowExpectedGrantRole)?;

    // A string is always an array of UInt's
    let Value::Array(index) = name_value else {
        return Err(DaedalusCapErrors::GrantRoleExpected)?;
    };

    let HeapItem::Array(fields) = virtual_machine.heap.get_item(index) else {
        return Err(DaedalusCapErrors::GrantRoleExpected)?;
    };

    // Collect all the string bytes and validate them as a utf-8 str
    let mut bytes = Vec::with_capacity(fields.len());
    for field in fields {
        let Value::UInt(byte) = field else {
            return Err(DaedalusCapErrors::GrantRoleExpected)?;
        };

        let byte = u8::try_from(*byte).map_err(|_| DaedalusCapErrors::GrantRoleExpected)?;
        bytes.push(byte);
    }

    // Get the role to lookup the grant again from the virtual machine's named grants
    let role = core::str::from_utf8(&bytes).map_err(|_| DaedalusCapErrors::GrantRoleExpected)?;

    let handle = virtual_machine
        .capability_state
        .named_grants
        .get(role)
        .copied()
        .ok_or_else(|| DaedalusCapErrors::CouldNotFindGrantRole {
            looked_up_role: role.to_string(),
        })?;

    virtual_machine.stack.push(Value::Tag(handle.0));
    Ok(())
}

/// = `mem_derive`
///
/// This creates a sub-region of the region specified by the supplied
/// region handle to this capability.
///
/// The permissions, length, offset, etc. must all be a subset of the parent
/// regions.
///
/// The arguments to the stack should be:
///
///     [<top> `permissions`, `len`, `offset`, `reegion`]
///
/// A new subregion derived will then be produced with a new handle
/// as follows:
///
///     [<top> `region`]
///
pub fn cap_mem_derive<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    // Pop permissions value off else error
    let perms_value = virtual_machine
        .stack
        .pop()
        .ok_or(DaedalusCapErrors::StackUnderflowExpectedRegionPermissions)?;

    // Turn into a RegionPermissions or be angry.
    let Value::UInt(perms_int) = perms_value else {
        return Err(DaedalusCapErrors::RegionPermissionsExpected {
            found_type: value_type_name(&perms_value),
        })?;
    };

    // Derive subset region
    let permissions = RegionPermissions::from_bits(perms_int).map_err(DaedalusCapErrors::from)?;

    let len = pop_region_length(virtual_machine)?;
    let offset = pop_region_offset(virtual_machine)?;
    let parent = pop_region(virtual_machine)?;

    let child = parent.derive(offset, len, permissions)?;

    // Create new handle for subset region and push to stack/add to daedalus state
    let handle = RegionHandle(virtual_machine.tagger.allocate_tag());
    virtual_machine
        .capability_state
        .regions
        .insert(handle, child);

    virtual_machine.stack.push(Value::Tag(handle.0));
    Ok(())
}

/// = `mem_release`
///
/// Drops one of the current program's own regions referred
/// to by a region handle.
///
///     [<top> `region`]
///
/// This will obliterate the region from existence, not any
/// sub-region of it but the current region.
///
/// Any other handles to this region will cease to function.
///
/// This does not free a `grant` region. The region handle
/// will just be obliterated transparently.
///
/// !!! BE CAREFUL !!!
pub fn cap_mem_release<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    let handle = pop_region_handle(virtual_machine)?;
    let state = &mut virtual_machine.capability_state;

    // Find if it's a grant,
    let role = state
        .named_grants
        .iter()
        .find(|(_, granted)| **granted == handle)
        .map(|(role, _)| *role);

    // Grants are not removed, but the process occurs transparently
    if role.is_some() {
        return Ok(());
    }

    // Grrr goodbye region
    if state.regions.remove(&handle).is_none() {
        return Err(DaedalusCapErrors::UnknownRegionHandle(handle).into());
    }

    Ok(())
}

/// = `mem_base`
///
/// This looks at the region referred to by a region handle:
///
///     [<top> `region`]
///
/// And pops the handle and pushes the base of the region onto
/// the stack as follows:
///
///     [<top> `base`]
///
/// This is a `UInt` value.
pub fn cap_mem_base<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    let region = pop_region(virtual_machine)?;
    virtual_machine.stack.push(Value::UInt(region.base as u64));
    Ok(())
}

/// = `mem_base`
///
/// This looks at the region referred to by a region handle:
///
///     [<top> `region`]
///
/// And pops the handle and pushes the length of the region onto
/// the stack as follows:
///
///     [<top> `length`]
///
/// This is a `UInt` value.
pub fn cap_mem_len<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    let region = pop_region(virtual_machine)?;
    virtual_machine.stack.push(Value::UInt(region.len as u64));
    Ok(())
}

/// = `mem_read`
///
/// Reads an aligned value outside of some `offset` in a `region`
/// with some `width`.
///
/// The stack should be as follows:
///
///     [<top> `width`, `offset`, `region`]
///
/// On a successful read, the produced value will be like this
/// on the stack:
///
///     [<top> `value`]
///
/// `width` must be either `1`, `2`, `4`, `8` and the region must
/// at least carry the `R` permission.
///
/// The value produced is pushed as a `UInt`.
///
/// The read is a single volatile access.
///
/// For programs sharing a writeable region, they should `mem_flush` as
/// there's no synchronisation that is enforced by this cap.
pub fn cap_mem_read<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    let width = pop_access_width(virtual_machine)?;
    let offset = pop_region_offset(virtual_machine)?;
    let region = pop_region(virtual_machine)?;

    // This ensures the permissions are met, the offset is aligned
    // with the width and everything is met !
    let pointer = region.resolve(offset, width.bytes(), RegionPermissions::R)?;

    // SAFETY:
    //
    // This read is already validated in terms of alignment and permissions
    // within the region system by the above `resolve`.
    let value = unsafe { Arch::read(pointer, width) };

    virtual_machine.stack.push(Value::UInt(value));
    Ok(())
}

/// = `mem_write`
///
/// Writes an aligned value at some `offset` in a `region`
/// with some `width`.
///
/// The stack should be as follows:
///
///      [<top> `value`, `width`, `offset`, `region`]
///
/// On a successful write, the produced value will be nothing
/// on the stack, and all values will be consumed.
///
/// `width` must be either `1`, `2`, `4`, `8` and the region must
/// at least carry the `W` permission.
///
/// The value written should be a `UInt`, and must fit in `width` bytes.
///
/// Please check if a `mem_flush` is required following this write :D
pub fn cap_mem_write<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    // Grab the UInt value we are writing out
    let value = virtual_machine
        .stack
        .pop()
        .ok_or(DaedalusCapErrors::StackUnderflowExpectedWriteValue)?;

    let Value::UInt(uint_write_out_value) = value else {
        return Err(DaedalusCapErrors::WriteValueExpected {
            found_type: value_type_name(&value),
        })?;
    };

    let width = pop_access_width(virtual_machine)?;
    let offset = pop_region_offset(virtual_machine)?;
    let region = pop_region(virtual_machine)?;

    // Reject the value if its greater than the max value of the width.
    if uint_write_out_value > width.max_value() {
        return Err(DaedalusCapErrors::ValueTooWideForAccess {
            value: uint_write_out_value,
            width: width.bytes(),
        }
        .into());
    }

    // Validate the permissions/access for this offset in the region.
    let pointer = region.resolve(offset, width.bytes(), RegionPermissions::W)?;

    // SAFETY:
    //
    // This write is already validated in terms of alignment and permissions
    // within the region system by the above `resolve`.
    unsafe { Arch::write(pointer, width, uint_write_out_value) };

    Ok(())
}

/// = `is_region_handle`
///
/// This capability consumes the tag at the top of the stack as follows:
///
///     [<top> `region`]
///
/// If this tag refers to a `Region` as a `RegionHandle` then this will push
/// `true`, otherwise, false.
///
/// The output will be as follows:
///
///     [<top> <is_region_handle>]
///
pub fn cap_is_region_handle<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    let handle = pop_region_handle(virtual_machine)?;
    let held = virtual_machine
        .capability_state
        .regions
        .contains_key(&handle);

    virtual_machine.stack.push(Value::Bool(held));
    Ok(())
}

/// = `mem_copy`
///
/// Copies a range of bytes from one region to another region.
///
/// The arguments on the stack should be:
///
///     [<top> `len`, `src_offset`, `src_region`, `dst_offset`, `dst_region`]
///
/// The source region must have the `R` and the destination `W` permissions wise.
///
/// The `src_offset` and `dst_offset` with `len` must be valid within both
/// regions and they both must be normal `Memory`.
///
/// Overlap between the two ranges is permitted. when `destination` > `source`
/// the bytes are copied backwards, otherwise forwards.
pub fn cap_mem_copy<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    // Pop arguments off
    let len = pop_region_length(virtual_machine)?;
    let source_offset = pop_region_offset(virtual_machine)?;
    let source = pop_region(virtual_machine)?;
    let destination_offset = pop_region_offset(virtual_machine)?;
    let destination = pop_region(virtual_machine)?;

    // We require `Memory` as per the header comment.
    if source.kind == RegionMemKind::Memory || destination.kind == RegionMemKind::Memory {
        Err(DaedalusCapErrors::BlockOperationOnNonMemoryRegion)?;
    }

    // Validate both ranges by checking without alignment since we don't require that for these
    // non-mimo guys :)
    let source_pointer =
        source.resolve_without_checking_alignment(source_offset, len, RegionPermissions::R)?;
    let destination_pointer = destination.resolve_without_checking_alignment(
        destination_offset,
        len,
        RegionPermissions::W,
    )?;

    if len == 0 {
        return Ok(());
    }

    // SAFETY:
    //
    // These writes have already been validated in terms permissions
    // and ranges within the region system by the above `resolve_without_alignment`s.
    unsafe { Arch::copy(source_pointer, destination_pointer, len) };

    Ok(())
}

/// = `mem_fill`
///
/// Fills a byte range of a region with one repeated byte `Value`.
///
/// The arguments to the stack should be:
///
///     [<top> `len`, `value`, `offset`, `region`]
///
/// The region must be `W` and a `Memory`-kind region. The `offset` and
/// `len` access should be within the rangeo f the `region`.
pub fn cap_mem_fill<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    let len = pop_region_length(virtual_machine)?;

    // The fill value as a byte.
    let value = virtual_machine
        .stack
        .pop()
        .ok_or(DaedalusCapErrors::StackUnderflowExpectedWriteValue)?;

    let Value::UInt(fill_value) = value else {
        return Err(DaedalusCapErrors::WriteValueExpected {
            found_type: value_type_name(&value),
        })?;
    };

    if fill_value > u64::from(u8::MAX) {
        Err(DaedalusCapErrors::ValueTooWideForAccess {
            value: fill_value,
            width: 1,
        })?;
    }

    let offset = pop_region_offset(virtual_machine)?;

    // Get the region and ensure it's a `Memory`-kind region
    let region = pop_region(virtual_machine)?;

    if region.kind == RegionMemKind::Memory {
        Err(DaedalusCapErrors::BlockOperationOnNonMemoryRegion)?;
    }

    // Check the perms/len in region
    let pointer = region.resolve_without_checking_alignment(offset, len, RegionPermissions::W)?;

    if len == 0 {
        return Ok(());
    }

    // SAFETY:
    //
    // These writes have already been validated in terms permissions
    // and ranges within the region system by the above `resolve_without_alignment`s.
    unsafe { Arch::fill(pointer, fill_value as u8, len) };

    Ok(())
}

//! This file contains all of the memory
//! related capabilities that have underlying
//! architecture-specific operations.

use core::error::Error;

use alloc::{boxed::Box, string::ToString, vec::Vec};
use lepton3::lepton_vm::{
    heap_allocator::{HeapAllocator, HeapItem},
    tagger::TagGenerator,
    values::Value,
};

use crate::{errors::DaedalusCapErrors, ipc::capabilities::DaedalusVm};

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

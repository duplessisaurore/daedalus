//! This file contains all of the IRQ
//! related capabilities that have underlying
//! architecture-specific operations.

use core::error::Error;

use alloc::boxed::Box;
use lepton3::lepton_vm::{
    heap_allocator::HeapAllocator, tagger::TagGenerator, values::Value,
    virtual_machine::value_type_name,
};

use crate::{
    errors::DaedalusCapErrors,
    ipc::capabilities::DaedalusVm,
    irq::{IrqBinding, IrqHandle},
};

/// Wrapped IRQ information that
/// is used everywhere lol, cleaner than tuples !! imo
#[derive(Clone, Copy, Debug)]
struct IrqInfo {
    /// The id of the interrupt this IRQ refers to
    interrupt_id: u32,

    /// The active binding to this interrupt id.
    binding: IrqBinding,
}

/// Pops an IrqInfo from the stack.
///
/// This will validate that the IrqHandle is actively bound
/// to the current program and valid and return the corresponding
/// live binding and interrupt number as an IrqInfo
fn pop_irq_info<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<IrqInfo, DaedalusCapErrors> {
    // same stuff as `cap_is_irq_handle`.
    let value = virtual_machine
        .stack
        .pop()
        .ok_or(DaedalusCapErrors::StackUnderflowExpectedIrqHandle)?;

    let Value::Tag(tag) = value else {
        return Err(DaedalusCapErrors::IrqHandleExpected {
            found_type: value_type_name(&value),
        });
    };

    let irq_handle = IrqHandle(tag);
    let current = virtual_machine.capability_state.current_program;

    // Find corresponding binding in irq bindings state
    // that matches the program !
    virtual_machine
        .capability_state
        .irqs
        .iter()
        .find(|(_, binding)| binding.irq_handle == irq_handle && binding.program == current)
        .map(|(interrupt_id, binding)| IrqInfo {
            interrupt_id: *interrupt_id,
            binding: *binding,
        })
        .ok_or(DaedalusCapErrors::UnknownIrqHandle(irq_handle))
}

/// = `is_irq_handle`
///
/// This capability consumes the tag at the top of the stack as follows:
///
///     [<top> `irq`]
///
/// If this tag refers to an `IrqBinding` as an `IrqHandle` then this will push
/// `true`, otherwise, false. This must be a currently-live registration.
///
/// The output will be as follows:
///
///     [<top> <is_irq_handle>]
///
pub fn cap_is_irq_handle<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    let value = virtual_machine
        .stack
        .pop()
        .ok_or(DaedalusCapErrors::StackUnderflowExpectedIrqHandle)?;

    // Make sure it's a tag
    let Value::Tag(tag) = value else {
        return Err(DaedalusCapErrors::IrqHandleExpected {
            found_type: value_type_name(&value),
        })?;
    };

    let irq_handle = IrqHandle(tag);
    let current = virtual_machine.capability_state.current_program;

    // Check that the irq binding exists and binds to the current program.
    let held = virtual_machine
        .capability_state
        .irqs
        .values()
        .any(|binding| binding.irq_handle == irq_handle && binding.program == current);

    virtual_machine.stack.push(Value::Bool(held));
    Ok(())
}

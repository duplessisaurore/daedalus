//! This file contains all of the IRQ
//! related capabilities that have underlying
//! architecture-specific operations.

use core::error::Error;

use alloc::boxed::Box;
use daedalus_program::find_program_interrupt_under_id;
use hashbrown::hash_map::Entry;
use lepton3::lepton_vm::{
    heap_allocator::HeapAllocator, tagger::TagGenerator, values::Value,
    virtual_machine::value_type_name,
};

use crate::{
    errors::DaedalusCapErrors,
    ipc::capabilities::DaedalusVm,
    irq::{IrqBinding, IrqHandle, arch::IrqArch, archs::TargetIRQArch},
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

/// = `irq_release`
///
/// Drops one of the current program's own IRQ registrations referred to
/// to by an IRQ handle.
///
///     [<top> `irq`]
///
/// The line will be masked and the registration removed.
///
/// This will permit another program to register with this IRQ, and
/// the current program can re-register at any time.
///
/// Any other handles to this binding will cease to function.
pub fn cap_irq_release<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    // Get the irq handle
    let IrqInfo {
        interrupt_id,
        binding: _,
    } = pop_irq_info(virtual_machine)?;

    // Mask the interrupt,
    //
    // # Safety
    //
    // we know the binding must exist
    // due to `pop_irq_info` so this is safe
    unsafe {
        TargetIRQArch::mask(interrupt_id);
    }

    // Remove from the total irq bindings.
    virtual_machine.capability_state.irqs.remove(&interrupt_id);
    Ok(())
}

/// = `irq_ack`
///
/// Unmasks an interrupt so that it may activate again.
///
///     [<top> `irq`]
///
/// This does not do anything other than unmasking, it is up to
/// the program to properly clear the activation condition or
/// whatever for this interrupt, depending on what the program wants.
pub fn cap_irq_ack<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    // Get the irq handle
    let IrqInfo {
        interrupt_id,
        binding: _,
    } = pop_irq_info(virtual_machine)?;

    // Unmask the interrupt,
    //
    // # Safety
    //
    // we know the binding must exist
    // due to `pop_irq_info` so this is safe
    unsafe { TargetIRQArch::unmask(interrupt_id) };

    Ok(())
}

/// = `irq_register`
///
/// Registers and binds a hardware interrupt with some ID to the current program.
///
/// This takes the `interrupt id` as an UInt:
///
///     [<top> `interrupt_id`]
///
/// The interrupt id must be granted to the current program from its manifest, and
/// there must be no other registrations bound to this interrupt id currently active.
///
/// This outputs the corresponding binding as an IRQ handle which is used to
/// refer to it in following capabilities, and inbox messages delivered on the interrupt
/// activation:
///
///     [<top> `irq`]
///
/// The configuration of the interrupt, that is it's trigger type, priority level etc.
/// are all done in the manifest statically.
pub fn cap_irq_register<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    // Get the interrupt id we are trying to bind to
    let value = virtual_machine
        .stack
        .pop()
        .ok_or(DaedalusCapErrors::StackUnderflowExpectedInterruptId)?;

    let Value::UInt(raw) = value else {
        return Err(DaedalusCapErrors::InterruptIdExpected {
            found_type: value_type_name(&value),
        })?;
    };

    // Check if this is actually a valid interrupt id under the target irq arch
    let interrupt_id = u32::try_from(raw)
        .ok()
        .filter(|id| TargetIRQArch::is_valid_irq_wrapper(*id))
        .ok_or(DaedalusCapErrors::InvalidInterruptId {
            raw_interrupt_id: raw,
        })?;

    let current = virtual_machine.capability_state.current_program;

    // Get the declaration for the interrupt under this program
    let declaration = find_program_interrupt_under_id(current, interrupt_id).ok_or(
        DaedalusCapErrors::InterruptNotDeclared {
            interrupt_id,
            program: current,
        },
    )?;

    // Already bound elsewhere?
    match virtual_machine.capability_state.irqs.entry(interrupt_id) {
        // Uh oh, already bound
        Entry::Occupied(occupied_entry) => Err(DaedalusCapErrors::InterruptAlreadyRegistered {
            interrupt_id,
            original_program: occupied_entry.get().program,
            new_program: current,
        })?,

        // We can fill it in as its not bound
        Entry::Vacant(_) => {}
    }

    // Make a new irq handle for this binding.
    let irq_handle = IrqHandle(virtual_machine.tagger.allocate_tag());

    virtual_machine.capability_state.irqs.insert(
        interrupt_id,
        IrqBinding {
            program: current,
            irq_handle,
        },
    );

    // # Safety
    //
    // We know the id is valid for this controller, and any level of priority/trigger
    // should be valid regardless.
    //
    // Setup ran on start of `Daedalus`.
    unsafe {
        TargetIRQArch::configure(interrupt_id, declaration.trigger, declaration.priority);
        TargetIRQArch::unmask(interrupt_id);
    }

    virtual_machine.stack.push(Value::Tag(irq_handle.0));
    Ok(())
}

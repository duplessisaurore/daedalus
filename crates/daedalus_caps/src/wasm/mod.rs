//! This module holds all of the wasm
//! access related capabilities

use core::error::Error;

use alloc::{boxed::Box, vec::Vec};
use daedalus_wasm::{InvocationRet, LiveInstance, WasmInstanceHandle};
use lepton3::lepton_vm::{
    heap_allocator::{HeapAllocator, HeapItem},
    tagger::TagGenerator,
    values::Value,
    virtual_machine::value_type_name,
};

use crate::{ipc::capabilities::DaedalusVm, wasm::errors::DaedalusWasmCapErrors};

/// Errors that can occur
pub mod errors;

/// Pops a `WasmInstanceTag` and resolves it in the current programs'
/// instances
///
/// This will either return the `LiveInstance` if it can be found or
/// an error if it could not be found.
fn pop_live_instance<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<&mut LiveInstance, Box<dyn Error>> {
    // Should be an instance handle here
    let value = virtual_machine
        .pop_maybe()
        .ok_or(DaedalusWasmCapErrors::StackUnderflowExpectedInstanceHandle)?;

    let Value::Tag(instance_tag) = value else {
        return Err(DaedalusWasmCapErrors::InstanceHandleExpected {
            found_type: value_type_name(&value),
        })?;
    };

    // Get the corresponding live instance in this program
    let state = &mut virtual_machine.capability_state;
    let handle = WasmInstanceHandle(instance_tag);

    if let Some(live_instance) = state.wasm_instances.get_mut(&handle) {
        return Ok(live_instance);
    }

    Err(DaedalusWasmCapErrors::UnknownInstanceHandle(handle))?
}

/// = `wasm_open`
///
/// Opens a new instance of a module under this
/// current program,
///
/// This takes the `name` of the module as a `Boson3` string
/// (UInt array):
///
///     [<top> `name`]
///
/// The program should have a grant to this wasm module,
/// this will output a handle to the program on success
///
///     [<top> `instance`]
///
pub fn cap_wasm_open<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    let name_value = virtual_machine
        .pop_maybe()
        .ok_or(DaedalusWasmCapErrors::StackUnderflowExpectedWasmModuleName)?;

    // A string is always an array of UInt's
    let Value::Array(index) = name_value else {
        return Err(DaedalusWasmCapErrors::WasmModuleNameExpected)?;
    };

    let HeapItem::Array(fields) = virtual_machine.heap.get_item(index) else {
        return Err(DaedalusWasmCapErrors::WasmModuleNameExpected)?;
    };

    // Collect all the string bytes and validate them as a utf-8 str
    let mut bytes = Vec::with_capacity(fields.len());
    for field in fields {
        let Value::UInt(byte) = field else {
            return Err(DaedalusWasmCapErrors::WasmModuleNameExpected)?;
        };

        let byte =
            u8::try_from(*byte).map_err(|_| DaedalusWasmCapErrors::WasmModuleNameExpected)?;
        bytes.push(byte);
    }

    // Get the name of the module..
    let module_name =
        core::str::from_utf8(&bytes).map_err(|_| DaedalusWasmCapErrors::WasmModuleNameExpected)?;

    let instance = virtual_machine.capability_state.wasm_manager.open(
        virtual_machine.capability_state.current_program,
        module_name,
        &mut virtual_machine.tagger,
    )?;

    // Push the tag of our instance
    virtual_machine
        .stack
        .push(Value::Tag(instance.instance_tag.0));

    // Insert into the current instances for this program
    virtual_machine
        .capability_state
        .wasm_instances
        .insert(instance.instance_tag, instance);

    Ok(())
}

/// = `wasm_call`
///
/// Calls the specific call handler on this wasm module.
///
/// Takes the following inputs:
///
///     [<top> output_capacity, input_bytes, operation_uint, abort, instance]
///
/// `output_capacity` is the expected output size for calling this operation on the module,
/// the module must match this, otherwise an error.
///
/// input are the inputs to send to the program, these should be in a byte-representation
/// UInt format in an array, where each UInt is some buyr
///
/// The `operation_uint` is a special operation identifier that can be used
/// to identify which operation to call easily on the module,
///
/// The `instance` should be the instance tag returned by `wasm_open`
///
/// The `abort` should be a boolean value as to where if the module fails,
/// execution of the program should abort rather than returning a status code.
///
/// Returns:
///     
///     [<top> status, output_bytes]
///
/// Where `status` is the status code returned by the call handler, and output
/// bytes are the byte-representation UInt form of the output from the program.
///
/// Generally status will be >= 0 on success, with the status representing how many
/// output bytes on success. On failure it will represent some code negative.
///
pub fn cap_wasm_call<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    let value = virtual_machine
        .pop_maybe()
        .ok_or(DaedalusWasmCapErrors::StackUnderflowExpectedOutputCapacity)?;

    // The capacity of the output
    let Value::UInt(output_capacity_u64) = value else {
        return Err(DaedalusWasmCapErrors::OutputCapacityExpected {
            found_type: value_type_name(&value),
        })?;
    };

    let output_capacity = usize::try_from(output_capacity_u64).map_err(|_| DaedalusWasmCapErrors::OutputCapacityExpected {
        found_type: "got an operation value greater than word size bits! this is not permitted it must be within the range of <word size bits"
    })?;

    // Input bytes
    let value = virtual_machine
        .pop_maybe()
        .ok_or(DaedalusWasmCapErrors::StackUnderflowExpectedInput)?;

    let Value::Array(input_idx) = value else {
        return Err(DaedalusWasmCapErrors::CallInputExpected {
            reason: "got a non-array type input to input bytes",
        })?;
    };

    let HeapItem::Array(values) = virtual_machine.heap.get_item(input_idx) else {
        return Err(DaedalusWasmCapErrors::CallInputExpected {
            reason: "got an invalid array heap reference",
        })?;
    };

    // Transform from UInt-array into byte array for wasm input
    let input = values
        .iter()
        .map(|item| match item {
            Value::UInt(b) => {
                u8::try_from(*b).map_err(|_| DaedalusWasmCapErrors::CallInputExpected {
                    reason: "got a UInt byte-value in input byte array that exceeds MAX_U8",
                })
            }
            _ => Err(DaedalusWasmCapErrors::CallInputExpected {
                reason: "got an non-UInt byte-value in input byte array",
            }),
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Operation UInt
    let value = virtual_machine
        .pop_maybe()
        .ok_or(DaedalusWasmCapErrors::StackUnderflowExpectedOperationUInt)?;

    let Value::UInt(operation_u64) = value else {
        return Err(DaedalusWasmCapErrors::OperationUIntExpected {
            found_type: value_type_name(&value),
        })?;
    };

    let operation = u32::try_from(operation_u64).map_err(|_| DaedalusWasmCapErrors::OperationUIntExpected {
        found_type: "got an operation value greater than 32 bits! this is not permitted it must be within the range of <32 bits"
    })?;

    // Abort value
    let value = virtual_machine
        .pop_maybe()
        .ok_or(DaedalusWasmCapErrors::StackUnderflowExpectedAbort)?;

    let Value::Bool(abort) = value else {
        return Err(DaedalusWasmCapErrors::AbortExpected {
            found_type: value_type_name(&value),
        })?;
    };

    // Instance tag
    let instance = pop_live_instance(virtual_machine)?;

    // Invoke the actual instance
    let call_outcome = instance.invoke(operation, &input, output_capacity);
    let ret = match call_outcome {
        Ok(ret) => ret,
        Err(err) => {
            // If we aren't aborting, turn the wasm module error into a normal ret
            if abort {
                return Err(Box::new(err));
            }

            InvocationRet {
                output: Vec::new(),
                status: err.status_code(),
            }
        }
    };

    // Output into the program
    virtual_machine.gc_collect();

    // Turn output value back into bytes
    let output_bytes = ret
        .output
        .iter()
        .map(|&byte| Value::UInt(u64::from(byte)))
        .collect();

    let output_arr = Value::Array(
        virtual_machine
            .heap
            .alloc_raw(HeapItem::Array(output_bytes)),
    );

    virtual_machine.stack.push(output_arr);
    virtual_machine.stack.push(Value::Int(ret.status as i64));
    Ok(())
}

/// = `is_instance_handle`
///
/// This capability consumes the tag at the top of the stack as follows:
///
///     [<top> `region`]
///
/// If this tag refers to a `LiveInstance` as a `WasmInstanceHandle` then this will push
/// `true`, otherwise, false.
///
/// The output will be as follows:
///
///     [<top> <is_instance_handle>]
///
pub fn cap_is_instance_handle<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    // Maybe instance handle here, should always be a tag.
    let value = virtual_machine
        .pop_maybe()
        .ok_or(DaedalusWasmCapErrors::StackUnderflowExpectedInstanceHandle)?;

    let Value::Tag(instance_tag) = value else {
        return Err(DaedalusWasmCapErrors::InstanceHandleExpected {
            found_type: value_type_name(&value),
        })?;
    };

    // Should have a corresponding wasm instance if it is a handle
    let state = &virtual_machine.capability_state;
    let handle = WasmInstanceHandle(instance_tag);
    let is_instance = state.wasm_instances.contains_key(&handle);

    virtual_machine.stack.push(Value::Bool(is_instance));
    Ok(())
}

/// = `wasm_close`
///
/// Closes an instance of a module under this
/// current program
///
/// This takes the `instance` of the module
///
///     [<top> `instance`]
///
/// The program will then drop this instance, making
/// the handle invalid. Nothing will be pushed to the stack.
///
pub fn cap_wasm_close<H: HeapAllocator, T: TagGenerator>(
    virtual_machine: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    // Maybe instance handle here, should always be a tag.
    let value = virtual_machine
        .pop_maybe()
        .ok_or(DaedalusWasmCapErrors::StackUnderflowExpectedInstanceHandle)?;

    let Value::Tag(instance_tag) = value else {
        return Err(DaedalusWasmCapErrors::InstanceHandleExpected {
            found_type: value_type_name(&value),
        })?;
    };

    // Should have a corresponding wasm instance if it is a handle
    let state = &mut virtual_machine.capability_state;
    let handle = WasmInstanceHandle(instance_tag);

    state.wasm_instances.remove(&handle);
    Ok(())
}

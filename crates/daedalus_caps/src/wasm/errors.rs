//! These are all the possible errors that
//! can occur during the running of the `Daedalus`
//! wasm specific capabilities.

use core::{error::Error, fmt::Display};

use daedalus_wasm::WasmInstanceHandle;

#[derive(Debug)]
pub enum DaedalusWasmCapErrors {
    /// Expected to pop a `name` for a `wasm_open` call in order
    /// to instantiate a wasm module.
    StackUnderflowExpectedWasmModuleName,

    /// A wasm module name was expected heere
    WasmModuleNameExpected,

    /// Expected to pop an `output_capacity` for a `wasm_call`
    StackUnderflowExpectedOutputCapacity,

    /// A UInt output capacity was expected here
    OutputCapacityExpected { found_type: &'static str },

    /// Expected to pop an `input` for a `wasm_call`
    StackUnderflowExpectedInput,

    /// A byte-array of UInt's was expected here
    ///
    /// Something was up though!
    CallInputExpected { reason: &'static str },

    /// Expected to pop an `operation_uint` for a `wasm_call`
    StackUnderflowExpectedOperationUInt,

    /// A UInt operation code was expected here
    OperationUIntExpected { found_type: &'static str },

    /// Expected to pop an `abort` for a `wasm_call`
    StackUnderflowExpectedAbort,

    /// A Bool abort was expected here
    AbortExpected { found_type: &'static str },

    /// Expected to pop an `instance` tag for a `wasm_call`
    StackUnderflowExpectedInstanceHandle,

    /// An Instance tag of a wasm module was expected here
    InstanceHandleExpected { found_type: &'static str },

    /// An instance tag was attempted to be used however it referred
    /// to an unknown/not real instance
    UnknownInstanceHandle(WasmInstanceHandle),
}

impl Display for DaedalusWasmCapErrors {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::StackUnderflowExpectedWasmModuleName => {
                write!(
                    f,
                    "daedalus expected to find a wasm module name on the stack but nothing was found!"
                )
            }
            Self::WasmModuleNameExpected => {
                write!(
                    f,
                    "daedalus expected a wasm module name as a `Boson3` string value (UInt Array), found an invalid one!"
                )
            }
            Self::StackUnderflowExpectedOutputCapacity => {
                write!(
                    f,
                    "daedalus expected to find an output capacity for a `wasm_call` on the stack but nothing was found!"
                )
            }
            Self::OutputCapacityExpected { found_type } => {
                write!(
                    f,
                    "daedalus expected an output capacity as a UInt value for a `wasm_call`, but instead found a `{found_type}`!"
                )
            }
            Self::StackUnderflowExpectedInput => {
                write!(
                    f,
                    "daedalus expected to find input bytes for a `wasm_call` on the stack but nothing was found!"
                )
            }
            Self::CallInputExpected { reason } => {
                write!(
                    f,
                    "daedalus expected input for a `wasm_call` as a byte array of UInt values, but the provided input was invalid! reason: `{reason}`!"
                )
            }
            Self::StackUnderflowExpectedOperationUInt => {
                write!(
                    f,
                    "daedalus expected to find a wasm operation code on the stack but nothing was found!"
                )
            }
            Self::OperationUIntExpected { found_type } => {
                write!(
                    f,
                    "daedalus expected a wasm operation code as a UInt value for a `wasm_call`, but instead found a `{found_type}`!"
                )
            }
            Self::StackUnderflowExpectedAbort => {
                write!(
                    f,
                    "daedalus expected to find an abort flag for a `wasm_call` on the stack but nothing was found!"
                )
            }
            Self::AbortExpected { found_type } => {
                write!(
                    f,
                    "daedalus expected an abort flag as a Bool value for a `wasm_call`, but instead found a `{found_type}`!"
                )
            }
            Self::StackUnderflowExpectedInstanceHandle => {
                write!(
                    f,
                    "daedalus expected to find a wasm instance handle on the stack but nothing was found!"
                )
            }
            Self::InstanceHandleExpected { found_type } => {
                write!(
                    f,
                    "daedalus expected a wasm instance handle for a `wasm_call`, but instead found a `{found_type}`!"
                )
            }
            Self::UnknownInstanceHandle(handle) => {
                write!(
                    f,
                    "daedalus found an attempt to invoke a wasm operation with instance handle `{handle:?}`, but this handle does not refer to a known live wasm module instance!"
                )
            }
        }
    }
}

impl Error for DaedalusWasmCapErrors {}

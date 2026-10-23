//! This provides all the actual wasm management
//! types for all of the wasm modules.

use core::{error::Error, fmt::Display};

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

use alloc::vec;
use daedalus_program::{WasmGrant, get_program, get_wasm_module};
use hashbrown::HashMap;
use lepton3::lepton_vm::{tagger::TagGenerator, values::Tag};
use wasmi::{
    Config, Engine, Instance, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder,
    TypedFunc,
};

/// A unique tag representing some WASM module instance
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
pub struct WasmInstanceHandle(pub Tag);

/// All possible wasm errors that can occur in `Daedalus`
#[derive(Debug)]
pub enum DaedalusWasmErrors {
    /// This program was attempted to be opened
    /// but it doesn't really actually exist!
    ModuleDoesNotExist { program_name: String, name: String },

    /// Attempted to look up a program under this name,
    /// but none could be found!
    CouldNotFindProgram { program_name: String },

    /// This program attempted to open a module
    /// which it did not have a grant for
    AccessToNonGrantedModule { program_name: String, name: String },

    /// A random unexpected WASMI error which we wrap
    Unexpected(String),

    /// The wasm module does not properly pertain to the memory abi
    InvalidMemoryAbi,

    /// The wasm module does not properly pertain to the allocation abi
    InvalidAllocationAbi,

    /// The wasm module does not properly pertain to the freeing abi
    InvalidFreeAbi,

    /// The wasm module does not properly pertain to the call abi
    InvalidCallAbi,

    /// The user tried to provide an input to a wasm module that exceeded
    /// the maximum size in bytes
    InputTooLarge { module: &'static str },

    /// The user tried to require an output from a wasm module that exceeded
    /// the maximum size in bytes
    UserOutputTooLarge { module: &'static str },

    /// The module tried to require an output that exceeded
    /// the maximum allowed size from it in bytes
    ModuleOutputTooLarge { module: &'static str },

    /// Allocation of memory to copy into wasm module failed
    InAllocationFailed { module: &'static str, err: String },

    /// Allocation of memory to copy out of a wasm module failed
    OutAllocationFailed { module: &'static str, err: String },

    /// Freeing of memory to copy into wasm module failed
    InFreeFailed { module: &'static str, err: String },

    /// Freeing of memory to copy out of a wasm module failed
    OutFreeFailed { module: &'static str, err: String },

    /// Failed to write input memory into the wasm module
    FailedToWriteInput { module: &'static str, err: String },

    /// Failed to retrieve output memory from the wasm module
    FailedToRetOutput { module: &'static str, err: String },

    /// An execution error occured while running the `call` function
    /// of a wasm module
    FailedToRunCall { module: &'static str, err: String },
}

/// The global WASM manager for all wasm modules
/// in all of daedalus.
///
/// We cache `compiled` modules based on name so
/// re-compilation doesn't need to occur.
pub struct WasmManager {
    engine: Engine,
    compiled: HashMap<&'static str, CompiledModule>,
}

/// One pre-compiled `wasmi` module
struct CompiledModule {
    module: Module,
}

/// One existing live instance of a module
/// owned by a program
pub struct LiveInstance {
    /// The specific tag that identifies this instance
    /// of this wasm module, we can call it using this
    pub instance_tag: WasmInstanceHandle,

    /// The specific instance of this module
    /// which we can call with
    pub instance: Instance,

    /// The grant to this module, for limit tracking
    pub grant: &'static WasmGrant,

    /// Reference to it's memory.
    pub memory: Memory,

    /// Store of the current program
    pub store: Store<StoreLimits>,

    /// Reference to its alloc function
    pub alloc: TypedFunc<u32, u32>,

    /// Reference to its free function
    pub free: TypedFunc<(u32, u32), ()>,

    /// Reference to it's call function
    pub call: TypedFunc<(u32, u32, u32, u32, u32), i32>,
}

impl WasmManager {
    /// Creates a new `WASM` manager with its initial engine.
    ///
    /// Has no compiled modules currently.
    pub fn new() -> Self {
        let mut config = Config::default();
        config.consume_fuel(true);
        config.allow_start_fn(false);

        Self {
            engine: Engine::new(&config),
            compiled: HashMap::new(),
        }
    }

    /// Opens a new wasm module under an `owner` program using
    /// the current WASM manage.r
    ///
    /// # Errors
    ///
    /// This may error in a multitude of ways:
    /// - The owner doesn't have permission to actually open this module
    /// - The module doesn't exist
    /// - The ABI of the module is invalid/doesn't' match.
    pub fn open(
        &mut self,
        owner: &'static str,
        name: &str,
        tag_alloc: &mut impl TagGenerator,
    ) -> Result<LiveInstance, DaedalusWasmErrors> {
        // Get the actual declared wasm module
        let module =
            get_wasm_module(name).ok_or_else(|| DaedalusWasmErrors::ModuleDoesNotExist {
                name: name.to_string(),
                program_name: owner.to_string(),
            })?;

        // Get the specific wasm grant to this module for the program
        let program =
            get_program(owner).ok_or_else(|| DaedalusWasmErrors::CouldNotFindProgram {
                program_name: owner.to_string(),
            })?;

        let grant = program
            .wasm
            .iter()
            .find(|grant| grant.wasm == name)
            .ok_or_else(|| DaedalusWasmErrors::AccessToNonGrantedModule {
                name: name.to_string(),
                program_name: owner.to_string(),
            })?;

        // Try find a compiled module or compile a new one,
        // we already validated at build time.
        let compiled = match self.compiled.entry(module.name) {
            hashbrown::hash_map::Entry::Occupied(occupied_entry) => occupied_entry.into_mut(),
            hashbrown::hash_map::Entry::Vacant(vacant_entry) => {
                // SAFETY:
                //
                // We already validated the module at build time.
                unsafe {
                    let compiled_module =
                        Module::new_unchecked(&self.engine, module.validated_wasm_bytes).map_err(
                            |e| DaedalusWasmErrors::Unexpected(format!("module creation: {e:?}")),
                        )?;
                    vacant_entry.insert(CompiledModule {
                        module: compiled_module,
                    })
                }
            }
        };

        // We limit this specific instance's memory etc.
        let limits = StoreLimitsBuilder::new()
            .memory_size(grant.max_memory_bytes)
            .memories(1)
            .instances(1)
            .build();

        let mut store = Store::new(&self.engine, limits);
        store.limiter(|state| state);

        store
            .set_fuel(grant.fuel_per_call)
            .map_err(|e| DaedalusWasmErrors::Unexpected(format!("fuel setup: {e:?}")))?;

        // Create actual instance of module
        let linker = Linker::<StoreLimits>::new(&self.engine);
        let instance = linker
            .instantiate_and_start(&mut store, &compiled.module)
            .map_err(|e| DaedalusWasmErrors::Unexpected(format!("instantiate: {e:?}")))?;

        // Get our actual ABI functions for interacting with daedalus
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or(DaedalusWasmErrors::InvalidMemoryAbi)?;
        let alloc = instance
            .get_typed_func::<u32, u32>(&store, "daedalus_alloc")
            .map_err(|_| DaedalusWasmErrors::InvalidAllocationAbi)?;
        let free = instance
            .get_typed_func::<(u32, u32), ()>(&store, "daedalus_free")
            .map_err(|_| DaedalusWasmErrors::InvalidFreeAbi)?;
        let call = instance
            .get_typed_func::<(u32, u32, u32, u32, u32), i32>(&store, "daedalus_call")
            .map_err(|_| DaedalusWasmErrors::InvalidCallAbi)?;

        Ok(LiveInstance {
            instance_tag: WasmInstanceHandle(tag_alloc.allocate_tag()),
            grant,
            instance,
            memory,
            store,
            alloc,
            free,
            call,
        })
    }
}

/// The return of one invocation on a live wasm
/// module instancee
pub struct InvocationRet {
    pub output: Vec<u8>,
    pub status: i32,
}

impl LiveInstance {
    /// Invokes the `call` function on this live instance, executing the
    /// instance's specific daedalus call handler.
    ///
    /// The `operation` should be the specific operation number we are invoking,
    /// `input` is copied into the program's memory for operating on.
    ///
    /// Output capacity should be the limit of the program's output (which
    /// is returned as a Vec<u8> of bytes)
    pub fn invoke(
        &mut self,
        operation: u32,
        input: &[u8],
        output_capacity: usize,
    ) -> Result<InvocationRet, DaedalusWasmErrors> {
        // Ensure input is in the requirements
        if input.len() > self.grant.max_input_bytes || input.len() > u32::MAX as usize {
            return Err(DaedalusWasmErrors::InputTooLarge {
                module: self.grant.wasm,
            });
        }

        // Same for output
        if output_capacity > self.grant.max_output_bytes || output_capacity > u32::MAX as usize {
            return Err(DaedalusWasmErrors::UserOutputTooLarge {
                module: self.grant.wasm,
            });
        }

        // Reset fuel for this invocation.
        self.store
            .set_fuel(self.grant.fuel_per_call)
            .map_err(|e| DaedalusWasmErrors::Unexpected(format!("fuel reset: {e:?}")))?;

        // Allocate at least one byte (or else explode null ptrs)
        // for the current call's outputs/inputs
        let input_allocation = input.len().max(1) as u32;
        let output_allocation = output_capacity.max(1) as u32;

        // Allocate space for the input.
        let in_ptr = self
            .alloc
            .call(&mut self.store, input_allocation)
            .map_err(|err| DaedalusWasmErrors::InAllocationFailed {
                module: self.grant.wasm,
                err: err.to_string(),
            })?;

        if in_ptr == 0 {
            return Err(DaedalusWasmErrors::InAllocationFailed {
                module: self.grant.wasm,
                err: "guest in allocation returned null!".to_string(),
            });
        }

        // Allocate space for the output.
        let out_ptr = self
            .alloc
            .call(&mut self.store, output_allocation)
            .map_err(|err| DaedalusWasmErrors::OutAllocationFailed {
                module: self.grant.wasm,
                err: err.to_string(),
            })?;

        if out_ptr == 0 {
            return Err(DaedalusWasmErrors::OutAllocationFailed {
                module: self.grant.wasm,
                err: "guest out allocation returned null!".to_string(),
            });
        }

        // Write the input into the memory
        self.memory
            .write(&mut self.store, in_ptr as usize, input)
            .map_err(|e| DaedalusWasmErrors::FailedToWriteInput {
                module: self.grant.wasm,
                err: e.to_string(),
            })?;

        // The guest returns the number of output bytes, or a negative status,
        // hopefully didn't error.
        let result = self
            .call
            .call(
                &mut self.store,
                (
                    operation,
                    in_ptr,
                    input.len() as u32,
                    out_ptr,
                    output_capacity as u32,
                ),
            )
            .map_err(|e| DaedalusWasmErrors::FailedToRunCall {
                module: self.grant.wasm,
                err: e.to_string(),
            })?;

        // ensure output requirements are ment
        let output = if result > 0 {
            if result as usize > output_capacity {
                return Err(DaedalusWasmErrors::ModuleOutputTooLarge {
                    module: self.grant.wasm,
                });
            }

            // Read output from the module
            let mut output = vec![0; result.try_into().unwrap()];

            self.memory
                .read(&self.store, out_ptr as usize, &mut output)
                .map_err(|e| DaedalusWasmErrors::FailedToRetOutput {
                    module: self.grant.wasm,
                    err: e.to_string(),
                })?;

            output
        } else {
            Vec::new()
        };

        // Now clean up our input and output allocations
        self.free
            .call(&mut self.store, (out_ptr, output_allocation))
            .map_err(|err| DaedalusWasmErrors::OutFreeFailed {
                module: self.grant.wasm,
                err: err.to_string(),
            })?;
        self.free
            .call(&mut self.store, (in_ptr, input_allocation))
            .map_err(|err| DaedalusWasmErrors::InFreeFailed {
                module: self.grant.wasm,
                err: err.to_string(),
            })?;

        Ok(InvocationRet {
            output,
            status: result,
        })
    }
}

impl Display for DaedalusWasmErrors {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ModuleDoesNotExist { program_name, name } => {
                write!(
                    f,
                    "daedalus program `{program_name}` attempted to open wasm module `{name}`, but no wasm module with that name exists!"
                )
            }
            Self::CouldNotFindProgram { program_name } => {
                write!(
                    f,
                    "daedalus attempted to look up program `{program_name}` as owner of new wasm module, but could not find any program with that name!"
                )
            }
            Self::AccessToNonGrantedModule { program_name, name } => {
                write!(
                    f,
                    "daedalus program `{program_name}` attempted to open wasm module `{name}`, but this program has no grant to this module!"
                )
            }
            Self::Unexpected(err) => {
                write!(
                    f,
                    "daedalus encountered an unexpected error while managing a wasm module: `{err}`!"
                )
            }
            Self::InvalidMemoryAbi => {
                write!(
                    f,
                    "daedalus expected wasm module to export a valid linear memory named `memory`, but no such memory was found!"
                )
            }
            Self::InvalidAllocationAbi => {
                write!(
                    f,
                    "daedalus expected wasm module to export an allocation function `daedalus_alloc` with signature `(u32) -> u32`, but no valid function was found!"
                )
            }
            Self::InvalidFreeAbi => {
                write!(
                    f,
                    "daedalus expected wasm module to export a freeing function `daedalus_free` with signature `(u32, u32) -> ()`, but no valid function was found!"
                )
            }
            Self::InvalidCallAbi => {
                write!(
                    f,
                    "daedalus expected wasm module to export a call function `daedalus_call` with signature `(u32, u32, u32, u32, u32) -> i32`, but no valid function was found!"
                )
            }
            Self::InputTooLarge { module } => {
                write!(
                    f,
                    "daedalus attempted to invoke wasm module `{module}` with an input that exceeds the maximum permitted input size for this module!"
                )
            }
            Self::UserOutputTooLarge { module } => {
                write!(
                    f,
                    "daedalus attempted to invoke wasm module `{module}` with a requested output capacity that exceeds the maximum permitted output size for this module!"
                )
            }
            Self::ModuleOutputTooLarge { module } => {
                write!(
                    f,
                    "daedalus wasm module `{module}` returned an output length that exceeds the output capacity provided for this invocation!"
                )
            }
            Self::InAllocationFailed { module, err } => {
                write!(
                    f,
                    "daedalus failed to allocate memory for input to wasm module `{module}`, underlying allocation error: `{err}`!"
                )
            }
            Self::OutAllocationFailed { module, err } => {
                write!(
                    f,
                    "daedalus failed to allocate memory for output from wasm module `{module}`, underlying allocation error: `{err}`!"
                )
            }
            Self::InFreeFailed { module, err } => {
                write!(
                    f,
                    "daedalus failed to free input memory allocated within wasm module `{module}`, underlying freeing error: `{err}`!"
                )
            }
            Self::OutFreeFailed { module, err } => {
                write!(
                    f,
                    "daedalus failed to free output memory allocated within wasm module `{module}`, underlying freeing error: `{err}`!"
                )
            }
            Self::FailedToWriteInput { module, err } => {
                write!(
                    f,
                    "daedalus failed to write input bytes into the linear memory of wasm module `{module}`, underlying memory error: `{err}`!"
                )
            }
            Self::FailedToRetOutput { module, err } => {
                write!(
                    f,
                    "daedalus failed to retrieve output bytes from the linear memory of wasm module `{module}`, underlying memory error: `{err}`!"
                )
            }
            Self::FailedToRunCall { module, err } => {
                write!(
                    f,
                    "daedalus failed to execute `daedalus_call` for wasm module `{module}`, underlying execution error: `{err}`!"
                )
            }
        }
    }
}

impl Error for DaedalusWasmErrors {}

impl DaedalusWasmErrors {
    /// Returns a unique status code identifying this error
    /// as an Int
    pub const fn status_code(&self) -> i32 {
        match self {
            DaedalusWasmErrors::ModuleDoesNotExist { .. } => -1001,
            DaedalusWasmErrors::CouldNotFindProgram { .. } => -1002,
            DaedalusWasmErrors::AccessToNonGrantedModule { .. } => -1003,
            DaedalusWasmErrors::Unexpected(_) => -1004,
            DaedalusWasmErrors::InvalidMemoryAbi => -1005,
            DaedalusWasmErrors::InvalidAllocationAbi => -1006,
            DaedalusWasmErrors::InvalidFreeAbi => -1007,
            DaedalusWasmErrors::InvalidCallAbi => -1008,
            DaedalusWasmErrors::InputTooLarge { .. } => -1009,
            DaedalusWasmErrors::UserOutputTooLarge { .. } => -1010,
            DaedalusWasmErrors::ModuleOutputTooLarge { .. } => -1011,
            DaedalusWasmErrors::InAllocationFailed { .. } => -1012,
            DaedalusWasmErrors::OutAllocationFailed { .. } => -1013,
            DaedalusWasmErrors::InFreeFailed { .. } => -1014,
            DaedalusWasmErrors::OutFreeFailed { .. } => -1015,
            DaedalusWasmErrors::FailedToWriteInput { .. } => -1016,
            DaedalusWasmErrors::FailedToRetOutput { .. } => -1017,
            DaedalusWasmErrors::FailedToRunCall { .. } => -1018,
        }
    }
}

//! This provides all the actual wasm management
//! types for all of the wasm modules.

use alloc::{
    format,
    string::{String, ToString},
    vec::{Vec},
};
use daedalus_program::{WasmGrant, get_program, get_wasm_module};
use hashbrown::HashMap;
use lepton3::lepton_vm::{tagger::TagGenerator, values::Tag};
use wasmi::{
    Config, Engine, Instance, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder,
    TypedFunc,
};

/// A unique tag representing some WASM module instance
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
pub struct WasmInstanceTag(pub Tag);

/// All possible wasm errors that can occur in `Daedalus`
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
    pub instance_tag: WasmInstanceTag,

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
                program_name: name.to_string(),
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
            .memory_size(grant.max_memory_kib)
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
            instance_tag: WasmInstanceTag(tag_alloc.allocate_tag()),
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
        let output = if result as usize > output_capacity {
            return Err(DaedalusWasmErrors::ModuleOutputTooLarge {
                module: self.grant.wasm,
            });
        } else if result > 0 {
            // Read out output to give back to our caller
            let mut output = Vec::with_capacity(result.unsigned_abs() as usize);
            self.memory
                .read(&self.store, out_ptr as usize, &mut output)
                .map_err(|e| DaedalusWasmErrors::FailedToRetOutput {
                    module: self.grant.wasm,
                    err: e.to_string(),
                })?;
            Ok(output)
        } else {
            Ok(Vec::new())
        }?;

        // Now clean up our input and output allocations
        self.free
            .call(&mut self.store, (out_ptr, output_allocation))
            .map_err(|err| DaedalusWasmErrors::InFreeFailed {
                module: self.grant.wasm,
                err: err.to_string(),
            })?;
        self.free
            .call(&mut self.store, (in_ptr, input_allocation))
            .map_err(|err| DaedalusWasmErrors::OutFreeFailed {
                module: self.grant.wasm,
                err: err.to_string(),
            })?;

        Ok(InvocationRet {
            output,
            status: result,
        })
    }
}

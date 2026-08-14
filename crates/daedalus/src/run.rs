//! This file provides a custom run-loop for `Lepton3`
//! which is responsible for handling the finish, IRQ's etc.
//!
//! This also initialises the full VM and brings it into function.

use core::fmt::Display;

use alloc::boxed::Box;
use daedalus_caps::{
    errors::DaedalusCapErrors,
    ipc::capabilities::run_next_ready,
    irq::{arch::IrqArch, archs::TargetIRQArch, pending::pending_any, send_irqs},
    memory::{arch::MemoryArch, archs::TargetMemoryArch},
    program::{DaedalusState, ProgramState},
};
use daedalus_program::{StaticDaedalusImageVariants, StaticSourceLocation};
use lepton3::{
    HeapAllocatorImpl, TagGeneratorImpl, VirtualMachine,
    lepton_image::image_trait::{LeptonImage, LeptonSourceLocation},
    lepton_vm::virtual_machine::VmError,
};

#[cfg(feature = "extra-debug")]
use crate::extra_debug;

/// This is a wrapped VM Panic that bundles the corresponding
/// image that panicked with the actual error itself,
///
/// We need the image to get back the file names as the VMError
/// only includes information about the trace, but does not bundle
/// the file information or anything, (that remains in `image`.)
pub struct VmPanic<'a> {
    pub error: &'a VmError<'static, StaticSourceLocation>,
    pub image: &'a StaticDaedalusImageVariants,
}

use crate::{capabilities, handoff::arch_handoff};

/// This is the entry point to running the actual programs in `Daedalus`.
///
/// This will initialise the `Lepton3` vm, all of the `Daedalus` state
/// and run it.
pub fn run() -> ! {
    // Get the first phase we are running and it's image.
    let entry_phase = daedalus_program::get_entry_phase();
    let image: &daedalus_program::StaticDaedalusImageVariants = entry_phase.program.image;

    // Initialise the VM and `DaedalusState` which is our primary mode of operation.
    let mut tagger = TagGeneratorImpl::default();
    let state =
        DaedalusState::new(entry_phase, &mut tagger).expect("first program's grants to resolve");

    // This will be our main vm, for program execution
    let mut vm = VirtualMachine::new(
        image,
        capabilities::all(),
        HeapAllocatorImpl::default(),
        tagger,
        state,
    );

    // We have a custom run-loop so we need to manually enter the first function
    let entry = image.header().entry_point as usize;
    vm.call_function(entry, 0)
        .expect("entry point to first program should successfully be called");

    // Run-loop for VM
    //
    // We need a custom one to process IRQ's on instructions
    loop {
        // Check if there are any IRQ's.
        //
        // # Safety
        //
        // This is always going to be ran on one core, as `run`
        // will only be called on one core.
        if unsafe { pending_any() } {
            // Send all of the pending IRQ's to all
            // of their corresponding programs
            //
            // # Safety
            //
            // This is the normal context, and same as above, single core.
            unsafe {
                send_irqs(&mut vm);
            }

            // If there is another ready program, such as one woken up
            // by `send_irqs`, yield to that to lower irq latency
            if !vm.capability_state.ready_queue.is_empty() {
                // This program never blocked, so its pushed back on as ready after all the irqs.
                run_next_ready(&mut vm, Some(ProgramState::Ready)).expect(
                    "expected because of ready queue not being empty, we could run another program",
                );
            }
        }

        // Run each normal instruction like the normal run loop.
        match vm.step() {
            Ok(None) => {}
            Ok(Some(_)) => {
                todo!()
            }
            Err(error) => {
                // If we've hit end of phases, handoff to this address.
                if let Some(handoff_address) = end_of_phases(&error) {
                    // # Safety
                    //
                    // Single core, normal context, and nothing runs after this.
                    unsafe {
                        TargetIRQArch::teardown();
                        TargetMemoryArch::teardown();
                        handoff(handoff_address);
                    }
                }

                // If we errored, capture a trace for better debug info in the lepton3 image itself.
                let trace = vm.capture_trace();
                panic!(
                    "Daedalus crashed! {}",
                    VmPanic {
                        error: &VmError::WithTrace {
                            error: Box::new(error),
                            trace,
                        },
                        image: vm.image
                    }
                );
            }
        }
    }
}

impl Display for VmPanic<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.error {
            VmError::WithTrace { error, trace } => {
                writeln!(f, "runtime error: {error:?}")?;
                writeln!(f, "stack trace:")?;

                // Get all the files to match the files to the trace
                let files = self.image.files();

                // Print each frame in the trace
                for frame in trace {
                    match &frame.source_location {
                        // If there is some location associated with this frame, print it.
                        Some(loc) => {
                            let file_name = files
                                .and_then(|files| files.get(loc.file() as usize))
                                .map(|s| s.as_ref())
                                .unwrap_or("<unknown file>");

                            writeln!(
                                f,
                                "  fn[{}] {}:{}:{} ({})",
                                frame.function_idx,
                                file_name,
                                loc.line(),
                                loc.column(),
                                loc.context()
                            )?;
                        }

                        // Otherwise the best effort we can do is just the offset into the function.
                        None => {
                            writeln!(
                                f,
                                "  fn[{}] <no debug info> offset {}",
                                frame.function_idx, frame.instruction_offset,
                            )?;
                        }
                    }
                }
            }

            other => write!(f, "{other:?}")?,
        }

        Ok(())
    }
}

/// Is this error a `DaedalusCapErrors::EndOfPhase`?
///
/// If so then we should do a corresponding `handoff` to the address returned
/// as Some() otherwise None
fn end_of_phases(error: &VmError<'static, StaticSourceLocation>) -> Option<usize> {
    match error {
        VmError::CapabilityError(inner) => {
            if let Some(DaedalusCapErrors::EndOfPhases { handoff_address }) =
                inner.downcast_ref::<DaedalusCapErrors>()
            {
                return Some(*handoff_address);
            }

            None
        }
        _ => None,
    }
}

/// Hands control to the payload staged at `address`.
///
/// # Safety
///
/// Must be the last thing Daedalus ever does.
///
/// All `Daedalus` IRQ's Memory etc. must be toredown
unsafe fn handoff(address: usize) -> ! {
    #[cfg(feature = "extra-debug")]
    extra_debug::debug_handoff(address);

    // # Safety
    //
    // Preconditions are preserved by this function's header.
    unsafe { arch_handoff(address) };
}

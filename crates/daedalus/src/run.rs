//! This file provides a custom run-loop for `Lepton3`
//! which is responsible for handling the finish, IRQ's etc.
//! 
//! This also initialises the full VM and brings it into function.

use daedalus_caps::program::DaedalusState;
use lepton3::{HeapAllocatorImpl, TagGeneratorImpl, VirtualMachine};

use crate::capabilities;

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
    let state = DaedalusState::new(entry_phase, &mut tagger)
        .expect("first program's grants to resolve");

    // This will be our main vm, for program execution
    let mut vm = VirtualMachine::new(
        image,
        capabilities::all(),
        HeapAllocatorImpl::default(),
        tagger,
        state,
    );
}


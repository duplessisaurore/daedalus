//! This provides the program abstraction and
//! the state management for the VM to move data
//! between programs.

use alloc::vec::Vec;
use daedalus_service::{Phase, StaticDaedalusImageVariants, StaticLeptonImage, StaticSourceLocation};
use hashbrown::HashMap;
use lepton3::{
    HeapAllocatorImpl, TagGeneratorImpl, VirtualMachine, lepton_vm::{
        heap_allocator::HeapAllocator,
        tagger::TagGenerator,
        values::{TypeTags, Value},
        virtual_machine::{CallFrame, ErrorHandler},
    },
};

/// The current state of an inactive program.
/// 
/// This decides whether or not this program can
/// be ran
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProgramState {
    /// Blocked, this program is waiting for an event
    /// to return to it or an event to be recieved
    Blocked,

    /// Ready, this program can execute and is waiting
    /// to be picked up
    Ready

    // Running is not here since the current VM program
    // is the running one.
}

/// An inactive program, this has some state
/// that is stored outside of the VM that can be readily
/// swapped into the VM to execute this program.
pub struct InactiveProgram<
    I: StaticLeptonImage + 'static,
    H: HeapAllocator = HeapAllocatorImpl,
    T: TagGenerator = TagGeneratorImpl,
> {
    // The image of this program which we should be
    // executing when this program is active
    pub image: &'static I,

    /// The current stack of values
    pub stack: Vec<Value>,

    /// The allocator for heap values and GC
    pub heap: H,

    /// The generator for unique tags
    pub tagger: T,

    /// Records for activations of functions in a stack
    pub call_stack: Vec<CallFrame>,

    /// Registered error handlers for `Try` and `Raise`
    pub error_handlers: Vec<ErrorHandler>,

    /// The current globals set for the VM
    pub globals: Vec<Value>,

    // Pre-allocated well-known type tags.
    pub type_tags: TypeTags,
}

pub trait ProgramSwappable<H: HeapAllocator = HeapAllocatorImpl, T: TagGenerator = TagGeneratorImpl>
{
    /// This should swap the current executing state of the implementor
    /// of this trait to the state described by the InactiveProgram.
    ///
    /// The previously executing program, now replaced should have its
    /// state stored in the `InactiveProgram` that is outputted by the `swap`.
    /// 
    /// The program state should persist.
    #[must_use]
    fn swap(
        &mut self,
        program: InactiveProgram<StaticDaedalusImageVariants, H, T>,
    ) -> InactiveProgram<StaticDaedalusImageVariants, H, T>;
}

impl<I: StaticLeptonImage + 'static, H: HeapAllocator, T: TagGenerator> InactiveProgram<I, H, T> {
    /// Creates a new `InactiveProgram` that can be swapped into from
    /// this implementation of `StaticLeptonImage`.
    ///
    /// This essentially constructs a new VM from the `StaticLeptonImage`
    /// and then steals all of its initial state to create the program.
    /// 
    /// This program starts in the `Ready` state.
    #[must_use]
    pub fn from_image(image: &'static I) -> Self {
        let mut initial_machine_state =
            VirtualMachine::new(image, Vec::new(), H::default(), T::default(), ());

        // Call the entry point, this should succeed...
        let entry = image.header().entry_point as usize;
        initial_machine_state.call_function(entry, 0).expect("expects entering the entry point to succeed");

        Self {
            image,
            stack: initial_machine_state.stack,
            heap: initial_machine_state.heap,
            tagger: initial_machine_state.tagger,
            call_stack: initial_machine_state.call_stack,
            error_handlers: initial_machine_state.error_handlers,
            globals: initial_machine_state.globals,
            type_tags: initial_machine_state.type_tags,
        }
    }
}

impl<CS, H: HeapAllocator, T: TagGenerator> ProgramSwappable<H, T>
    for VirtualMachine<'static, CS, StaticSourceLocation, H, T, StaticDaedalusImageVariants>
{
    fn swap(
        &mut self,
        program: InactiveProgram<StaticDaedalusImageVariants, H, T>,
    ) -> InactiveProgram<StaticDaedalusImageVariants, H, T> {
        // Replace each component of the VM so we execute the new inactive program
        // and return all of the prior stuff as an InactiveProgram.
        InactiveProgram {
            image: core::mem::replace(&mut self.image, program.image),
            stack: core::mem::replace(&mut self.stack, program.stack),
            heap: core::mem::replace(&mut self.heap, program.heap),
            tagger: core::mem::replace(&mut self.tagger, program.tagger),
            call_stack: core::mem::replace(&mut self.call_stack, program.call_stack),
            error_handlers: core::mem::replace(&mut self.error_handlers, program.error_handlers),
            globals: core::mem::replace(&mut self.globals, program.globals),
            type_tags: core::mem::replace(&mut self.type_tags, program.type_tags),
        }
    }
}

/// The current state of the daedalus execution
/// 
/// This is stored with capabilities as the main engine
/// of the state
pub struct DaedalusState<I: StaticLeptonImage + 'static, H: HeapAllocator, T: TagGenerator> {
    /// The current phase being executed
    pub current_phase: &'static Phase<I>,

    // The set of programs to execute that
    // are not currently executing
    pub programs: HashMap<&'static str, InactiveProgram<I, H, T>>,
}

impl<I: StaticLeptonImage + 'static, H: HeapAllocator, T: TagGenerator> DaedalusState<I, H, T> {
    /// Creates a new DaedalusState with empty ready programs initialised
    /// with the current phase
    /// 
    /// It is expected that this is instantly used in a `VirtualMachine`
    /// with the current_phase properly matching the image else doom will occur.
    pub fn new(current_phase: &'static Phase<I>) -> Self {
        Self {
            current_phase,
            programs: HashMap::new()
        }
    }
}
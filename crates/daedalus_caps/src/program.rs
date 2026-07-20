//! This provides the program abstraction and
//! the state management for the VM to move data
//! between programs.

use alloc::vec::Vec;
use daedalus_service::StaticLeptonImage;
use lepton3::{
    HeapAllocatorImpl, TagGeneratorImpl, VirtualMachine, lepton_vm::{
        heap_allocator::HeapAllocator,
        tagger::TagGenerator,
        values::{TypeTags, Value},
        virtual_machine::{CallFrame, ErrorHandler},
    },
};

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
    /// state storeed in the `InactiveProgram` that is outputted by the `swap`.
    #[must_use]
    fn swap(
        &mut self,
        program: InactiveProgram<impl StaticLeptonImage, H, T>,
    ) -> InactiveProgram<impl StaticLeptonImage, H, T>;
}

impl<I: StaticLeptonImage + 'static, H: HeapAllocator, T: TagGenerator> InactiveProgram<I, H, T> {
    /// Creates a new `InactiveProgram` that can be swapped into from
    /// this implementation of `StaticLeptonImage`.
    /// 
    /// This essentially constructs a new VM from the `StaticLeptonImage`
    /// and then steals all of its initial state to create the program.
    #[must_use]
    pub fn from_image(image: &'static I) -> Self {
        let initial_machine_state = VirtualMachine::new(image, Vec::new(), H::default(), T::default(), ());

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

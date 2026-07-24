//! This provides the program abstraction and
//! the state management for the VM to move data
//! between programs.

use alloc::{collections::vec_deque::VecDeque, vec::Vec};
use daedalus_program::{
    Phase, StaticDaedalusImageVariants, StaticLeptonImage, StaticSourceLocation,
};
use hashbrown::{HashMap, hash_map::Entry};
use lepton3::{
    HeapAllocatorImpl, TagGeneratorImpl, VirtualMachine,
    lepton_vm::{
        heap_allocator::HeapAllocator,
        tagger::TagGenerator,
        values::{Tag, TypeTags, Value},
        virtual_machine::{CallFrame, ErrorHandler},
    },
};

use crate::migrate::migrate;

/// A unique program's call reply association
#[derive(Debug, Clone, Copy)]
pub struct CallAssociation {
    /// This is the tag in the caller side (which we are replying to)
    pub caller_side_tag: CallTag,

    /// This is the name of the caller's program which we return to
    pub caller_program: &'static str,
}

/// A unique call's Tag which associates a reply back
/// to some program
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
pub struct CallTag(pub Tag);

/// A request sitting in the inbox of a program, waiting
/// to be recieved (see `inbox` in `DaedalusState`)
pub struct Message {
    /// The unique call tag associated with this new message
    /// to the inbox so the receiever can reply
    ///
    /// `None` marks a message that will have it's `call_tag`
    /// be delivered as a `Unit` This is for cases where a program
    /// is woken up in the `block_recv` state by not a call (for
    /// example with a `finish`)
    pub tag: Option<CallTag>,

    /// The argument the caller passed
    pub args: Value,
}

impl Message {
    /// Pushes this message onto `stack` in the shape discussed in
    /// the header comment of `daedalus_caps::capabilities`,.
    ///
    ///     [<top> `payload`, `call_tag`]
    ///
    /// If the tag is `None` this is a `Unit`.
    pub fn deliver_onto(self, stack: &mut Vec<Value>) {
        match self.tag {
            Some(tag) => stack.push(Value::Tag(tag.0)),
            None => stack.push(Value::Unit),
        }

        stack.push(self.args);
    }
}
/// The current state of an inactive program.
///
/// This decides whether or not this program can
/// be ran and the condition that's blocking it.
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProgramState {
    /// Blocked, but waiting for a `recv` that can
    /// potentially wake it up.
    BlockedOnRecv,

    /// This program is blocked and is waiting for a `reply`
    /// on one of it's calls to a different program
    ///
    /// This can only be woken up on a reply with the associated
    /// `CallTag`.
    BlockedOnReply { tag: CallTag },

    /// Ready, this program can execute and is waiting
    /// to be picked up
    Ready,
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
    /// Name of this Program in the set
    /// of `daedalus_program`
    pub name: &'static str,

    /// The current inactivity state of the program.
    ///
    /// This is essentially the three-state program model
    /// but without running (as if it was running it wouldn't
    /// be an `InactiveProgram`)
    pub state: ProgramState,

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

    /// Pending replies for the inactive program
    ///
    /// This is a map of the tag allocated for its calls back to the
    /// program that called it
    pub pending_replies: HashMap<CallTag, CallAssociation>,

    /// Pending messages to the inactive program
    ///
    /// This is because a program can yield without
    /// necessarily having to recv a message, say
    /// calling something else too
    pub inbox: VecDeque<Message>,
}

pub trait ProgramSwappable<H: HeapAllocator = HeapAllocatorImpl, T: TagGenerator = TagGeneratorImpl>
{
    /// This should swap the current executing state of the implementor
    /// of this trait to the state described by the InactiveProgram.
    ///
    /// The previously executing program, now replaced should have its
    /// state stored in the `InactiveProgram` that is outputted by the `swap`.
    ///
    /// The program state passed in is the new program state of the previously
    /// running program that is returned
    #[must_use]
    fn swap(
        &mut self,
        program: InactiveProgram<StaticDaedalusImageVariants, H, T>,
        new_state: ProgramState,
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
    ///
    /// The name of the program must match the one that can be looked up
    /// to find this program again.
    ///
    /// This `Program` has no arguments in it's entry point function.
    #[must_use]
    pub fn from_image_with_name(image: &'static I, name: &'static str) -> Self {
        let mut initial_machine_state =
            VirtualMachine::new(image, Vec::new(), H::default(), T::default(), ());

        // Call the entry point in the new image, this should succeed...
        let entry = image.header().entry_point as usize;
        initial_machine_state
            .call_function(entry, 0)
            .expect("expects entering the entry point to succeed");

        Self::from_initial_machine(image, name, initial_machine_state)
    }

    /// Does the same as `from_image_with_name` but
    /// instead, passes in the provided `arg` that may exist
    /// in the heap allocator `arg_heap_alloc` to the starting
    /// method of the `image`.
    ///
    /// The `arg_heap_alloc` is required for the full recursive
    /// migration over into the new `InactiveProgram`.
    #[must_use]
    pub fn from_image_with_name_and_arg(
        image: &'static I,
        name: &'static str,
        arg: Value,
        arg_heap_alloc: &mut H,
    ) -> Self {
        let mut initial_machine_state =
            VirtualMachine::new(image, Vec::new(), H::default(), T::default(), ());

        // Migrate over the argument if necessary over to the new initial machine state which we package up
        // for the `InactiveProgram`
        initial_machine_state.stack.push(migrate(
            arg_heap_alloc,
            &mut initial_machine_state.heap,
            &mut initial_machine_state.tagger,
            arg,
        ));

        // Call the entry point in the new image, this should succeed...
        // we also pass in our one argument here
        let entry = image.header().entry_point as usize;
        initial_machine_state
            .call_function(entry, 1)
            .expect("expects entering the entry point to succeed");

        Self::from_initial_machine(image, name, initial_machine_state)
    }

    /// Creates a new `InactiveProgram` that can be swapped into from
    /// this implementation of `StaticLeptonImage`.
    ///
    /// This takes in an `initial_machine_state` VirtualMachine and packages
    /// all of its current state alongside the `image` and it's `name`
    /// into an `InactiveMachine`
    #[must_use]
    fn from_initial_machine(
        image: &'static I,
        name: &'static str,
        initial_machine_state: VirtualMachine<'_, (), StaticSourceLocation, H, T, I>,
    ) -> Self {
        Self {
            name,
            state: ProgramState::Ready,
            image,
            stack: initial_machine_state.stack,
            heap: initial_machine_state.heap,
            tagger: initial_machine_state.tagger,
            call_stack: initial_machine_state.call_stack,
            error_handlers: initial_machine_state.error_handlers,
            globals: initial_machine_state.globals,
            type_tags: initial_machine_state.type_tags,
            pending_replies: HashMap::new(),
            inbox: VecDeque::new(),
        }
    }

    /// If this program is blocked in `block_recv` and has an
    /// item in it's inbox (from another program) then wakes up
    /// this program (puts it in the `Ready` state) and adds the `Message`
    /// to the stack of this program.
    ///
    /// This returns whether or not the program was woken.
    pub fn wake_recv(&mut self) -> bool {
        if self.state != ProgramState::BlockedOnRecv {
            return false;
        }

        let Some(message) = self.inbox.pop_front() else {
            return false;
        };

        message.deliver_onto(&mut self.stack);

        self.state = ProgramState::Ready;
        true
    }
}

impl<H: HeapAllocator, T: TagGenerator> ProgramSwappable<H, T>
    for VirtualMachine<
        'static,
        DaedalusState<StaticDaedalusImageVariants, H, T>,
        StaticSourceLocation,
        H,
        T,
        StaticDaedalusImageVariants,
    >
{
    fn swap(
        &mut self,
        program: InactiveProgram<StaticDaedalusImageVariants, H, T>,
        new_state: ProgramState,
    ) -> InactiveProgram<StaticDaedalusImageVariants, H, T> {
        // Replace each component of the VM so we execute the new inactive program
        // and return all of the prior stuff as an InactiveProgram.
        InactiveProgram {
            state: new_state,
            image: core::mem::replace(&mut self.image, program.image),
            stack: core::mem::replace(&mut self.stack, program.stack),
            heap: core::mem::replace(&mut self.heap, program.heap),
            tagger: core::mem::replace(&mut self.tagger, program.tagger),
            call_stack: core::mem::replace(&mut self.call_stack, program.call_stack),
            error_handlers: core::mem::replace(&mut self.error_handlers, program.error_handlers),
            globals: core::mem::replace(&mut self.globals, program.globals),
            type_tags: core::mem::replace(&mut self.type_tags, program.type_tags),

            // Swap the current daedalus state so we can reply/recv things again
            pending_replies: core::mem::replace(
                &mut self.capability_state.pending_replies,
                program.pending_replies,
            ),

            inbox: core::mem::replace(&mut self.capability_state.inbox, program.inbox),
            name: core::mem::replace(&mut self.capability_state.current_program, program.name),
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

    /// The name of the program currently loaded in the VM.
    ///
    /// This is seperate from the current phase as programs
    /// can call other programs
    pub current_program: &'static str,

    /// Pending replies for the current phase being executed
    ///
    /// This is a map of the tag allocated for this call back to the
    /// program that called it.
    pub pending_replies: HashMap<CallTag, CallAssociation>,

    /// Pending messages to the current phase being execeuted
    ///
    /// This is because programs can technically send a message to a `Running`
    /// program, in which then they wait. but the running program should not
    /// recieve these messages until they are actually in the `BlockedOnRecv` state.
    pub inbox: VecDeque<Message>,

    // The set of programs to execute that
    // are not currently executing
    pub programs: HashMap<&'static str, InactiveProgram<I, H, T>>,

    /// Names of programs currently in a `Ready` state, in order
    pub ready_queue: VecDeque<&'static str>,
}

impl<I: StaticLeptonImage + 'static, H: HeapAllocator, T: TagGenerator> DaedalusState<I, H, T> {
    /// Creates a new DaedalusState with empty ready programs initialised
    /// with the current phase
    ///
    /// It is expected that this is instantly used in a `VirtualMachine`
    /// with the current_phase properly matching the image else doom will occur.
    pub fn new(current_phase: &'static Phase<I>) -> Self {
        Self {
            current_program: current_phase.program.name,
            current_phase,
            programs: HashMap::new(),
            ready_queue: VecDeque::new(),
            pending_replies: HashMap::new(),
            inbox: VecDeque::new(),
        }
    }

    /// Ensures `name` exists as a program and is ready
    ///
    /// This will either find `name` in the current set of programs and
    /// mark it as ready (if it's BlockedOnReply/Recv) or create a new
    /// program from the image associated with the program `name`.
    ///
    /// This new program from the image associated
    pub fn make_ready(&mut self, name: &'static str, image: &'static I) {
        match self.programs.entry(name) {
            Entry::Occupied(mut entry) => {
                // Mark as ready and push onto the queue
                let program = entry.get_mut();

                if program.state != ProgramState::Ready {
                    // Since a blocked program (blockrecv)
                    // expects a message on its inbox (else
                    // itll pop too much) we push the empty
                    // message notification.
                    program.inbox.push_back(Message {
                        tag: None,
                        args: Value::Unit,
                    });

                    if program.wake_recv() {
                        self.ready_queue.push_back(name);
                    }
                }

                // Already ready
            }

            // Create new image and push onto the queue
            Entry::Vacant(entry) => {
                entry.insert(InactiveProgram::from_image_with_name(image, name));
                self.ready_queue.push_back(name);
            }
        }
    }
}

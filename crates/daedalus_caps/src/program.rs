//! This provides the program abstraction and
//! the state management for the VM to move data
//! between programs.

use alloc::{collections::vec_deque::VecDeque, vec::Vec};
use daedalus_program::{
    Grant, Phase, StaticDaedalusImageVariants, StaticLeptonImage, StaticSourceLocation,
};
use hashbrown::{HashMap, hash_map::Entry};
use lepton3::{
    HeapAllocatorImpl, TagGeneratorImpl, VirtualMachine,
    lepton_vm::{
        capabilities::CapabilityGcRoots,
        heap_allocator::HeapAllocator,
        tagger::TagGenerator,
        values::{Tag, TypeTags, Value},
        virtual_machine::{CallFrame, ErrorHandler},
    },
};

use crate::{
    errors::DaedalusCapErrors,
    ipc::migrate::migrate,
    memory::{MintedGrantRegions, Region, RegionHandle, mint_grants},
};

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
    /// be delivered as a `Unit`.
    ///
    /// These are for notifications from `daedalus` rather than
    /// from a call itself, and cannot be replied to.
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

    // View `DaedalusState` for the meaning of these.
    pub pending_replies: HashMap<CallTag, CallAssociation>,
    pub inbox: VecDeque<Message>,
    pub regions: HashMap<RegionHandle, Region>,
    pub named_grants: HashMap<&'static str, RegionHandle>,
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
    pub fn from_image_with_name(
        image: &'static I,
        name: &'static str,
        grants: &'static [Grant],
    ) -> Result<Self, DaedalusCapErrors> {
        let mut initial_machine_state =
            VirtualMachine::new(image, Vec::new(), H::default(), T::default(), ());

        // Call the entry point in the new image, this should succeed...
        let entry = image.header().entry_point as usize;
        initial_machine_state
            .call_function(entry, 0)
            .map_err(|_| DaedalusCapErrors::FailedToEnterProgramEntryPoint { name })?;

        Self::from_initial_machine(image, name, grants, initial_machine_state)
    }

    /// Does the same as `from_image_with_name` but
    /// instead, passes in the provided `arg` that may exist
    /// in the heap allocator `arg_heap_alloc` to the starting
    /// method of the `image`.
    ///
    /// The `arg_heap_alloc` is required for the full recursive
    /// migration over into the new `InactiveProgram`.
    ///
    /// This does not guarantee the argument is passed to the new
    /// program if the new program's entry point does not take any
    /// arguments.
    #[must_use]
    pub fn from_image_with_name_and_arg(
        image: &'static I,
        name: &'static str,
        grants: &'static [Grant],
        arg: Value,
        arg_heap_alloc: &mut H,
    ) -> Result<Self, DaedalusCapErrors> {
        let mut initial_machine_state =
            VirtualMachine::new(image, Vec::new(), H::default(), T::default(), ());

        // Grab the entry point, and the number of args
        // we optionally parse the argument if there is one
        let entry = image.header().entry_point as usize;
        let arg_count = image
            .function_table()
            .get(entry)
            .expect("validator ensures that entry point must exist in the function table")
            .arg_count;

        match arg_count {
            // Entry takes no arguments, drop the arg, but still meow and purr andd mrrrprr everywhere
            0 => initial_machine_state
                .call_function(entry, 0)
                .map_err(|_| DaedalusCapErrors::FailedToEnterProgramEntryPoint { name })?,

            // Actual argument, call it with 1 arg.
            1 => {
                initial_machine_state.stack.push(migrate(
                    arg_heap_alloc,
                    &mut initial_machine_state.heap,
                    &mut initial_machine_state.tagger,
                    arg,
                ));
                initial_machine_state
                    .call_function(entry, 1)
                    .map_err(|_| DaedalusCapErrors::FailedToEnterProgramEntryPoint { name })?
            }

            _ => unreachable!("daedalus build validation handles this casee"),
        }

        Self::from_initial_machine(image, name, grants, initial_machine_state)
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
        grants: &'static [Grant],
        mut initial_machine_state: VirtualMachine<'_, (), StaticSourceLocation, H, T, I>,
    ) -> Result<Self, DaedalusCapErrors> {
        // Turn the grants into `Regions` in our new initial machine state.
        let MintedGrantRegions {
            regions,
            named_grants,
        } = mint_grants(grants, &mut initial_machine_state.tagger)?;

        Ok(Self {
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
            regions,
            named_grants,
        })
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
            regions: core::mem::replace(&mut self.capability_state.regions, program.regions),
            named_grants: core::mem::replace(
                &mut self.capability_state.named_grants,
                program.named_grants,
            ),
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
    /// program that called it alongside the tag in the program that called it's space
    /// as part of a `CallAssociation`.
    pub pending_replies: HashMap<CallTag, CallAssociation>,

    /// Pending messages to the current phase being execeuted
    ///
    /// These can be viewed in a program through the `block_recv` capability call,
    /// and a program may have multiple pending messgaes (through `non_block_call`
    /// to it etc.)
    pub inbox: VecDeque<Message>,

    // The set of programs to execute that
    // are not currently executing
    pub programs: HashMap<&'static str, InactiveProgram<I, H, T>>,

    /// Names of programs currently in a `Ready` state, in order
    pub ready_queue: VecDeque<&'static str>,

    /// The set of regions this program owns, essentially the memory
    /// regions it is allowed to access, by handle.
    pub regions: HashMap<RegionHandle, Region>,

    /// All regions initially come from "grants" which are *named*
    /// this is how programs initially get the handles for their regions.
    pub named_grants: HashMap<&'static str, RegionHandle>,
}

impl<I: StaticLeptonImage + 'static, H: HeapAllocator, T: TagGenerator> DaedalusState<I, H, T> {
    /// Creates a new DaedalusState with empty ready programs initialised
    /// with the current phase
    ///
    /// It is expected that this is instantly used in a `VirtualMachine`
    /// with the current_phase properly matching the image else doom will occur.
    pub fn new(
        current_phase: &'static Phase<I>,
        tagger: &mut T,
    ) -> Result<Self, DaedalusCapErrors> {
        let program = current_phase.program;

        // The initial program passed in is going to need
        // its regions set up too from grants, so do that.
        let MintedGrantRegions {
            regions,
            named_grants,
        } = mint_grants(program.grants, tagger)?;

        Ok(Self {
            current_program: current_phase.program.name,
            current_phase,
            programs: HashMap::new(),
            ready_queue: VecDeque::new(),
            pending_replies: HashMap::new(),
            inbox: VecDeque::new(),
            regions,
            named_grants
        })
    }

    /// Ensures `name` exists as a program and sets it up as ready if it should be.
    ///
    /// This will either find `name` in the current set of programs and
    /// mark it as ready if it is ready to be ready (e.g BlockOnRecv has inbox msg).
    /// or create a new program from the image associated with the program `name`.
    /// 
    /// The grants are used to setup the initial regions for this program.
    pub fn make_ready(&mut self, name: &'static str, image: &'static I, grants: &'static [Grant]) -> Result<(), DaedalusCapErrors> {
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
                entry.insert(InactiveProgram::from_image_with_name(image, name, grants)?);
                self.ready_queue.push_back(name);
            }
        };

        Ok(())
    }
}

impl<I: StaticLeptonImage + 'static, H: HeapAllocator, T: TagGenerator> CapabilityGcRoots
    for DaedalusState<I, H, T>
{
    /// The running program's inbox payloads maybe heap values which should not
    /// be collected by the gc while they remain in the inbox (else by the time a program
    /// get's them out, they may all be corrupted!)
    fn append_roots<'roots>(&'roots mut self, roots: &mut Vec<&'roots mut Value>) {
        for message in &mut self.inbox {
            roots.push(&mut message.args);
        }
    }
}

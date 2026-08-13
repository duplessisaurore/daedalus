//! Panic handler with extra debug attached
//!
//! This is a special panic handling module that relies
//! on the existence of a `Daedalus` phase named "panic" to exist,
//!
//! This phase's program will be started in a fresh vm with the arguments
//! that corresponding to a Boson3 string of the panic message,
//!
//! It is then up to the program using the corresponding `Daedalus`
//! items to print out this message or do something to recover from an unexpected panic.

use core::fmt::Write;
use core::panic::PanicInfo;

use alloc::{string::String, vec::Vec};
use daedalus_caps::program::DaedalusState;
use daedalus_program::get_phase;
use lepton3::{
    HeapAllocatorImpl, TagGeneratorImpl, VirtualMachine,
    lepton_image::image_trait::LeptonImage,
    lepton_vm::{
        heap_allocator::{HeapAllocator, HeapItem},
        values::Value,
    },
};

use crate::capabilities;

/// Validates that a panic phase actually exists at comptime
const fn validate_panic_phase() {
    get_phase("panic").expect("A panic phase must exist in the current user defined daedalus execution graph for `panic-debug` to be enabled!");
}

const _: () = validate_panic_phase();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // This is the panic message, which we will pass as the arg to the panic phase.
    let mut message = String::new();

    writeln!(
        message,
        "\n\n=^..^=   =^..^=   =^..^=    ERROR!    =^..^=    =^..^=    =^..^=\n"
    )
    .ok();
    writeln!(message, "Message: {}", info.message()).ok();

    if let Some(location) = info.location() {
        writeln!(
            message,
            "\nLocation: {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        )
        .ok();
    } else {
        writeln!(message, "\nLocation: unknown").ok();
    }

    writeln!(
        message,
        "\n=^..^=   =^..^=   =^..^=    =^..^=    =^..^=    =^..^=    =^..^="
    )
    .ok();

    // Get the panic phase we are running and it's image.
    let panic_phase = get_phase("panic").unwrap();
    let image: &daedalus_program::StaticDaedalusImageVariants = panic_phase.program.image;

    // Initialise the VM and a new fresh `DaedalusState` for this.
    let mut tagger = TagGeneratorImpl::default();
    let state =
        DaedalusState::new(panic_phase, &mut tagger).expect("first program's grants to resolve");

    // This will be our main vm, for program execution
    let mut vm = VirtualMachine::new(
        image,
        capabilities::all(),
        HeapAllocatorImpl::default(),
        tagger,
        state,
    );

    // Turn the message into an actual Lepton3 value and push onto the image
    let message_value = message
        .as_bytes()
        .iter()
        .map(|byte| Value::UInt((*byte) as u64))
        .collect::<Vec<_>>();
    let heap_index = vm.heap.alloc_raw(HeapItem::Array(message_value));
    vm.stack.push(Value::Array(heap_index));

    // We need to enter the image in a custom way with the panic message as the arg
    let entry = image.header().entry_point as usize;
    vm.call_function(entry, 1)
        .expect("entry point to first program should successfully be called");

    // Run the image.
    loop {
        match vm.step() {
            Ok(None) => {},
            Ok(_) => loop {
                core::hint::spin_loop();
            },
            Err(_) => loop {
                core::hint::spin_loop();
            },
        }
    }
}

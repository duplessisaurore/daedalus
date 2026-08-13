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

use daedalus_program::get_phase;

/// Validates that a panic phase actually exists at comptime
const fn validate_panic_phase() {
    get_phase("panic").expect("A panic phase must exist in the current user defined daedalus execution graph for `panic-debug` to be enabled!");
}

const _: () = validate_panic_phase();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    
}

//! Panic handler
//!
//! This is the simpler one which just aborts and loops,
//! see `extra-debug`, the feature for a more comprehensive one.

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

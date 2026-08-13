//! Panic handler
//!
//! This either will loop, or TODO: platform specific method
//! on debug feature enabled

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

//! Panic handler with extra debug attached
//!
//! This is a special panic handling module that relies
//! on the existence of a special debug out writing module

use core::fmt::Write;
use core::panic::PanicInfo;

/// Prints a bunch of information about the current
/// state of daedalus on start for debugging purposes.
pub fn debug_start() {
    // This is the debug writer which the fancy info will be printed through
    let mut writer = DebugWriter::new();
    writer.init();

    writeln!(writer, "\n=^..^=  DAEDALUS DEBUG  =^..^=").ok();

    writeln!(writer, "Daedalus Version: {}", env!("CARGO_PKG_VERSION"),).ok();

    writeln!(
        writer,
        "Built for {} in {} mode",
        env!("DAEDALUS_BUILD_TARGET"),
        env!("DAEDALUS_BUILD_PROFILE"),
    )
    .ok();

    writeln!(writer, "Build time: {}", env!("DAEDALUS_BUILD_TIME"),).ok();

    writeln!(writer, "=^..^=  =^..^=  =^..^=  =^..^=\n").ok();
}

/// Prints a final barrier at the end of handoff for daedalus if the user
/// programs have no output, for extra debug reasons
pub fn debug_handoff(address: usize) {
    // This is the debug writer which the fancy info will be printed through
    let mut writer = DebugWriter::new();
    writer.init();

    writeln!(
        writer,
        "\n=^..^= DAEDALUS HANDOFF TO 0x{:x} =^..^=\n",
        address
    )
    .ok();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // This is the debug writer which the fancy panic will be printed through
    let mut writer = DebugWriter::new();
    writer.init();

    writeln!(
        writer,
        "\n\n\x1b[95m=^..^=   =^..^=   =^..^=    ERROR!    =^..^=    =^..^=    =^..^=\x1b[0m\n"
    )
    .ok();

    writeln!(writer, "Message: {}", info.message()).ok();

    if let Some(location) = info.location() {
        writeln!(
            writer,
            "\nLocation: {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        )
        .ok();
    } else {
        writeln!(writer, "\nLocation: unknown").ok();
    }

    writeln!(
        writer,
        "\n\x1b[95m=^..^=   =^..^=   =^..^=    =^..^=    =^..^=    =^..^=    =^..^=\x1b[0m"
    )
    .ok();

    loop {
        core::hint::spin_loop();
    }
}

/// This is the generic trait that all of the debug writers must implement
pub trait DaedalusDebugWriter: Write {
    /// This should return a new copy of this writer
    fn new() -> Self;

    /// This should do all the corresponding initialisation the writer needs
    fn init(&mut self);
}

// The specific debug writer to use for the module
cfg_if::cfg_if! {
    // zynqmp, over uart0
    if #[cfg(all(target_arch = "aarch64", feature = "platform-zynqmp"))] {
        mod zynqmp;
        pub use zynqmp::Uart0 as DebugWriter;
    } else {
        compile_error!("no target platform selected that has supported panic debug; enable an option (or add one if it doesn't exist!)");
    }
}

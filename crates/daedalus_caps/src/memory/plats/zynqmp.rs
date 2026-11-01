//! The ZYNQMP platform requires for extra-debug
//! the UART0 debug mapped in as Device-ngnre (to talk to it)

use crate::memory::plats::ExtraDeviceSpan;

#[cfg(feature = "extra-debug")]
/// UART0, used for the bootloader's optional low-level debug output.
pub const DEBUG_UART_BASE: usize = 0xFF00_0000;

#[cfg(feature = "extra-debug")]
/// One page contains all UART0 registers accessed by the debug writer.
pub const DEBUG_UART_SIZE: usize = 0x1000;

pub const EXTRA_DEVICE_SPANS: &'static [ExtraDeviceSpan] = &[

    #[cfg(feature = "extra-debug")]
    ExtraDeviceSpan {
        base: DEBUG_UART_BASE,
        len: DEBUG_UART_SIZE
    }
    
];
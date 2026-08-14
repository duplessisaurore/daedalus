//! This is the specific debug output for the ZYNQMP platform.
//!
//! This uses the UART0 writer.

use core::fmt::Write;

use crate::extra_debug::DaedalusDebugWriter;

/// The base address of UART0 on the ZYNQMP platforms.
const UART0_BASE: usize = 0xFF00_0000;

/// Status register, whether or not the UART0 can be written to.
const UART_SR: usize = UART0_BASE + 0x2C;

/// The actual FIFO register for writing our debug data out
const UART_FIFO: usize = UART0_BASE + 0x30;

/// Whether the SR register indicates the UART0 is full.
const SR_TXFULL: u32 = 1 << 4;

/// A writer which uses the UART0
/// of the zynqmp platforms.
pub struct Uart0;

impl Uart0 {
    #[inline(always)]
    fn read_reg(addr: usize) -> u32 {
        // # Safety
        //
        // This is the debug output, for print
        // we should be safely able to access this
        // as daedalus should have full access to memory
        // but its not so bad if it fails ig.
        unsafe { core::ptr::read_volatile(addr as *const u32) }
    }

    #[inline(always)]
    fn write_reg(addr: usize, value: u32) {
        // # Safety
        //
        // This is the debug output, for print
        // we should be safely able to access this
        // as daedalus should have full access to memory
        // but its not so bad if it fails ig.
        unsafe { core::ptr::write_volatile(addr as *mut u32, value) }
    }

    pub fn putc(&mut self, c: u8) {
        // Wait until TX FIFO has room
        while Self::read_reg(UART_SR) & SR_TXFULL != 0 {
            core::hint::spin_loop();
        }

        Self::write_reg(UART_FIFO, c as u32);
    }

    pub fn puts(&mut self, s: &str) {
        for byte in s.bytes() {
            if byte == b'\n' {
                self.putc(b'\r');
            }

            self.putc(byte);
        }
    }
}

impl DaedalusDebugWriter for Uart0 {
    fn new() -> Self {
        Uart0 {}
    }

    fn init(&mut self) {}
}

impl Write for Uart0 {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.puts(s);
        Ok(())
    }
}

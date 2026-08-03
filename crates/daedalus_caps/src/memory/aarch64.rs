//! The architecture specific memory operations
//! for `aarch64`.
//!
//! This supplies the necessary underlying
//! arch specific ops for memory caps.

use crate::memory::arch::{AccessWidth, MemoryArch};

/// The `AArch64` memory operations
///
/// These are for `ARM 64-bit` platforms
/// such as the ZCU106.
pub struct Aarch64;

impl MemoryArch for Aarch64 {
    unsafe fn read(pointer: *const u8, width: AccessWidth) -> u64 {
        // The `read` function invariants specify that this must be a safe
        // aligned access.
        unsafe {
            match width {
                AccessWidth::U8 => u64::from(pointer.read_volatile()),
                AccessWidth::U16 => u64::from((pointer as *const u16).read_volatile()),
                AccessWidth::U32 => u64::from((pointer as *const u32).read_volatile()),
                AccessWidth::U64 => (pointer as *const u64).read_volatile(),
            }
        }
    }

    unsafe fn write(pointer: *mut u8, width: AccessWidth, value: u64) {
        // The `write` function invariants specify that this must be a safe
        // aligned access.
        unsafe {
            match width {
                AccessWidth::U8 => pointer.write_volatile(value as u8),
                AccessWidth::U16 => (pointer as *mut u16).write_volatile(value as u16),
                AccessWidth::U32 => (pointer as *mut u32).write_volatile(value as u32),
                AccessWidth::U64 => (pointer as *mut u64).write_volatile(value),
            }
        }
    }

    unsafe fn copy(_source: *const u8, _destination: *mut u8, _len: usize) {
        todo!()
    }

    unsafe fn fill(_destination: *mut u8, _value: u8, _len: usize) {
        todo!()
    }

    unsafe fn flush_range(_pointer: *mut u8, _len: usize) {
        todo!()
    }

    unsafe fn setup() {
        todo!()
    }

    unsafe fn teardown() {
        todo!()
    }
}

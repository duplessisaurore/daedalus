//! The architecture specific memory operations
//! for `aarch64`.
//! 
//! This supplies the necessary underlying
//! arch specific ops for memory caps.

use crate::memory::arch::MemoryArch;

/// The `AArch64` memory operations
/// 
/// These are for `ARM 64-bit` platforms
/// such as the ZCU106.
pub struct Aarch64;

impl MemoryArch for Aarch64 {
    unsafe fn read(pointer: *const u8, width: super::arch::AccessWidth) -> u64 {
        todo!()
    }

    unsafe fn write(pointer: *mut u8, width: super::arch::AccessWidth, value: u64) {
        todo!()
    }

    unsafe fn copy(source: *const u8, destination: *mut u8, len: usize) {
        todo!()
    }

    unsafe fn fill(destination: *mut u8, value: u8, len: usize) {
        todo!()
    }

    unsafe fn flush_range(pointer: *mut u8, len: usize) {
        todo!()
    }

    unsafe fn setup() {
        todo!()
    }

    unsafe fn teardown() {
        todo!()
    }
}
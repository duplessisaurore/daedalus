//! All implemented architectures lie in this
//! submodule for memory ops
//!
//! This module exports based on the target_arch
//! the memory ops.

cfg_if::cfg_if! {
    if #[cfg(target_arch = "aarch64")] {
        mod aarch64;
        pub use aarch64::Aarch64 as TargetMemoryArch;
    } else {
        compile_error!("no target memory architecture selected when memory was attempted to be used; enable an option");
    }
}

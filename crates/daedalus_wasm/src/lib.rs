//! `Daedalus` is an experimental open-source micro bootloader for
//! the LionsOS system developed ontop of seL4.
//!
//! Check out the [repository README](https://github.com/duplessisaurore/daedalus/blob/main/README.md)
//! for more information.
//!
//! ## Daedalus Caps
//!
//! The `daedalus_wasm` crate provides `WASM` integration through
//! capabilities to `daedaelus` programs, enabling integration with
//! more existing tooling/software around for Lepton3.

#![no_std]

extern crate alloc;

/// WASM manager, this is the state-shared
/// module manager for all wasm programs.
pub mod manager;
pub use manager::*;
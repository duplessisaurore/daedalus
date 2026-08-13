//! `Daedalus` is an experimental open-source micro bootloader for
//! the LionsOS system developed ontop of seL4.
//!
//! Check out the [repository README](https://github.com/duplessisaurore/daedalus/blob/main/README.md)
//! for more information.
//!
//! This is the actual binary for `Daedalus` which builds ontop of
//! `daedalus_program` and `daedalus_caps`.

#![no_std]
#![no_main]

extern crate alloc;

/// The entry point to `Daedalus` is defined here
mod entry;

/// Heap setup for `Lepton3`
mod heap;

/// The actual entry point to `Lepton3` with a custom
/// run-loop for handling IRQ's.
mod run;

/// The capabilities we provide to the `Lepton3` vm
/// for the bootloader programs.
mod capabilities;

/// Platform/configuration specific entry method into `Daedalus`
/// from prior stages
mod startup;

/// Rust panic handling
mod panic;
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

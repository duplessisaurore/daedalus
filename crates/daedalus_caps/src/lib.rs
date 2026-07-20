//! `Daedalus` is an experimental open-source micro bootloader for
//! the LionsOS system developed ontop of seL4.
//!
//! Check out the [repository README](https://github.com/duplessisaurore/daedalus/blob/main/README.md)
//! for more information.
//!
//! ## Daedalus Caps
//!
//! The `daedalus_caps` crate provides the platform-specific set of
//! capabilities for the `Lepton3` boot programs for the daedalus
//! bootloader environment.

#![no_std]

extern crate alloc;

// Data migration from one heap allocator's data
// to another heap allocator to permit for capability
// calls with heap values.
pub mod migrate;
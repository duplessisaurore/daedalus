//! This module holds all of the IPC-related
//! capabilities and the `migration` function for in-daedalus
//! heap migration.

/// Data migration from one heap allocator's data
/// to another heap allocator to permit for capability
/// calls with heap values.
pub mod migrate;

/// The actual capabilities themselves which provide IPC
/// functionality
pub mod capabilities;

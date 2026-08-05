//! This defines the core architecture specific
//! trait that all architectures must implement
//! to inherently implement the IRQ operations
//! on that architecture.

/// A single architecture's IRQ operations
///
/// This supplies the boundary between arch-generic and arch-specific
/// IRQ operations that `Daedalus` provides an abstracted view over.
/// 
/// Nothing arch-specific should be at a "higher level" than this.
pub trait IrqArch {
    /// This is the total number of interrupts this architecture
    /// supports, each interrupt ID in this space will have a unique
    /// bit associated with it.
    const INTERUPT_IDS: usize;
}
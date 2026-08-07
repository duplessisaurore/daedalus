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
///
/// This type that impls the trait must have a const fn is_valid_irq
/// that takes a u32 and outputs a bool determining if the IRQ is valid.
pub trait IrqArch {
    /// This must exist (no const_trait_impl plz)
    /// pub const fn is_valid_irq(id: u32) -> bool { ... }
    /// but on the type.
    ///
    /// This function should call the const function. (it is not used,
    /// but a reminder).
    fn is_valid_irq(id: u32) -> bool;
}

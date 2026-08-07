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
    /// This is the opaque state permissable to the
    /// interrupt state saved from `disable_interrupts`
    /// which can be restored to with `restore_interrupts`
    type InterruptState;

    /// This must exist (no const_trait_impl plz)
    /// pub const fn is_valid_irq(id: u32) -> bool { ... }
    /// but on the type.
    ///
    /// This function should call the const function. (it is not used,
    /// but a reminder).
    fn is_valid_irq_wrapper(id: u32) -> bool;

    /// This should use the arch specific method to
    /// disable all interupts such that they can never
    /// fire and interrupt the current execution state of
    /// `Daedalus`.
    ///
    /// An example of this is by masking all interrupts on
    /// aarch64.
    ///
    /// This should (regardless of if there is any) return
    /// some `Self::InterruptState`.
    ///
    /// This `InterruptState` type represents the state of
    /// interrupts prior to disabling them, as if we mask
    /// all interrupts (or whatever arch specific method)
    /// then we may obliterate the state prior to disabling
    /// them.
    ///
    /// This is then passed later always into a `restore_interrupts`
    /// which should be the inverse of this and restore the
    /// prior state (if necessary).
    ///
    /// # Safety
    ///
    /// This assumes that the caller will properly pass the `InterruptState`
    /// returned back to a corresponding `restore_interrupts`.
    ///
    /// The caller must also not do any masking or restoring inbetween,
    /// as those will be obliterated on the `restore_interrupts` (unless
    /// intentional or smthn lmao)
    unsafe fn disable_interrupts() -> Self::InterruptState;

    /// This should use the arch specific method to
    /// restore all interupts such that they can fire
    /// again and remain in the same state as specified
    /// by the `InterruptState`.
    ///
    /// An example of this is by restoring all interrupt
    /// masks on aarch64.
    ///
    /// This `InterruptState` type represents the state of
    /// interrupts prior to disabling them, as if we mask
    /// all interrupts (or whatever arch specific method)
    /// then we may obliterate the state prior to disabling
    /// them.
    ///
    /// This `InterruptStatea` comes from some `disable_interrupts`.
    ///
    /// # Safety
    ///
    /// This assumes that the caller will properly have passed
    /// the `InterruptState` produced from a `disable_interrupts`.
    unsafe fn restore_interrupts(state: Self::InterruptState);
}

//! This defines the core architecture specific
//! trait that all architectures must implement
//! to inherently implement the IRQ operations
//! on that architecture.

use daedalus_program::{InterruptPriority, InterruptTrigger};

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

    /// Set up the interrupts on this architecture
    ///
    /// An example of some things that maybe required:
    ///     - Setting up the exception vector table.
    ///     - Configuring the interrupt controller
    ///
    /// # Safety
    ///
    /// Must be ran only exactly once during boot before
    /// any interrupt is registered by any `Daedalus` programs.
    ///
    /// (there must be no proceeding `IrqArch` operations).
    ///
    /// Must run after .bss is initialised/zerod (the pending tracker
    /// lives there)
    ///
    /// The caller must ensure that `Daedalus` only ever runs on one core at
    /// once.
    unsafe fn setup();

    /// Teardown all of the interrupt system that was modified
    /// throughout the running of `Daedalus`.
    ///
    /// This should generally do the inverse of any setup that was required,
    /// such that we tore down anything `Daedalus` setup so that we can hand off
    /// to the OS/kernel plainly.
    ///
    /// # Safety
    ///
    /// Must be ran only once, right before hand off.
    ///
    /// There must be no following `IrqArch` operations.
    ///
    /// The caller must ensure that `Daedalus` only ever runs on one core at
    /// once.
    unsafe fn teardown();

    /// Configure an interrupt.
    ///
    /// This should update the interrupt's trigger mode
    /// to the provided `trigger`, and update the priority
    /// to the specified `priority` level :3c
    ///
    /// This should also ensure that the interrupt is routed
    /// to the CPU that `Daedalus` is on (if that exists on the
    /// arch).
    ///
    /// # Safety
    ///
    /// It is assumed that `id` must satisfy this architecture's
    /// `is_valid_irq` function.
    ///
    /// This is also assumed to run bfore `unmask` for this interrupt
    /// to ensure that it doesn't get fired with a garbage priority/etc.
    ///
    /// This must be run after `setup` is called.
    unsafe fn configure(interrupt_id: u32, trigger: InterruptTrigger, priority: InterruptPriority);

    /// Prevent an interrupt from reaching `Daedalus`
    ///
    /// # Safety
    ///
    /// It is assumed that `id` must satisfy this architecture's
    /// `is_valid_irq` function.
    ///
    /// This must be run after `setup` is called.
    unsafe fn mask(interrupt_id: u32);

    /// Allow an interrupt from reaching `Daedalus`
    ///
    /// # Safety
    ///
    /// It is assumed that `id` must satisfy this architecture's
    /// `is_valid_irq` function.
    ///
    /// This must be run after `setup` is called.
    ///
    /// The caller should make sure the device's condition has
    /// been cleared first, else it will just refire...
    unsafe fn unmask(interrupt_id: u32);

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

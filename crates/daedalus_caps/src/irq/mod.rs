//! This module holds all of the irq
//! handling and creation caps across platforms
//! 
//! this module assumes only single core daedalus

use lepton3::lepton_vm::values::Tag;

/// A tag handle to a registered interrupt for a program
/// that the program can use to refer to a specific interrupt.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
pub struct IrqHandle(pub Tag);

/// A registrated binding of an IRQ to a program.
/// 
/// This should be referred to by some `IRQ` int id.
#[derive(Clone, Copy, Debug)]
pub struct IrqBinding {
    /// Which program receives messages for this
    pub program: &'static str,

    /// The tag that refers to this specific IRQ
    /// binding in the program's space.
    pub irq_handle: IrqHandle,
}
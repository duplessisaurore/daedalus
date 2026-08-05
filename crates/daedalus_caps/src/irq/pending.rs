//! The pending bitmap.
//! 
//! This is the bitmap responsible for how we associate
//! what irqs were triggered in the "normal context" after
//! finishing up handling an irq in the "irq context"
//! 
//! essentially
//! 
//! irq -> 
//!     set bitmap -> 
//!         ret to normal -> 
//!               normal check bitmap ->
//!                   fire irq msg to program
//! 
//! This only supports single-core execution, not SMP as
//! the data structure is inherently not atomic/exclusive.

/// How many bits to use for one "word" in the bitmap (one unit of
/// bit storage)
const BITMAP_WORD_SIZE: usize = 64;

/// The number of words in the bitmap of pending IRQ's
/// to be handled.
const PENDING_BITMAP_SIZE: usize = Arch::INTERUPT_IDS.div_ceil(BITMAP_WORD_SIZE);


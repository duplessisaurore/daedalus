//! All implemented architectures lie in this
//! submodule for IRQ ops
//!
//! This module exports based on the target_arch and
//! sometimes the controller the IRQ ops.

use daedalus_program::INTERRUPT_ARRAY;

cfg_if::cfg_if! {
    if #[cfg(all(target_arch = "aarch64", feature = "gicv2"))] {
        mod gicv2;
        pub use gicv2::GICv2 as TargetIRQArch;
    } else {
        compile_error!("no target IRQ architecture selected when IRQs were attempted to be used; enable an option");
    }
}

// Validate all of the interrupts are in the valid sest
// for TargetIRQArch

const fn validate_irqs_for_current_irq_arch() {
    let mut i = 0;

    while i < INTERRUPT_ARRAY.len() {
        let id = INTERRUPT_ARRAY[i];

        // Make sure the interrupt is valid for our target irq arch
        if !TargetIRQArch::is_valid_irq(id) {
            let _ = ["IRQ id not supported by the selected interrupt arch"; 0][id as usize]; // INVALID IRQ (see above as index is <invalid_irq_num>)
        }
        i += 1;
    }
}

const _: () = validate_irqs_for_current_irq_arch();

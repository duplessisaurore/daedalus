//! A set of capabilities provided to the `Lepton3` VM for daedalus
//! phases/services

use alloc::vec::Vec;
use daedalus_caps::program::DaedalusState;
use daedalus_caps::{
    ipc::capabilities as ipc, irq::capabilities as irq, memory::capabilities as mem,
};
use daedalus_program::{StaticDaedalusImageVariants, StaticSourceLocation};
use lepton3::{
    CapabilityFn,
    lepton_vm::{heap_allocator::HeapAllocator, tagger::TagGenerator},
};

/// This is the expected type for the `CapabilityFn`'s of `Daedalus`
/// that are provided by `daedalus_caps`
pub type DaedalusCapabilityFn<H, T> = CapabilityFn<
    'static,
    DaedalusState<StaticDaedalusImageVariants, H, T>,
    StaticSourceLocation,
    H,
    T,
    StaticDaedalusImageVariants,
>;

/// This returns the full set of `CapabilityFn`'s providedd by
/// `daedalus_caps`.
///
/// This includes: IPC, IRQs, Memory.
///
/// Awrrufff!
pub fn all<H: HeapAllocator, T: TagGenerator>() -> Vec<DaedalusCapabilityFn<H, T>> {
    // rust kinda struggles to infer the type, so we need to help it out
    let mut caps: Vec<DaedalusCapabilityFn<H, T>> = Vec::new();

    // IPC caps
    caps.push(ipc::cap_block_call);
    caps.push(ipc::cap_block_recv);
    caps.push(ipc::cap_caller_of);
    caps.push(ipc::cap_finish);
    caps.push(ipc::cap_is_replyable_tag);
    caps.push(ipc::cap_non_block_call);
    caps.push(ipc::cap_non_block_reply);
    caps.push(ipc::cap_yield_now);

    // Memory caps
    caps.push(mem::cap_mem_grant);
    caps.push(mem::cap_mem_derive);
    caps.push(mem::cap_mem_release);
    caps.push(mem::cap_mem_base);
    caps.push(mem::cap_mem_len);
    caps.push(mem::cap_mem_read);
    caps.push(mem::cap_mem_write);
    caps.push(mem::cap_is_region_handle);
    caps.push(mem::cap_mem_copy);
    caps.push(mem::cap_mem_fill);
    caps.push(mem::cap_mem_flush);

    // IRQ caps
    caps.push(irq::cap_is_irq_handle);
    caps.push(irq::cap_irq_release);
    caps.push(irq::cap_irq_ack);
    caps.push(irq::cap_irq_register);

    caps
}

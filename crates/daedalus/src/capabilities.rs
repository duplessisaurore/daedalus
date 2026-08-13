//! A set of capabilities provided to the `Lepton3` VM for daedalus
//! phases/services

use alloc::vec;
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
pub fn all<H: HeapAllocator, T: TagGenerator>() -> vec::Vec<DaedalusCapabilityFn<H, T>> {
    // rust kinda struggles to infer the type, so we need to help it out
    let caps: vec::Vec<DaedalusCapabilityFn<H, T>> = vec![
        // IPC caps
        ipc::cap_block_call,
        ipc::cap_block_recv,
        ipc::cap_caller_of,
        ipc::cap_finish,
        ipc::cap_is_replyable_tag,
        ipc::cap_non_block_call,
        ipc::cap_non_block_reply,
        ipc::cap_yield_now,
        // Memory caps
        mem::cap_mem_grant,
        mem::cap_mem_derive,
        mem::cap_mem_release,
        mem::cap_mem_base,
        mem::cap_mem_len,
        mem::cap_mem_read,
        mem::cap_mem_write,
        mem::cap_is_region_handle,
        mem::cap_mem_copy,
        mem::cap_mem_fill,
        mem::cap_mem_flush,
        // IRQ caps
        irq::cap_is_irq_handle,
        irq::cap_irq_release,
        irq::cap_irq_ack,
        irq::cap_irq_register,
    ];

    caps
}

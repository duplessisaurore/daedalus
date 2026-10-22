//! A null capability
//! 
//! Invalid to be called, but fills in spots for optional extensions

use core::error::Error;

use alloc::boxed::Box;
use lepton3::lepton_vm::{heap_allocator::HeapAllocator, tagger::TagGenerator};

use crate::{errors::DaedalusCapErrors, ipc::capabilities::DaedalusVm};


pub fn cap_null<H: HeapAllocator, T: TagGenerator>(
    _: &mut DaedalusVm<H, T>,
) -> Result<(), Box<dyn Error>> {
    Err(DaedalusCapErrors::InvokedNullCap)?
}
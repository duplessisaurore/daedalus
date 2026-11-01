//! This directory should for certain platforms
//! if more memory areas are required export a function that exports spans
//! of memory regions

cfg_if::cfg_if! {
    if #[cfg(all(target_arch = "aarch64", feature = "platform-zynqmp"))] {
        #[path = "zynqmp.rs"]
        pub mod zynqmp;
        pub use zynqmp::EXTRA_DEVICE_SPANS as SPANS;
    } else {
        compile_error!("no platform memory constants selected for this platform, if there are no expected ones, add an empty section!");
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct ExtraDeviceSpan {
    pub base: usize,
    pub len: usize
}

pub fn get_extra_device_spans_for_platform() -> &'static [ExtraDeviceSpan] {
    SPANS
}

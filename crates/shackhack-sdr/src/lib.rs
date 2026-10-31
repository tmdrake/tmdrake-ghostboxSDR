//! RTL-SDR capture, hardware window hops, and the radio engine thread.

mod engine;
mod source;

pub use engine::{start, Command, RadioHandle, Snapshot, VfoSnap};
pub use source::{DeviceInfo, SourceKind};

pub fn list_rtl_devices() -> Result<Vec<String>, String> {
    source::list_rtl_devices()
}

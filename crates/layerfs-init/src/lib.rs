//! Boot-time root assembly logic, factored out of `main.rs` so it can be
//! exercised by integration examples/tests without a real initramfs.

mod device_scan;
pub mod log;
mod luks;
pub mod migrate;
pub mod mount;
pub mod store;
pub mod switch_root;

//! Boot-time root assembly logic, factored out of `main.rs` so it can be
//! exercised by integration examples/tests without a real initramfs.

pub mod log;
pub mod mount;

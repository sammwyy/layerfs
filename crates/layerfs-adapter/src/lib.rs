//! Shared runner for package-manager adapters (dnf, apt, pacman, ...).
//! Each adapter crate supplies only its own verb classification.

mod adapter;
mod env;

pub use adapter::Adapter;
pub use env::bin_env_var;

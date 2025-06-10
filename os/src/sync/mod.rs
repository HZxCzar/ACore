//! Synchronization and interior mutability primitives

mod up;
pub mod mutex;

pub use up::UPSafeCell;

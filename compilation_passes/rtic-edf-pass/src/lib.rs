// Enable the `no_std` attribute if `no_std` is enabled
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
pub mod edf_pass;

#[cfg(feature = "std")]
pub use edf_pass::*;

pub mod critical_section;
pub mod scheduler;
pub mod task;
pub mod types;

pub mod export;

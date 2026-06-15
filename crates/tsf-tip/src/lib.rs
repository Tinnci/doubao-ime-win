#![cfg_attr(not(windows), allow(unused))]

#[cfg(windows)]
mod windows_tip;

#[cfg(windows)]
pub use windows_tip::*;

#[cfg(not(windows))]
pub const TIP_DESCRIPTION: &str = "Doubao Voice Input";

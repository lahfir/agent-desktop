#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

mod actions;
mod adapter;
mod input;
mod system;
mod tree;

pub use adapter::LinuxAdapter;

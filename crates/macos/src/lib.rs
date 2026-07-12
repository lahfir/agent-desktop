#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

mod actions;
mod adapter;
mod adapter_actions;
mod adapter_input;
mod adapter_observation;
mod adapter_system;
mod cf_type;
mod delivery_tracker;
mod input;
mod notifications;
mod system;
mod tree;

pub use adapter::MacOSAdapter;
pub use input::clipboard::helper_entry_from_env as clipboard_helper_from_env;
pub use system::cocoa_runtime::ensure_cocoa_multithreaded;
pub use system::permission_helper::entry_from_env as permission_prompt_helper_from_env;

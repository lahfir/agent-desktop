pub(crate) mod blocked_combo;
#[cfg(target_os = "windows")]
pub(crate) mod clipboard;
pub(crate) mod clipboard_bytes;
pub(crate) mod clipboard_files;
#[cfg(target_os = "windows")]
pub(crate) mod clipboard_formats;
#[cfg(target_os = "windows")]
pub(crate) mod clipboard_guard;
#[cfg(target_os = "windows")]
pub(crate) mod clipboard_image;
#[cfg(target_os = "windows")]
pub(crate) mod clipboard_session;
pub(crate) mod clipboard_text;
pub(crate) mod clipboard_worker_state;
#[cfg(target_os = "windows")]
pub(crate) mod clipboard_write;
pub(crate) mod drag;
mod drag_state;
pub(crate) mod elevation;
pub(crate) mod keyboard;
mod keyboard_event;
mod keyboard_map;
mod keyboard_send;
mod keyboard_text;
pub(crate) mod mouse;
mod mouse_click_guard;
mod mouse_coord;
mod mouse_modifier;
pub(crate) mod mouse_send;
mod release_state;

#[cfg(all(test, target_os = "windows"))]
pub(crate) use drag_state::DragReleaseGuard;
#[cfg(all(test, target_os = "windows"))]
pub(crate) use keyboard_event::KeyReleaseGuard;
#[cfg(all(test, target_os = "windows"))]
pub(crate) use keyboard_send::keyboard_send_fake_sink;
#[cfg(all(test, target_os = "windows"))]
pub(crate) use keyboard_text::{ensure_chunk_budget, preflight_text};
#[cfg(all(test, target_os = "windows"))]
pub(crate) use mouse_coord::NormalizedPoint;
#[cfg(all(test, target_os = "windows"))]
pub(crate) use mouse_send::mouse_send_fake_sink;

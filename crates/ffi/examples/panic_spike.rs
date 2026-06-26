//! Regression guard against a `panic = "abort"` regression in the `release-ffi`
//! Cargo profile.
//!
//! ## What this proves
//!
//! The `release-ffi` profile keeps `panic = "unwind"`, so
//! `std::panic::catch_unwind` — the same primitive that
//! `crate::ffi_try::trap_panic` relies on — remains effective under that
//! optimized profile.  If `panic = "abort"` were accidentally restored, this
//! example would SIGABRT instead of catching: a loud, immediate regression
//! signal.
//!
//! ## What this does NOT prove
//!
//! This example does NOT dlopen the shipped cdylib and trigger a panic inside
//! a real `ad_*` entrypoint's `trap_panic` fence.  A full cdylib-dlopen
//! panic-injection test is a tracked follow-up (Phase C+).
//!
//! ## Build and run
//!
//!     cargo run --profile release-ffi --example panic_spike -p agent-desktop-ffi
//!
//! Expected output: `PANIC CAUGHT OK (code = -1)`, exit code 0.

use std::panic::AssertUnwindSafe;

#[unsafe(no_mangle)]
pub extern "C" fn spike_panicking_entry() -> i32 {
    std::panic::catch_unwind(AssertUnwindSafe(|| -> i32 {
        panic!("synthetic panic inside extern C fn");
    }))
    .unwrap_or(-1)
}

fn main() {
    let code = spike_panicking_entry();
    if code == -1 {
        println!("PANIC CAUGHT OK (code = -1)");
        std::process::exit(0);
    } else {
        println!("UNEXPECTED: got code {}", code);
        std::process::exit(2);
    }
}

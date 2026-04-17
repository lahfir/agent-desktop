//! Regression example proving the custom `release-ffi` profile keeps
//! `catch_unwind` effective under the optimized cdylib build.
//!
//! Build and run with:
//!     cargo run --profile release-ffi --example panic_spike -p agent-desktop-ffi
//!
//! Expected: prints "PANIC CAUGHT OK (code = -1)" and exits with code 0.
//!
//! If someone accidentally flips `panic = "abort"` back on the `release-ffi`
//! profile (or removes the profile), this example will SIGABRT instead of
//! catching — a loud regression signal.

use std::panic::AssertUnwindSafe;

#[no_mangle]
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

//! Cargo's default install_name is the absolute build-machine path, which
//! breaks linking consumers (Swift/SPM, clang) once the dylib is extracted
//! from a release tarball. @rpath is the portable form.
//!
//! # panic=abort guard (Finding #14)
//!
//! `trap_panic` (the `catch_unwind` fence in `src/ffi.rs`) requires
//! `panic = "unwind"` to function; building with `panic = "abort"` silently
//! degrades hostile-input panics to process abort.  The `release-ffi` profile
//! enforces `panic = "unwind"` and is the only profile used to ship the cdylib.
//!
//! A build-time check via `CARGO_CFG_PANIC` is not viable here: because this
//! crate declares both `cdylib` and `rlib` crate-types, Cargo always reports
//! `CARGO_CFG_PANIC = "unwind"` in the build script regardless of the active
//! profile's `panic` setting.  The invariant is therefore documented rather
//! than machine-enforced; CI gates the cdylib via `--profile release-ffi` only.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-install_name,@rpath/libagent_desktop_ffi.dylib");
    }
}

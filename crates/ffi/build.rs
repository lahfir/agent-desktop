//! Cargo's default install_name is the absolute build-machine path, which
//! breaks linking consumers (Swift/SPM, clang) once the dylib is extracted
//! from a release tarball. @rpath is the portable form.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-install_name,@rpath/libagent_desktop_ffi.dylib");
    }
}

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=cbindgen.toml");
    println!("cargo:rerun-if-changed=src/");

    // Cargo's default install_name is the absolute build-machine path,
    // which breaks linking consumers (Swift/SPM, clang) once the dylib
    // is extracted from a release tarball. @rpath is the portable form.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-install_name,@rpath/libagent_desktop_ffi.dylib");
    }

    let crate_dir = match env::var("CARGO_MANIFEST_DIR") {
        Ok(d) => d,
        Err(_) => {
            println!("cargo:warning=CARGO_MANIFEST_DIR not set; skipping cbindgen");
            return;
        }
    };
    let out_dir = match env::var("OUT_DIR") {
        Ok(d) => d,
        Err(_) => {
            println!("cargo:warning=OUT_DIR not set; skipping cbindgen");
            return;
        }
    };

    let config = match cbindgen::Config::from_file("cbindgen.toml") {
        Ok(c) => c,
        Err(err) => panic!("cbindgen.toml parse error: {}", err),
    };

    let out_path = Path::new(&out_dir).join("agent_desktop.h");

    let bindings = match cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .generate()
    {
        Ok(b) => b,
        Err(err) => panic!("cbindgen generation failed: {}", err),
    };

    bindings.write_to_file(&out_path);
    if !out_path.exists() {
        panic!("cbindgen produced no header at {:?}", out_path);
    }

    // Deliberately NOT copying into crates/ffi/include/ — the committed
    // header is the ABI contract. Auto-copy would self-heal CI drift checks.
    // Developers refresh it via scripts/update-ffi-header.sh.

    // Stamp the absolute path of the generated header at a stable location
    // so CI and scripts don't have to `find target | head -1` (ambiguous
    // under warm caches with multiple pkg-hash dirs).
    if let Some(target_root) = target_root_from_out_dir(Path::new(&out_dir)) {
        let stamp = target_root.join("ffi-header-path.txt");
        if let Err(err) = std::fs::write(&stamp, out_path.to_string_lossy().as_bytes()) {
            println!(
                "cargo:warning=failed to stamp FFI header path at {:?}: {}",
                stamp, err
            );
        }
    } else {
        println!(
            "cargo:warning=could not infer cargo target root from OUT_DIR={}; skipping ffi-header-path.txt",
            out_dir
        );
    }
}

fn target_root_from_out_dir(out_dir: &Path) -> Option<PathBuf> {
    // OUT_DIR = .../target/<profile>/build/<pkg-hash>/out
    let mut current = out_dir;
    for _ in 0..4 {
        current = current.parent()?;
    }
    Some(current.to_path_buf())
}

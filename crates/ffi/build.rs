use std::env;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=cbindgen.toml");
    println!("cargo:rerun-if-changed=src/");

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

    // cbindgen 0.27 returns `false` when the header file content hasn't changed and the
    // existing file already matches — that's a clean no-op, not an error. We just need a
    // valid file at `out_path` for downstream build consumers.
    bindings.write_to_file(&out_path);
    if !out_path.exists() {
        panic!("cbindgen produced no header at {:?}", out_path);
    }

    // NOTE: We intentionally do NOT copy the generated header back into
    // `crates/ffi/include/`. That committed header is the ABI contract
    // checked into source control. CI's "FFI header drift check" compares
    // $OUT_DIR/agent_desktop.h against the committed copy; if the build
    // script auto-copied it, the drift check would self-heal instead of
    // catching stale headers. Developers update the committed header by
    // running `scripts/update-ffi-header.sh`.
}

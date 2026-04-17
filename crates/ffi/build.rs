use std::env;
use std::path::{Path, PathBuf};

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

    // Stamp the absolute path to the just-generated header at a stable,
    // deterministic location that CI and scripts can read without ever
    // resorting to `find target | head -1` (which picks arbitrarily when
    // multiple `agent-desktop-ffi-<hash>/` build dirs are cached). Walking
    // 4 parents up from OUT_DIR yields the cargo target root:
    //   {target}/{profile}/build/{pkg-hash}/out  →  {target}
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

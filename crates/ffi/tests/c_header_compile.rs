//! Verifies the committed FFI header compiles as C and that every named
//! enum discriminant documented in the header is usable from C code. This
//! guards against cbindgen configuration drift that would silently drop
//! the `AdActionKind` / `AdDirection` / etc. enum blocks, forcing C
//! consumers to hard-code numeric literals instead of AD_* constants.
//!
//! The test shells out to the system `cc`; it skips on platforms where
//! that binary is not on PATH so cargo test still passes on bare CI
//! images.

use std::path::PathBuf;
use std::process::Command;

fn system_cc() -> Option<&'static str> {
    let cc = if cfg!(target_os = "windows") {
        "cl"
    } else {
        "cc"
    };
    Command::new(cc)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|_| cc)
}

fn header_include_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("include");
    p
}

#[test]
fn committed_header_compiles_with_every_public_enum_constant() {
    let cc = match system_cc() {
        Some(cc) => cc,
        None => {
            eprintln!("skipping: system C compiler not found");
            return;
        }
    };

    let tmp = std::env::temp_dir().join("agent_desktop_header_abi_test.c");
    let obj = std::env::temp_dir().join("agent_desktop_header_abi_test.o");
    // Touch every named-constant family so a missing enum block in the
    // header fails compilation with "undeclared identifier". Keeping the
    // list close to cbindgen.toml's [export].include ensures they stay
    // in sync: add an entry there, add its representative here.
    let src = r#"
#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
#include "agent_desktop.h"

int main(void) {
    (void)AD_ACTION_KIND_CLICK;
    (void)AD_DIRECTION_UP;
    (void)AD_MODIFIER_CMD;
    (void)AD_MOUSE_BUTTON_LEFT;
    (void)AD_MOUSE_EVENT_KIND_MOVE;
    (void)AD_SCREENSHOT_KIND_FULL_SCREEN;
    (void)AD_SNAPSHOT_SURFACE_WINDOW;
    (void)AD_WINDOW_OP_KIND_RESIZE;
    (void)AD_IMAGE_FORMAT_PNG;
    (void)AD_RESULT_OK;
    return 0;
}
"#;
    std::fs::write(&tmp, src).expect("write test translation unit");

    let include = header_include_dir();
    let status = Command::new(cc)
        .arg("-Wall")
        .arg("-Werror")
        .arg("-I")
        .arg(&include)
        .arg("-c")
        .arg(&tmp)
        .arg("-o")
        .arg(&obj)
        .status()
        .expect("cc invocation failed");

    let _ = std::fs::remove_file(&tmp);
    let _ = std::fs::remove_file(&obj);

    assert!(
        status.success(),
        "C compile of agent_desktop.h failed — a named enum constant is missing. \
         Check crates/ffi/cbindgen.toml [export].include."
    );
}

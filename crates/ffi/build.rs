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
//!
//! # FFI command codegen
//!
//! Family-B wrappers (command-backed, JSON-returning) are generated from
//! per-command template files under `codegen_templates/`. Each file is named
//! `<command>.rs.in` and loaded via `include_str!` in `command_templates()`.
//! The template set is the single source of truth for the command universe.
//!
//! Adding a new Family-B command requires exactly these steps:
//! 1. Create `codegen_templates/<name>.rs.in` with the wrapper body.
//! 2. Add `m.insert("<name>", include_str!("codegen_templates/<name>.rs.in"))` to
//!    `command_templates()` in this file.
//! 3. Add `"<name>"` to `EXPECTED_COMMANDS` in
//!    `tests/codegen_exhaustiveness.rs`.
//!
//! The build fails at step 3 if steps 1 and 2 are done without step 3.
//! An orphan `.rs.in` file (present in the dir but not in the map) triggers
//! a build-time panic so stale templates cannot silently accumulate.

use std::collections::BTreeMap;
use std::path::PathBuf;

const GENERATED_HEADER_STATIC: &str = "\
//! @generated — produced by crates/ffi/build.rs codegen.
//! Edit the templates under crates/ffi/codegen_templates/, not this file.
";

/// Map from command name to its wrapper template (alphabetical by convention).
///
/// This map is the single source of truth for which Family-B commands exist.
/// Every `.rs.in` file in `codegen_templates/` must appear here; the build
/// panics otherwise.
fn command_templates() -> BTreeMap<&'static str, &'static str> {
    let mut m = BTreeMap::new();
    m.insert(
        "execute_by_ref",
        include_str!("codegen_templates/execute_by_ref.rs.in"),
    );
    m.insert("snapshot", include_str!("codegen_templates/snapshot.rs.in"));
    m.insert("status", include_str!("codegen_templates/status.rs.in"));
    m.insert("version", include_str!("codegen_templates/version.rs.in"));
    m.insert("wait", include_str!("codegen_templates/wait.rs.in"));
    m
}

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-install_name,@rpath/libagent_desktop_ffi.dylib");
    }

    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));

    let templates_dir = manifest_dir.join("codegen_templates");
    let generated_path = manifest_dir.join("src/commands/generated.rs");

    let templates = command_templates();

    for entry in std::fs::read_dir(&templates_dir)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", templates_dir.display()))
    {
        let entry = entry.unwrap_or_else(|e| panic!("dir entry error in codegen_templates: {e}"));
        let fname = entry.file_name();
        let fname_str = fname.to_string_lossy();
        if let Some(name) = fname_str.strip_suffix(".rs.in") {
            if !templates.contains_key(name) {
                panic!(
                    "orphan template file `{fname_str}` found in codegen_templates/ but not \
                     registered in `command_templates()` in build.rs — add an entry or delete \
                     the file."
                );
            }
        }
    }

    for name in templates.keys() {
        println!(
            "cargo:rerun-if-changed={}",
            templates_dir.join(format!("{name}.rs.in")).display()
        );
    }

    let command_list = templates.keys().copied().collect::<Vec<_>>().join(", ");
    let dynamic_comment = format!("//! Commands in alphabetical order: {command_list}.\n");

    let mut output = String::from(GENERATED_HEADER_STATIC);
    output.push_str(&dynamic_comment);

    let use_block = "\
\nuse crate::AdAdapter;\
\nuse crate::actions::conversion::action_from_c;\
\nuse crate::commands::app_error_to_adapter;\
\nuse crate::commands::envelope_out::write_command_envelope;\
\nuse crate::convert::string::{\
\n    decode_optional_filter, optional_adapter_string, required_adapter_string,\
\n};\
\nuse crate::convert::surface::snapshot_surface_from_c;\
\nuse crate::error::{self, AdResult, set_last_error};\
\nuse crate::ffi_try::trap_panic;\
\nuse crate::main_thread::require_main_thread;\
\nuse crate::pointer_guard::guard_non_null;\
\nuse crate::types::wait_args::AdWaitArgs;\
\nuse crate::types::{AdAction, AdPolicyKind};\
\nuse agent_desktop_core::commands::snapshot::SnapshotArgs;\
\nuse agent_desktop_core::commands::status::execute_with_report_with_context;\
\nuse agent_desktop_core::commands::wait::{WaitArgs, WaitModeArgs, WaitPredicateArgs};\
\nuse agent_desktop_core::error::{AdapterError, AppError, ErrorCode};\
\nuse agent_desktop_core::refs::validate_ref_id;\
\nuse std::ffi::c_char;\
\nuse std::ptr;\n";

    output.push_str(use_block);

    for name in templates.keys() {
        output.push_str(templates[name]);
    }

    let existing = std::fs::read_to_string(&generated_path).unwrap_or_default();
    if existing != output {
        std::fs::write(&generated_path, &output)
            .unwrap_or_else(|e| panic!("failed to write {}: {e}", generated_path.display()));
    }
}

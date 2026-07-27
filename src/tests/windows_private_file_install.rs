#![cfg(target_os = "windows")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

static SCRATCH_ID: AtomicU64 = AtomicU64::new(1);

struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn create(label: &str) -> Self {
        let id = SCRATCH_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "agent-desktop-install-{label}-{}-{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create scratch root");
        Self { root }
    }

    fn dir(&self, name: &str) -> PathBuf {
        let path = self.root.join(name);
        std::fs::create_dir_all(&path).expect("create scratch subdirectory");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn plant_junction(link: &Path, target: &Path) {
    let output = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .expect("cmd /c mklink starts");
    assert!(
        output.status.success(),
        "mklink /J must succeed without privilege: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let attributes = {
        use std::os::windows::fs::MetadataExt;
        std::fs::symlink_metadata(link)
            .expect("junction link exists")
            .file_attributes()
    };
    assert!(
        attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0,
        "planted link must carry FILE_ATTRIBUTE_REPARSE_POINT"
    );
}

fn run_session_start(home: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_agent-desktop"))
        .args(["session", "start"])
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env_remove("AGENT_DESKTOP_SESSION")
        .output()
        .expect("binary starts")
}

fn parse_envelope(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).expect("stdout is one JSON envelope")
}

fn regular_files_under(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                pending.push(entry.path());
            } else {
                files.push(entry.path());
            }
        }
    }
    files
}

#[test]
fn session_start_through_a_junction_home_is_refused_by_the_installed_windows_ops() {
    let scratch = Scratch::create("junction");
    let home = scratch.dir("junction-home");
    let target = scratch.dir("junction-target");
    plant_junction(&home.join(".agent-desktop"), &target);

    let output = run_session_start(&home);
    let envelope = parse_envelope(&output);

    assert_eq!(
        output.status.code(),
        Some(1),
        "session start must fail structurally when ~/.agent-desktop is a junction; \
         success means the portable default wrote through the junction"
    );
    assert_eq!(envelope["ok"], false);
    let leaked = regular_files_under(&target);
    assert!(
        leaked.is_empty(),
        "no session artifact may land under the junction target: {leaked:?}"
    );
}

#[test]
fn session_start_in_a_real_home_succeeds_as_the_junction_control() {
    let scratch = Scratch::create("control");
    let home = scratch.dir("real-home");

    let output = run_session_start(&home);
    let envelope = parse_envelope(&output);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(envelope["ok"], true);
    let session_id = envelope["data"]["session_id"]
        .as_str()
        .expect("session start reports its session id");
    let manifest = home
        .join(".agent-desktop")
        .join("sessions")
        .join(session_id)
        .join("session.json");
    assert!(
        manifest.is_file(),
        "the control session manifest must exist under the real home"
    );
}

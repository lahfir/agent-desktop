use super::*;

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "agent-desktop-{name}-{}-{}",
        std::process::id(),
        crate::refs::new_snapshot_id()
    ));
    crate::private_file_parent::ensure_private(&dir).expect("private scratch directory");
    dir
}

#[test]
fn appended_private_files_are_lockable() {
    let path = scratch_dir("append-lock").join("trace.jsonl");

    let file = open_private_append(&path).expect("append handle");
    file.lock()
        .expect("append handles must be lockable on every platform");
    file.unlock().expect("unlock");

    drop(file);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn append_handles_reopen_and_relock_after_close() {
    let path = scratch_dir("append-relock").join("trace.jsonl");

    for _ in 0..2 {
        let file = open_private_append(&path).expect("append handle");
        file.lock().expect("lock");
        file.unlock().expect("unlock");
    }

    let _ = std::fs::remove_file(&path);
}

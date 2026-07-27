use super::*;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Mutex;

struct RecordingOps {
    calls: Mutex<Vec<&'static str>>,
}

impl RecordingOps {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            calls: Mutex::new(Vec::new()),
        })
    }

    fn record(&self, call: &'static str) {
        self.calls.lock().unwrap().push(call);
    }

    fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().unwrap().clone()
    }
}

impl PrivateFileOps for RecordingOps {
    fn write_atomic(&self, _path: &Path, _bytes: &[u8]) -> std::io::Result<()> {
        self.record("write_atomic");
        Ok(())
    }

    fn open_private_append(&self, _path: &Path) -> std::io::Result<File> {
        self.record("open_private_append");
        Err(std::io::Error::new(ErrorKind::Unsupported, "fake append"))
    }

    fn open_private_lock(&self, _path: &Path, _create: bool) -> std::io::Result<File> {
        self.record("open_private_lock");
        Err(std::io::Error::new(ErrorKind::Unsupported, "fake lock"))
    }

    fn read_private_bounded(&self, _path: &Path, _max_bytes: u64) -> std::io::Result<Vec<u8>> {
        self.record("read_private_bounded");
        Ok(b"routed".to_vec())
    }

    fn ensure_private(&self, _path: &Path) -> std::io::Result<()> {
        self.record("ensure_private");
        Ok(())
    }
}

fn untouched_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "agent-desktop-ops-{name}-{}-{}",
        std::process::id(),
        crate::refs::new_snapshot_id()
    ))
}

fn scratch_directory(name: &str) -> PathBuf {
    let directory = untouched_path(name);
    crate::private_file_parent::ensure_private(&directory).unwrap();
    directory
}

#[test]
fn every_primitive_routes_through_the_overriding_ops() {
    let missing = untouched_path("route-five");
    let recorder = RecordingOps::new();

    with_test_ops_override(recorder.clone(), || {
        crate::private_file::write_atomic(&missing, b"bytes").unwrap();
        let append_error = crate::private_file::open_private_append(&missing).unwrap_err();
        assert_eq!(append_error.kind(), ErrorKind::Unsupported);
        let lock_error = crate::private_file::open_private_lock(&missing, true).unwrap_err();
        assert_eq!(lock_error.kind(), ErrorKind::Unsupported);
        assert_eq!(
            crate::private_file::read_private_bounded(&missing, 8).unwrap(),
            b"routed"
        );
        crate::private_file_parent::ensure_private(&missing).unwrap();
    });

    assert_eq!(
        recorder.calls(),
        vec![
            "write_atomic",
            "open_private_append",
            "open_private_lock",
            "read_private_bounded",
            "ensure_private",
        ]
    );
    assert!(!missing.exists());
}

#[test]
fn the_override_is_scoped_to_its_thread() {
    let recorder = RecordingOps::new();

    with_test_ops_override(recorder.clone(), || {
        let directory = std::thread::spawn(|| {
            let directory = scratch_directory("other-thread");
            crate::private_file::write_atomic(&directory.join("data"), b"portable").unwrap();
            directory
        })
        .join()
        .unwrap();

        assert_eq!(std::fs::read(directory.join("data")).unwrap(), b"portable");
        assert_eq!(
            crate::private_file::read_private_bounded(&directory.join("data"), 16).unwrap(),
            b"routed"
        );
        std::fs::remove_dir_all(directory).unwrap();
    });

    assert_eq!(recorder.calls(), vec!["read_private_bounded"]);
}

#[test]
fn portable_behavior_resumes_after_the_scoped_override() {
    let directory = scratch_directory("restore");
    let path = directory.join("data");
    let recorder = RecordingOps::new();

    with_test_ops_override(recorder.clone(), || {
        crate::private_file::write_atomic(&path, b"faked").unwrap();
    });

    assert!(!path.exists());
    crate::private_file::write_atomic(&path, b"portable").unwrap();
    assert_eq!(
        crate::private_file::read_private_bounded(&path, 16).unwrap(),
        b"portable"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_panicking_override_scope_still_restores_portable_behavior() {
    let directory = scratch_directory("panic-restore");
    let path = directory.join("data");
    let recorder = RecordingOps::new();

    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_test_ops_override(recorder, || panic!("scope failure"));
    }));

    assert!(panicked.is_err());
    crate::private_file::write_atomic(&path, b"portable").unwrap();
    assert_eq!(
        crate::private_file::read_private_bounded(&path, 16).unwrap(),
        b"portable"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn installing_ops_a_second_time_is_rejected() {
    assert!(install_private_file_ops(Box::new(PortablePrivateFileOps)).is_ok());
    assert!(install_private_file_ops(Box::new(PortablePrivateFileOps)).is_err());
}

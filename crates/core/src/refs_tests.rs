use super::*;
use crate::refs_test_support::HomeGuard;

fn entry(role: &str, name: Option<&str>) -> RefEntry {
    RefEntry {
        pid: 1,
        role: role.into(),
        name: name.map(String::from),
        value: None,
        description: None,
        states: vec![],
        bounds: None,
        bounds_hash: None,
        available_actions: vec!["Click".into()],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    }
}

#[test]
fn test_allocate_sequential() {
    let mut map = RefMap::new();
    let r1 = map.allocate(entry("button", Some("OK")));
    let r2 = map.allocate(entry("button", Some("OK")));
    assert_eq!(r1, "@e1");
    assert_eq!(r2, "@e2");
    assert_eq!(map.len(), 2);
}

#[test]
fn test_get_existing() {
    let mut map = RefMap::new();
    let ref_id = map.allocate(RefEntry {
        pid: 42,
        role: "textfield".into(),
        name: None,
        value: None,
        description: None,
        states: vec![],
        bounds: None,
        bounds_hash: Some(12345),
        available_actions: vec![],
        source_app: Some("Finder".into()),
        source_window_id: None,
        source_window_title: Some("Documents".into()),
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    });
    let retrieved = map.get(&ref_id).unwrap();
    assert_eq!(retrieved.pid, 42);
    assert_eq!(retrieved.role, "textfield");
}

#[test]
fn test_get_missing() {
    let map = RefMap::new();
    assert!(map.get("@e99").is_none());
}

#[test]
fn test_validate_ref_id_accepts_positive_element_refs() {
    assert!(validate_ref_id("@e1").is_ok());
    assert!(validate_ref_id("@e14").is_ok());
    assert!(validate_ref_id("@e999").is_ok());
}

#[test]
fn test_validate_ref_id_rejects_malformed_refs() {
    assert!(validate_ref_id("@").is_err());
    assert!(validate_ref_id("e1").is_err());
    assert!(validate_ref_id("@e").is_err());
    assert!(validate_ref_id("@e0").is_err());
    assert!(validate_ref_id("@e0abc").is_err());
    assert!(validate_ref_id("1").is_err());
    assert!(validate_ref_id("").is_err());
}

#[test]
fn test_remove_by_root_ref() {
    let mut map = RefMap::new();
    let base = entry("button", Some("OK"));

    map.allocate(base.clone());

    let drilled = RefEntry {
        root_ref: Some("@e1".into()),
        ..base
    };
    map.allocate(drilled.clone());
    map.allocate(drilled);
    assert_eq!(map.len(), 3);

    map.remove_by_root_ref("@e1");
    assert_eq!(map.len(), 1);
    assert!(map.get("@e1").is_some());
}

#[test]
fn test_counter_continues_after_skeleton_into_drill_down() {
    let mut map = RefMap::new();
    let skeleton_entry = entry("button", Some("Skeleton"));

    let last_skeleton = (0..10)
        .map(|_| map.allocate(skeleton_entry.clone()))
        .last()
        .unwrap();
    assert_eq!(last_skeleton, "@e10");

    let drilled = RefEntry {
        root_ref: Some("@e3".into()),
        ..skeleton_entry
    };

    let first_drilled = map.allocate(drilled.clone());
    let second_drilled = map.allocate(drilled);
    assert_eq!(
        first_drilled, "@e11",
        "counter should continue past skeleton ids, not reset"
    );
    assert_eq!(second_drilled, "@e12");
    assert_eq!(map.len(), 12);

    map.remove_by_root_ref("@e3");
    assert_eq!(
        map.len(),
        10,
        "scoped invalidation should drop only the drill-down refs"
    );
    assert!(map.get("@e3").is_some(), "skeleton @e3 must survive");
}

#[test]
fn test_root_ref_serde_roundtrip() {
    let entry = RefEntry {
        root_ref: Some("@e5".into()),
        ..entry("button", None)
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("root_ref"));
    let back: RefEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(back.root_ref.as_deref(), Some("@e5"));
}

#[test]
fn test_serialize_with_size_check_rejects_oversized() {
    let mut map = RefMap::new();
    let big_name = "x".repeat(2048);
    for _ in 0..600 {
        map.allocate(entry("button", Some(&big_name)));
    }

    let result = map.serialize_with_size_check();
    assert!(result.is_err(), "oversized refmap should be rejected");
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("1MB"),
        "error should mention the 1MB limit, got: {msg}"
    );
}

#[test]
fn test_serialize_with_size_check_accepts_normal() {
    let mut map = RefMap::new();
    for _ in 0..50 {
        map.allocate(entry("button", Some("OK")));
    }

    let result = map.serialize_with_size_check();
    assert!(result.is_ok(), "normal-sized refmap should serialize");
}

#[test]
fn test_save_load_roundtrip_with_home_override() {
    let _guard = HomeGuard::new();
    let mut map = RefMap::new();
    map.allocate(RefEntry {
        pid: 7,
        role: "button".into(),
        name: Some("Send".into()),
        value: None,
        description: None,
        states: vec![],
        bounds: None,
        bounds_hash: Some(42),
        available_actions: vec!["Click".into()],
        source_app: Some("TestApp".into()),
        source_window_id: None,
        source_window_title: Some("Test Window".into()),
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    });
    map.save().expect("save should succeed under HomeGuard");

    let loaded = RefMap::load().expect("load should succeed");
    assert_eq!(loaded.len(), 1);
    let entry = loaded.get("@e1").unwrap();
    assert_eq!(entry.pid, 7);
    assert_eq!(entry.name.as_deref(), Some("Send"));
}

#[test]
fn test_snapshot_ids_are_compact_and_valid() {
    let id = new_snapshot_id();

    assert!(id.starts_with('s'));
    assert!(id.len() <= 14, "snapshot id should stay token-light: {id}");
    validate_snapshot_id(&id).unwrap();
}

#[test]
fn test_base36_encoding() {
    assert_eq!(base36(0), "0000");
    assert_eq!(base36(35), "000z");
    assert_eq!(base36(36), "0010");
    assert_eq!(base36(36 * 36 * 36 + 35), "100z");
    assert!(base36(u64::MAX).len() >= 4);
}

#[test]
fn test_new_snapshot_id_passes_validation() {
    for _ in 0..256 {
        let id = new_snapshot_id();
        validate_snapshot_id(&id).expect("generated snapshot id must validate");
    }
}

#[test]
fn test_validate_snapshot_id_rejects_bad_values() {
    assert!(validate_snapshot_id("").is_err());
    assert!(validate_snapshot_id("s").is_err());
    assert!(validate_snapshot_id(&format!("s{}", "x".repeat(64))).is_err());
    assert!(validate_snapshot_id("bad/id").is_err());
}

#[test]
fn test_save_oversize_preserves_previous_file() {
    let _guard = HomeGuard::new();

    let mut original = RefMap::new();
    original.allocate(entry("button", Some("Original")));
    original.save().expect("baseline save");

    let mut oversize = RefMap::new();
    let big = "x".repeat(2048);
    for _ in 0..600 {
        oversize.allocate(entry("button", Some(&big)));
    }
    let result = oversize.save();
    assert!(result.is_err(), "oversize save must reject");

    let reloaded = RefMap::load().expect("previous file must still load");
    assert_eq!(reloaded.len(), 1);
    let entry = reloaded.get("@e1").unwrap();
    assert_eq!(entry.name.as_deref(), Some("Original"));
}

#[cfg(unix)]
#[test]
fn test_write_private_file_rejects_tmp_symlink() {
    let dir = std::env::temp_dir().join(format!(
        "agent-desktop-ref-symlink-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("refmap.json");
    let target = dir.join("target.json");
    let tmp = path.with_extension("tmp");
    std::fs::write(&target, b"existing").unwrap();
    std::os::unix::fs::symlink(&target, &tmp).unwrap();

    let result = write_private_file(&path, b"new");

    assert!(result.is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"existing");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_root_ref_none_omitted() {
    let entry = entry("button", None);
    let json = serde_json::to_string(&entry).unwrap();
    assert!(!json.contains("root_ref"));
}

use super::*;

#[test]
fn list_returns_known_skills() {
    let v = list().expect("list");
    let arr = v["skills"].as_array().expect("array");
    assert!(arr.iter().any(|s| s["name"] == "agent-desktop"));
    assert!(arr.iter().any(|s| s["name"] == "agent-desktop-ffi"));
}

#[test]
fn get_resolves_alias() {
    let v = get(GetArgs {
        name: "desktop".into(),
        full: false,
        reference: None,
    })
    .expect("get");
    assert_eq!(v["skill"], "agent-desktop");
    assert!(v["content"].as_str().unwrap().contains("agent-desktop"));
}

#[test]
fn get_full_inlines_references() {
    let v = get(GetArgs {
        name: "desktop".into(),
        full: true,
        reference: None,
    })
    .expect("get full");
    let content = v["content"].as_str().expect("string");
    assert!(content.contains("--- references/workflows.md ---"));
    assert!(content.contains("--- references/macos.md ---"));
    assert!(content.contains("@s8f3k2p9:e1"));
    assert!(content.contains("session start` does not activate later processes"));
    assert!(content.contains("session-owned ref still requires the same `--session`"));
    assert!(content.contains("bounded isolated helper"));
    assert!(!content.contains("~/.agent-desktop/current_session"));
    assert!(!content.contains("resolves cross-session"));
    assert!(!content.contains("snapshot IDs do not require also passing `--session`"));
}

#[test]
fn ffi_skill_defines_ref_and_concurrency_contracts() {
    let v = get(GetArgs {
        name: "ffi".into(),
        full: true,
        reference: None,
    })
    .expect("get full ffi");
    let content = v["content"].as_str().expect("string");
    assert!(content.contains("## Ref token validation"));
    assert!(content.contains("read + mutation"));
    assert!(content.contains("## Off-main-thread migration"));
}

#[test]
fn get_specific_reference() {
    let v = get(GetArgs {
        name: "desktop".into(),
        full: false,
        reference: Some("workflows".into()),
    })
    .expect("get ref");
    assert_eq!(v["reference"], "references/workflows.md");
}

#[test]
fn unknown_skill_errors() {
    let err = get(GetArgs {
        name: "nope".into(),
        full: false,
        reference: None,
    })
    .expect_err("should error");
    assert!(format!("{err}").contains("Unknown skill"));
}

#[test]
fn path_lists_canonical_names() {
    let v = path().expect("path");
    assert_eq!(v["location"], "embedded");
    let avail = v["available"].as_array().expect("arr");
    assert!(avail.iter().any(|s| s == "agent-desktop"));
}

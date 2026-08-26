use super::*;
use std::collections::BTreeSet;
use std::path::Path;

#[test]
fn list_returns_known_skills() {
    let v = list().expect("list");
    let arr = v["skills"].as_array().expect("array");
    assert!(arr.iter().any(|s| s["name"] == "agent-desktop"));
    assert!(arr.iter().any(|s| s["name"] == "agent-desktop-ffi"));
    assert!(arr.iter().any(|s| s["name"] == "agent-desktop-windows"));
}

#[test]
fn windows_skill_serves_main_and_references() {
    let body = get(GetArgs {
        name: "windows".into(),
        full: false,
        reference: None,
    })
    .expect("get windows");
    assert_eq!(body["skill"], "agent-desktop-windows");
    assert!(
        body["content"]
            .as_str()
            .expect("string")
            .contains("Capability Table")
    );

    let reference = get(GetArgs {
        name: "agent-desktop-windows".into(),
        full: false,
        reference: Some("permissions-and-elevation.md".into()),
    })
    .expect("get windows reference");
    assert_eq!(
        reference["reference"],
        "references/permissions-and-elevation.md"
    );

    let err = get(GetArgs {
        name: "windows".into(),
        full: false,
        reference: Some("nope.md".into()),
    })
    .expect_err("unknown reference should error");
    assert!(format!("{err}").contains("Unknown reference"));
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
    if cfg!(target_os = "macos") {
        assert!(content.contains("--- references/macos.md ---"));
    }
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

#[test]
fn every_skill_markdown_on_disk_is_served_from_the_skills_table() {
    let served = served_skill_paths();
    let found = walk_markdowns(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../skills"));

    let unserved: Vec<&String> = found
        .iter()
        .filter(|path| !served.contains(*path) && !allowed_unserved(path))
        .collect();
    assert!(
        unserved.is_empty(),
        "skill files exist on disk but the SKILLS table never serves them \
         (wire them into skills.rs): {unserved:?}"
    );
}

fn served_skill_paths() -> BTreeSet<String> {
    let mut served = BTreeSet::new();
    for skill in SKILLS {
        served.insert(format!("{}/SKILL.md", skill.canonical));
        for reference in skill.refs {
            served.insert(format!("{}/{}", skill.canonical, reference.rel_path));
        }
    }
    served
}

fn allowed_unserved(rel_path: &str) -> bool {
    rel_path == "agent-desktop/references/macos.md" && !cfg!(target_os = "macos")
}

fn walk_markdowns(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    walk_markdowns_inner(root, root, &mut found);
    found.sort();
    found
}

fn walk_markdowns_inner(root: &Path, dir: &Path, found: &mut Vec<String>) {
    let entries = std::fs::read_dir(dir).expect("skills directory must be readable");
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            walk_markdowns_inner(root, &path, found);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            let rel = path
                .strip_prefix(root)
                .expect("walked path stays under skills root")
                .to_string_lossy()
                .replace('\\', "/");
            found.push(rel);
        }
    }
}

#[test]
fn shipped_first_time_setup_names_both_platforms() {
    let section = first_time_setup_section(SKILL_DESKTOP_REF_WORKFLOWS);
    assert!(
        section.contains("macOS"),
        "setup must keep its macOS branch"
    );
    assert!(
        section.contains("Windows"),
        "the embedded setup section regressed to macOS-only instructions; \
         a Windows reader is sent to an affordance that does not exist"
    );
}

fn first_time_setup_section(document: &str) -> String {
    let start = document
        .find("## First-Time Setup")
        .expect("First-Time Setup section exists");
    let rest = &document[start..];
    let end = rest[1..]
        .find("\n## ")
        .map(|offset| offset + 1)
        .unwrap_or(rest.len());
    rest[..end].to_owned()
}

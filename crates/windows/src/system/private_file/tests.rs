use std::hash::{BuildHasher, RandomState};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "locality_tests.rs"]
mod locality_tests;
#[path = "owner_tests.rs"]
mod owner_tests;

#[path = "owner_token_tests.rs"]
mod owner_token_tests;
#[path = "path_tests.rs"]
mod path_tests;
#[path = "replace_tests.rs"]
mod replace_tests;

static SCRATCH_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(super) struct Scratch {
    root: PathBuf,
}

impl Scratch {
    pub(super) fn new(name: &str) -> Self {
        Self::adopt(std::env::temp_dir().join(format!(
            "agent-desktop-pf-{name}-{}-{:016x}",
            std::process::id(),
            scratch_nonce()
        )))
    }

    pub(super) fn adopt(root: PathBuf) -> Self {
        std::fs::create_dir_all(&root).expect("scratch root must be creatable");
        Self { root }
    }

    pub(super) fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub(super) fn scratch_nonce() -> u64 {
    RandomState::new().hash_one((
        std::process::id(),
        SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed),
        std::time::SystemTime::now(),
    ))
}

pub(super) fn create_junction(link: &Path, target: &Path) {
    let status = std::process::Command::new("cmd")
        .arg("/c")
        .arg("mklink")
        .arg("/J")
        .arg(link)
        .arg(target)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("cmd /c mklink /J must spawn");
    assert!(status.success(), "junction creation must succeed");
    let created = std::fs::symlink_metadata(link).expect("junction metadata must be readable");
    assert!(
        created.file_type().is_symlink(),
        "the planted link must surface as a reparse point"
    );
}

#[test]
fn no_banned_acl_or_ace_symbol_appears_anywhere_in_this_module() {
    let sources: &[(&str, &str)] = &[
        ("mod.rs", include_str!("mod.rs")),
        ("path.rs", include_str!("path.rs")),
        ("replace.rs", include_str!("replace.rs")),
        ("owner.rs", include_str!("owner.rs")),
        ("locality.rs", include_str!("locality.rs")),
        ("tests.rs", include_str!("tests.rs")),
        ("path_tests.rs", include_str!("path_tests.rs")),
        ("replace_tests.rs", include_str!("replace_tests.rs")),
        ("owner_tests.rs", include_str!("owner_tests.rs")),
        ("owner_token_tests.rs", include_str!("owner_token_tests.rs")),
        ("locality_tests.rs", include_str!("locality_tests.rs")),
    ];
    let banned_symbols: Vec<String> = [
        ("Get", "Ace"),
        ("Get", "AclInformation"),
        ("Initialize", "Acl"),
        ("AddAccessAllowed", "AceEx"),
    ]
    .iter()
    .map(|(head, tail)| format!("{head}{tail}"))
    .collect();
    for (name, contents) in sources {
        for symbol in &banned_symbols {
            assert!(
                !contents.contains(symbol.as_str()),
                "{name} must not mention the banned ACL/ACE symbol {symbol}"
            );
        }
    }
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/system/private_file");
    let mut on_disk: Vec<String> = std::fs::read_dir(directory)
        .expect("the module directory must be listable")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".rs"))
        .collect();
    let mut scanned: Vec<String> = sources
        .iter()
        .map(|(name, _)| (*name).to_string())
        .collect();
    on_disk.sort();
    scanned.sort();
    assert_eq!(
        on_disk, scanned,
        "every source file in the module directory must be covered by this scan"
    );
}

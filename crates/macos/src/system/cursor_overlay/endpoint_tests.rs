use super::*;
use std::os::unix::fs::PermissionsExt;

#[test]
fn long_state_root_uses_a_short_deterministic_socket_path() {
    let root = Path::new("/private/tmp").join("deep".repeat(40));
    let first = path_for_root(&root, "run-1", None);
    let second = path_for_root(&root, "run-1", None);

    assert_eq!(first, second);
    assert_eq!(first.parent(), Some(private_fallback_root().as_path()));
    assert!(first.as_os_str().as_bytes().len() < 100);
}

#[test]
fn private_directory_is_owner_matched_and_mode_restricted() {
    let directory = std::env::temp_dir().join(format!(
        "agent-desktop-endpoint-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir(&directory);
    std::fs::create_dir(&directory).expect("create test directory");
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755))
        .expect("make test directory permissive");
    assert!(ensure_private_directory(&directory).is_err());
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
        .expect("protect test directory");

    ensure_private_directory(&directory).expect("restrict test directory");
    let metadata = std::fs::symlink_metadata(&directory).expect("read test directory");
    assert!(metadata.file_type().is_dir());
    assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
    assert_eq!(metadata.mode() & 0o777, 0o700);
    std::fs::remove_dir(&directory).expect("remove test directory");
}

#[test]
fn protocol_generation_uses_a_distinct_socket() {
    let root = Path::new("/private/tmp/state");

    assert_ne!(
        path_for_root(root, "run-1", None),
        path_for_root(root, "run-1", Some("agent-a"))
    );
}

#[test]
fn current_protocol_never_reuses_a_previous_generation_renderer_socket() {
    let root = Path::new("/private/tmp/state");
    let v2_default = socket_for_name(
        root,
        format!(
            ".cursor-overlay-{:016x}.sock",
            endpoint_hash(root, "run-1", Some("v2"))
        ),
    );

    assert_eq!(PREVIOUS_PROTOCOL_VERSION, "v2");
    assert_eq!(
        path_for_protocol(root, "run-1", None, PREVIOUS_PROTOCOL_VERSION),
        v2_default
    );
    assert_ne!(path_for_root(root, "run-1", None), v2_default);
    for agent in [None, Some("agent-a")] {
        assert_ne!(
            path_for_root(root, "run-1", agent),
            path_for_protocol(root, "run-1", agent, PREVIOUS_PROTOCOL_VERSION)
        );
    }
}

#[test]
fn teardown_scan_still_finds_previous_generation_agent_sockets() {
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let directory = PathBuf::from(format!("/tmp/ae-g{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("create scan directory");
    let prefixes = [PROTOCOL_VERSION, PREVIOUS_PROTOCOL_VERSION]
        .map(|protocol| agent_prefix(&directory, "run-1", protocol));
    let current = directory.join(format!("{}0011223344556677.sock", prefixes[0]));
    let previous = directory.join(format!("{}8899aabbccddeeff.sock", prefixes[1]));
    let current_listener = bind_private_socket(&current);
    let previous_listener = bind_private_socket(&previous);

    let (paths, error) = collect_session_sockets(std::slice::from_ref(&directory), &prefixes);

    assert!(error.is_none());
    let mut expected = vec![current, previous];
    expected.sort();
    assert_eq!(paths, expected);
    drop((current_listener, previous_listener));
    std::fs::remove_dir_all(&directory).expect("remove scan directory");
}

#[test]
fn named_endpoints_are_distinct_and_session_prefixed() {
    let root = Path::new("/private/tmp/state");
    let first = path_for_root(root, "run-1", Some("agent-a"));
    let second = path_for_root(root, "run-1", Some("agent-b"));
    assert_ne!(first, second);
    assert!(first.file_name().unwrap().to_string_lossy().contains('-'));
}

#[test]
fn v2_is_a_valid_agent_id_without_colliding_with_default() {
    let root = Path::new("/private/tmp/state");
    assert_ne!(
        path_for_root(root, "run-1", None),
        path_for_root(root, "run-1", Some("v2"))
    );
}

fn bind_private_socket(path: &Path) -> std::os::unix::net::UnixListener {
    let listener = std::os::unix::net::UnixListener::bind(path).expect("bind test socket");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .expect("protect test socket");
    listener
}

#[test]
fn socket_validation_uses_the_shared_ownership_predicate() {
    let directory = std::env::temp_dir().join(format!("ae-p{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir(&directory).expect("create predicate directory");
    let missing = directory.join("missing.sock");
    let regular = directory.join("regular.sock");
    std::fs::write(&regular, "probe").expect("write probe file");
    let link = directory.join("link.sock");
    std::os::unix::fs::symlink(&regular, &link).expect("link probe file");
    let socket = directory.join("live.sock");
    let listener = bind_private_socket(&socket);
    assert!(!is_private_socket(&missing));
    assert!(validate_socket_path(&missing).is_ok());
    for path in [&regular, &link, &directory] {
        assert!(!is_private_socket(path));
        assert!(validate_socket_path(path).is_err());
    }
    assert!(is_private_socket(&socket));
    assert!(validate_socket_path(&socket).is_ok());
    drop(listener);
    std::fs::remove_dir_all(&directory).expect("remove predicate directory");
}

#[test]
fn partial_listing_keeps_sockets_but_reports_unconfirmed_teardown() {
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let directory = std::env::temp_dir().join(format!("ae-s{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let listed = directory.join("l");
    let blocked = directory.join("b");
    std::fs::create_dir_all(&listed).expect("create listed directory");
    std::fs::create_dir_all(&blocked).expect("create blocked directory");
    let prefix = ".co-0123abcd-";
    let socket = listed.join(format!("{prefix}0011223344556677.sock"));
    let listener = bind_private_socket(&socket);
    let decoy = listed.join(format!("{prefix}aabbccddeeff0011.sock"));
    std::fs::write(&decoy, "probe").expect("write decoy file");
    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o000))
        .expect("block directory listing");
    let (paths, error) = collect_session_sockets(&[listed, blocked.clone()], &[prefix.to_owned()]);
    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o700))
        .expect("restore blocked directory");
    assert!(!paths.contains(&decoy));
    assert_eq!(paths, vec![socket]);
    assert!(error.is_some());
    drop(listener);
    std::fs::remove_dir_all(&directory).expect("remove scan directory");
}

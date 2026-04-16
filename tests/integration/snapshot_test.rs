/// Integration tests for the snapshot command.
///
/// These tests require macOS with Accessibility permissions granted to the
/// terminal running the tests. They are skipped automatically on other
/// platforms or when the binary is not built.
#[cfg(test)]
mod tests {
    use std::process::Command;

    fn agent_desktop_bin() -> std::path::PathBuf {
        let mut p = std::env::current_exe().unwrap();
        p.pop();
        p.pop();
        p.push("agent-desktop");
        p
    }

    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "requires Accessibility permissions and running macOS apps"]
    fn snapshot_finder_returns_non_empty_tree() {
        let bin = agent_desktop_bin();
        let output = Command::new(&bin)
            .args(["snapshot", "--app", "Finder"])
            .output()
            .expect("failed to run agent-desktop");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("output is not valid JSON");

        assert_eq!(json["ok"], true);
        assert!(json["data"]["ref_count"].as_u64().unwrap_or(0) > 0);
    }

    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "requires Accessibility permissions and running macOS apps"]
    fn snapshot_textedit_returns_refs() {
        let bin = agent_desktop_bin();
        let output = Command::new(&bin)
            .args(["snapshot", "--app", "TextEdit"])
            .output()
            .expect("failed to run agent-desktop");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("output is not valid JSON");

        assert_eq!(json["ok"], true);
    }

    #[test]
    fn version_command_outputs_json() {
        let bin = agent_desktop_bin();
        if !bin.exists() {
            return; // binary not built yet
        }
        let output = Command::new(&bin)
            .args(["version", "--json"])
            .output()
            .expect("failed to run agent-desktop");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("output is not valid JSON");

        assert_eq!(json["ok"], true);
        assert!(json["data"]["version"].is_string());
    }

    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "requires Accessibility permissions and running macOS apps"]
    fn snapshot_skeleton_returns_shallow_tree_with_children_count() {
        let bin = agent_desktop_bin();
        let output = Command::new(&bin)
            .args(["snapshot", "--app", "Finder", "--skeleton", "-i"])
            .output()
            .expect("failed to run agent-desktop");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("output is not valid JSON");

        assert_eq!(json["ok"], true);
        let tree = &json["data"]["tree"];
        let max_depth = find_max_depth(tree, 0);
        assert!(
            max_depth <= 4,
            "skeleton must clamp to depth ~3, got depth {max_depth}"
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "requires Accessibility permissions and running macOS apps"]
    fn snapshot_skeleton_refresh_does_not_accumulate_stale_refs() {
        let bin = agent_desktop_bin();
        let run = |extra: &[&str]| {
            let mut args = vec!["snapshot", "--app", "Finder", "--skeleton", "-i"];
            args.extend_from_slice(extra);
            Command::new(&bin)
                .args(&args)
                .output()
                .expect("failed to run agent-desktop")
        };

        let first = run(&[]);
        let first_json: serde_json::Value =
            serde_json::from_str(&String::from_utf8_lossy(&first.stdout)).unwrap();
        let first_count = first_json["data"]["ref_count"].as_u64().unwrap_or(0);

        let second = run(&[]);
        let second_json: serde_json::Value =
            serde_json::from_str(&String::from_utf8_lossy(&second.stdout)).unwrap();
        let second_count = second_json["data"]["ref_count"].as_u64().unwrap_or(0);

        assert_eq!(
            first_count, second_count,
            "repeated skeleton refresh must produce identical ref_count (no accumulation)"
        );
    }

    #[test]
    fn snapshot_invalid_root_ref_format_returns_invalid_args() {
        let bin = agent_desktop_bin();
        if !bin.exists() {
            return;
        }
        let output = Command::new(&bin)
            .args(["snapshot", "--app", "Finder", "--root", "bad-ref"])
            .output()
            .expect("failed to run agent-desktop");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("output is not valid JSON");

        assert_eq!(json["ok"], false);
        assert_eq!(
            json["error"]["code"], "INVALID_ARGS",
            "malformed --root must return INVALID_ARGS, got: {}",
            json["error"]["code"]
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "requires Accessibility permissions and running macOS apps"]
    fn snapshot_root_drill_returns_non_empty_subtree() {
        let bin = agent_desktop_bin();
        let skeleton_out = Command::new(&bin)
            .args(["snapshot", "--app", "Finder", "--skeleton", "-i"])
            .output()
            .expect("failed to run agent-desktop");

        let skeleton_json: serde_json::Value =
            serde_json::from_str(&String::from_utf8_lossy(&skeleton_out.stdout)).unwrap();
        assert_eq!(skeleton_json["ok"], true);

        let first_ref = first_ref_id(&skeleton_json["data"]["tree"]);
        let Some(ref_id) = first_ref else {
            return;
        };

        let drill_out = Command::new(&bin)
            .args(["snapshot", "--app", "Finder", "--root", &ref_id, "-i"])
            .output()
            .expect("failed to run agent-desktop");

        let drill_json: serde_json::Value =
            serde_json::from_str(&String::from_utf8_lossy(&drill_out.stdout)).unwrap();

        assert_eq!(drill_json["ok"], true);
        assert!(
            drill_json["data"]["ref_count"].as_u64().unwrap_or(0) > 0,
            "drill-down must return refs"
        );
    }

    fn find_max_depth(node: &serde_json::Value, depth: usize) -> usize {
        let children = match node.get("children").and_then(|c| c.as_array()) {
            Some(c) if !c.is_empty() => c,
            _ => return depth,
        };
        children
            .iter()
            .map(|c| find_max_depth(c, depth + 1))
            .max()
            .unwrap_or(depth)
    }

    fn first_ref_id(node: &serde_json::Value) -> Option<String> {
        if let Some(r) = node.get("ref_id").and_then(|v| v.as_str()) {
            return Some(r.to_string());
        }
        if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
            for child in children {
                if let Some(r) = first_ref_id(child) {
                    return Some(r);
                }
            }
        }
        None
    }

    #[test]
    fn list_apps_on_non_macos_errors_gracefully() {
        #[cfg(not(target_os = "macos"))]
        {
            let bin = agent_desktop_bin();
            if !bin.exists() {
                return;
            }
            let output = Command::new(&bin)
                .args(["list-apps"])
                .output()
                .expect("failed to run agent-desktop");

            let stdout = String::from_utf8_lossy(&output.stdout);
            let json: serde_json::Value =
                serde_json::from_str(&stdout).expect("output is not valid JSON");

            assert_eq!(json["ok"], false);
            assert_eq!(json["error"]["code"], "PLATFORM_NOT_SUPPORTED");
        }
    }
}

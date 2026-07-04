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

    /// Regression guard for the AX-to-CGWindowID bridge: every id that
    /// `list-windows` reports must resolve back to a real accessibility window
    /// in `snapshot`. U6 briefly keyed this match on the nonexistent
    /// `AXWindowNumber` attribute, so `snapshot` returned `WINDOW_NOT_FOUND`
    /// for every app while mock-adapter unit tests stayed green. The bridge
    /// now uses `_AXUIElementGetWindow`; this fails closed if it regresses.
    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "requires Accessibility permissions and running macOS apps"]
    fn snapshot_resolves_a_window_id_reported_by_list_windows() {
        let bin = agent_desktop_bin();
        let list = Command::new(&bin)
            .args(["list-windows", "--app", "Finder"])
            .output()
            .expect("failed to run agent-desktop");
        let list_json: serde_json::Value =
            serde_json::from_str(&String::from_utf8_lossy(&list.stdout)).unwrap();
        let ids: Vec<String> = list_json["data"]
            .as_array()
            .map(|windows| {
                windows
                    .iter()
                    .filter_map(|w| w["id"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if ids.is_empty() {
            return;
        }

        let snap = Command::new(&bin)
            .args(["snapshot", "--app", "Finder"])
            .output()
            .expect("failed to run agent-desktop");
        let snap_json: serde_json::Value =
            serde_json::from_str(&String::from_utf8_lossy(&snap.stdout)).unwrap();

        assert_eq!(
            snap_json["ok"], true,
            "snapshot must resolve a live window, got error {}",
            snap_json["error"]["code"]
        );
        let resolved = snap_json["data"]["window"]["id"].as_str().unwrap_or("");
        assert!(
            ids.iter().any(|id| id == resolved),
            "snapshot window id {resolved:?} must be one reported by list-windows {ids:?}"
        );
    }

    /// Regression guard for accessible-name consistency: an element found by
    /// role reports an accessible name; searching that same role by that exact
    /// name must return the same element. The name computation diverged across
    /// the builder, live matcher, and strict resolver on this branch, so
    /// `find --name` returned null even for the name the element itself
    /// reported. Fails closed if those name paths drift apart again.
    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "requires Accessibility permissions and running macOS apps"]
    fn find_by_name_matches_the_element_that_reports_that_name() {
        let bin = agent_desktop_bin();
        let by_role = Command::new(&bin)
            .args(["find", "--app", "Finder", "--role", "button", "--first"])
            .output()
            .expect("failed to run agent-desktop");
        let role_json: serde_json::Value =
            serde_json::from_str(&String::from_utf8_lossy(&by_role.stdout)).unwrap();
        let matched = &role_json["data"]["match"];
        let (Some(ref_id), Some(name)) = (matched["ref_id"].as_str(), matched["name"].as_str())
        else {
            return;
        };
        if name.is_empty() || name.starts_with("(unnamed") {
            return;
        }

        let by_name = Command::new(&bin)
            .args([
                "find", "--app", "Finder", "--role", "button", "--name", name, "--first",
            ])
            .output()
            .expect("failed to run agent-desktop");
        let name_json: serde_json::Value =
            serde_json::from_str(&String::from_utf8_lossy(&by_name.stdout)).unwrap();

        assert_eq!(
            name_json["data"]["match"]["ref_id"].as_str(),
            Some(ref_id),
            "find --name {name:?} must resolve the same element find --role reported with that \
             name; got {}",
            name_json["data"]["match"]
        );
    }

    /// Regression guard for strict ref re-resolution: a ref just produced by
    /// `find` must re-resolve through `get`. Ref identity re-derivation
    /// recomputed a different accessible name than the one stored in the ref,
    /// so freshly created refs returned STALE_REF. Fails closed if identity and
    /// the stored name drift apart again.
    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "requires Accessibility permissions and running macOS apps"]
    fn a_ref_from_find_reresolves_through_get() {
        let bin = agent_desktop_bin();
        let found = Command::new(&bin)
            .args(["find", "--app", "Finder", "--role", "button", "--first"])
            .output()
            .expect("failed to run agent-desktop");
        let found_json: serde_json::Value =
            serde_json::from_str(&String::from_utf8_lossy(&found.stdout)).unwrap();
        let Some(ref_id) = found_json["data"]["match"]["ref_id"].as_str() else {
            return;
        };

        let got = Command::new(&bin)
            .args(["get", ref_id, "--property", "role"])
            .output()
            .expect("failed to run agent-desktop");
        let got_json: serde_json::Value =
            serde_json::from_str(&String::from_utf8_lossy(&got.stdout)).unwrap();

        assert_eq!(
            got_json["ok"], true,
            "a ref from find must re-resolve through get, got error {}",
            got_json["error"]["code"]
        );
    }

    #[test]
    fn version_command_outputs_json() {
        let bin = agent_desktop_bin();
        if !bin.exists() {
            return;
        }
        let output = Command::new(&bin)
            .args(["version"])
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

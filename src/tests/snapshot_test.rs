#[cfg(test)]
mod tests {
    use std::process::Command;

    fn run(args: &[&str]) -> serde_json::Value {
        let output = Command::new(env!("CARGO_BIN_EXE_agent-desktop"))
            .args(args)
            .output()
            .expect("failed to run agent-desktop");
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "agent-desktop output is not JSON: {error}; stderr={}",
                String::from_utf8_lossy(&output.stderr)
            )
        })
    }

    /// Guards the native AX-to-CG window identity bridge used by snapshots.
    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "requires Accessibility permissions and running macOS apps"]
    fn snapshot_resolves_a_window_id_reported_by_list_windows() {
        let list = run(&["list-windows", "--app", "Finder"]);
        let ids: Vec<&str> = list["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|window| window["id"].as_str())
            .collect();
        if ids.is_empty() {
            eprintln!(
                "SKIP snapshot_resolves_a_window_id_reported_by_list_windows: Finder has no windows"
            );
            return;
        }

        let snapshot = run(&["snapshot", "--app", "Finder"]);
        assert_eq!(
            snapshot["ok"], true,
            "snapshot must resolve a live window, got error {}",
            snapshot["error"]["code"]
        );
        let resolved = snapshot["data"]["window"]["id"].as_str().unwrap_or("");
        assert!(
            ids.contains(&resolved),
            "snapshot window id {resolved:?} must be reported by list-windows {ids:?}"
        );
    }

    /// Guards consistent accessible-name computation across snapshot and find.
    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "requires Accessibility permissions and running macOS apps"]
    fn find_by_name_matches_the_element_that_reports_that_name() {
        let by_role = run(&["find", "--app", "Finder", "--role", "button", "--first"]);
        let matched = &by_role["data"]["match"];
        let (Some(ref_id), Some(name)) = (matched["ref_id"].as_str(), matched["name"].as_str())
        else {
            eprintln!(
                "SKIP find_by_name_matches_the_element_that_reports_that_name: no named button"
            );
            return;
        };
        if name.is_empty() || name.starts_with("(unnamed") {
            eprintln!(
                "SKIP find_by_name_matches_the_element_that_reports_that_name: unusable name"
            );
            return;
        }

        let by_name = run(&[
            "find", "--app", "Finder", "--role", "button", "--name", name, "--first",
        ]);
        assert_eq!(
            by_name["data"]["match"]["ref_id"].as_str(),
            Some(ref_id),
            "find --name {name:?} must resolve the element that reported that name"
        );
    }

    /// Guards strict identity re-resolution for a freshly produced ref.
    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "requires Accessibility permissions and running macOS apps"]
    fn a_ref_from_find_reresolves_through_get() {
        let found = run(&["find", "--app", "Finder", "--role", "button", "--first"]);
        let Some(ref_id) = found["data"]["match"]["ref_id"].as_str() else {
            eprintln!("SKIP a_ref_from_find_reresolves_through_get: no button found");
            return;
        };

        let got = run(&["get", ref_id, "--property", "role"]);
        assert_eq!(
            got["ok"], true,
            "a ref from find must re-resolve through get, got error {}",
            got["error"]["code"]
        );
    }

    #[test]
    fn real_app_regression_gate_stays_registered() {
        let source = include_str!("snapshot_test.rs");
        assert_eq!(source.matches(concat!("#[", "ignore =")).count(), 3);
        for name in [
            "snapshot_resolves_a_window_id_reported_by_list_windows",
            "find_by_name_matches_the_element_that_reports_that_name",
            "a_ref_from_find_reresolves_through_get",
        ] {
            assert!(source.contains(&format!("fn {name}()")));
        }
    }

    #[test]
    fn version_command_outputs_json() {
        let json = run(&["version"]);
        assert_eq!(json["ok"], true);
        assert!(json["data"]["version"].is_string());
    }

    #[test]
    fn snapshot_invalid_root_ref_format_returns_invalid_args() {
        let json = run(&["snapshot", "--app", "Finder", "--root", "bad-ref"]);
        assert_eq!(json["ok"], false);
        assert_eq!(json["error"]["code"], "INVALID_ARGS");
    }

    #[test]
    fn list_apps_inventory_matches_the_platform() {
        #[cfg(target_os = "windows")]
        {
            let json = run(&["list-apps"]);
            assert_eq!(json["ok"], true, "list-apps is live on Windows");
            assert!(
                json["data"]["apps"].is_array(),
                "list-apps returns an apps array on Windows"
            );
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            let json = run(&["list-apps"]);
            assert_eq!(json["ok"], false);
            assert_eq!(json["error"]["code"], "PLATFORM_NOT_SUPPORTED");
        }
    }
}
